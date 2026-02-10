use crate::auth::AuthMethod;
use crate::pagination::{next_link_from_body, next_link_from_headers};
use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use reqwest::header::HeaderMap;
#[cfg(feature = "write")]
use reqwest::{Body, multipart};
use reqwest::{Client as HttpClient, Method, Response};
use serde_json::Value;
#[cfg(feature = "write")]
use std::path::Path;
use std::time::Duration;
#[cfg(feature = "write")]
use tokio_util::io::ReaderStream;
use url::Url;

const MAX_ATTEMPTS: u32 = 3;
const API_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const USER_AGENT: &str = concat!("confcli/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Clone)]
pub struct ApiClient {
    /// Web base URL (used for browser links, download links, etc).
    site_url: String,
    /// API base URL for Confluence REST v1.
    api_base_v1: String,
    /// API base URL for Confluence REST v2 (Cloud).
    api_base_v2: String,
    /// Scheme+host(+port), used for absolute URLs from pagination/link headers.
    origin: String,
    auth: AuthMethod,
    http: HttpClient,
    verbose: u8,
}

impl ApiClient {
    pub fn new(
        site_url: String,
        api_base_v1: String,
        api_base_v2: String,
        auth: AuthMethod,
        verbose: u8,
    ) -> Result<Self> {
        let site_url = site_url.trim_end_matches('/').to_string();
        let api_base_v1 = api_base_v1.trim_end_matches('/').to_string();
        let api_base_v2 = api_base_v2.trim_end_matches('/').to_string();
        let origin = origin_from_url(&site_url)?;
        let http = HttpClient::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(10))
            .build()?;
        Ok(Self {
            site_url,
            api_base_v1,
            api_base_v2,
            origin,
            auth,
            http,
            verbose,
        })
    }

    pub fn base_url(&self) -> &str {
        &self.site_url
    }

    pub fn origin_url(&self) -> &str {
        &self.origin
    }

    pub fn http(&self) -> &HttpClient {
        &self.http
    }

    pub fn v2_url(&self, path: &str) -> String {
        format!("{}{}", self.api_base_v2, path)
    }

    pub fn v1_url(&self, path: &str) -> String {
        format!("{}{}", self.api_base_v1, path)
    }

    pub fn apply_auth(&self, builder: reqwest::RequestBuilder) -> Result<reqwest::RequestBuilder> {
        match &self.auth {
            AuthMethod::Basic { email, token } => {
                let raw = format!("{email}:{token}");
                let encoded = base64::engine::general_purpose::STANDARD.encode(raw);
                Ok(builder.header("Authorization", format!("Basic {encoded}")))
            }
            AuthMethod::Bearer { token } => {
                Ok(builder.header("Authorization", format!("Bearer {token}")))
            }
        }
    }

    /// Parse a Retry-After header value (integer seconds), falling back to
    /// exponential backoff: 2^(attempt-1) seconds.
    pub fn retry_wait_from_headers(headers: &HeaderMap, attempt: u32) -> Duration {
        if let Some(val) = headers.get("retry-after")
            && let Ok(s) = val.to_str()
            && let Ok(secs) = s.trim().parse::<u64>()
        {
            return Duration::from_secs(secs) + jitter(Duration::from_millis(250));
        }
        Duration::from_secs(2u64.pow(attempt - 1)) + jitter(Duration::from_millis(250))
    }

    async fn send_impl<F>(&self, method: Method, url: String, mut configure: F) -> Result<Response>
    where
        F: FnMut(reqwest::RequestBuilder) -> reqwest::RequestBuilder,
    {
        let mut attempts = 0;

        loop {
            if self.verbose > 0 {
                if attempts > 0 {
                    eprintln!("{} {} (retry {})", method, url, attempts);
                } else {
                    eprintln!("{} {}", method, url);
                }
            }

            let start = std::time::Instant::now();
            let builder = self
                .http
                .request(method.clone(), url.clone())
                .timeout(API_REQUEST_TIMEOUT);
            let builder = configure(builder);
            let builder = self.apply_auth(builder)?;

            match builder.send().await {
                Ok(response) => {
                    if self.verbose > 1 {
                        eprintln!("<- {} ({:?})", response.status(), start.elapsed());
                        if let Some(id) = request_id(response.headers()) {
                            eprintln!("<- request-id: {id}");
                        }
                    }

                    if response.status().is_success() {
                        return Ok(response);
                    }

                    let status = response.status();
                    if attempts < MAX_ATTEMPTS && (status == 429 || status.is_server_error()) {
                        attempts += 1;
                        let wait = Self::retry_wait_from_headers(response.headers(), attempts);
                        if self.verbose > 0 {
                            eprintln!("Received {}, retrying in {:?}...", status, wait);
                        }
                        tokio::time::sleep(wait).await;
                        continue;
                    }

                    let body = response.text().await.unwrap_or_default();
                    let msg = friendly_error(status, &body);
                    if self.verbose > 0 {
                        return Err(anyhow!(format!("{msg}\n\nResponse body:\n{body}")));
                    }
                    bail!("{msg}");
                }
                Err(e) => {
                    if attempts < MAX_ATTEMPTS {
                        attempts += 1;
                        // No response headers on request errors; still use the same backoff+jitter.
                        let wait = Self::retry_wait_from_headers(&HeaderMap::new(), attempts);
                        if self.verbose > 0 {
                            eprintln!("Request error: {}, retrying in {:?}...", e, wait);
                        }
                        tokio::time::sleep(wait).await;
                        continue;
                    }
                    return Err(e.into());
                }
            }
        }
    }

    async fn send(&self, method: Method, url: String) -> Result<Response> {
        self.send_impl(method, url, |b| b).await
    }

    /// Send a request with a JSON body, using the same retry logic as `send()`.
    #[cfg(feature = "write")]
    async fn send_with_json_body(
        &self,
        method: Method,
        url: String,
        body: &Value,
    ) -> Result<Response> {
        self.send_impl(method, url, |b| b.json(body)).await
    }

    pub async fn get_json(&self, url: String) -> Result<(Value, HeaderMap)> {
        let response = self.send(Method::GET, url).await?;
        let headers = response.headers().clone();
        let json = response.json::<Value>().await?;
        Ok((json, headers))
    }

    pub async fn get_paginated_results(&self, url: String, all: bool) -> Result<Vec<Value>> {
        self.get_paginated_results_with_limit(url, all, 10_000)
            .await
    }

    async fn get_paginated_results_with_limit(
        &self,
        url: String,
        all: bool,
        max_pages: usize,
    ) -> Result<Vec<Value>> {
        let mut results = Vec::new();
        let mut next_url: Option<String> = Some(url);
        let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut pages = 0usize;

        while let Some(url) = next_url {
            pages += 1;
            if pages > max_pages {
                bail!("Pagination aborted after {max_pages} pages (possible looping 'next' link)");
            }
            if !visited.insert(url.clone()) {
                bail!("Pagination loop detected: already visited next URL: {url}");
            }

            let (json, headers) = self.get_json(url.clone()).await?;
            if let Some(array) = json.get("results").and_then(|v| v.as_array()) {
                results.extend(array.iter().cloned());
            } else if json.is_array() {
                results.extend(json.as_array().cloned().unwrap_or_default());
            } else {
                bail!("Unexpected response shape: missing results array");
            }

            if !all {
                break;
            }

            let next = next_link_from_headers(&headers).or_else(|| next_link_from_body(&json));
            next_url = match next {
                Some(next) => Some(resolve_next_page_url(&url, &next)?),
                None => None,
            };
        }
        Ok(results)
    }

    #[cfg(feature = "write")]
    pub async fn post_json(&self, url: String, body: Value) -> Result<Value> {
        let response = self.send_with_json_body(Method::POST, url, &body).await?;
        Ok(response.json::<Value>().await?)
    }

    #[cfg(feature = "write")]
    pub async fn put_json(&self, url: String, body: Value) -> Result<Value> {
        let response = self.send_with_json_body(Method::PUT, url, &body).await?;
        Ok(response.json::<Value>().await?)
    }

    #[cfg(feature = "write")]
    pub async fn delete(&self, url: String) -> Result<()> {
        let response = self.send(Method::DELETE, url).await?;
        drop(response);
        Ok(())
    }

    /// Upload an attachment via the v1 API.
    ///
    /// Retries are implemented by re-opening the file and rebuilding the
    /// multipart form on each attempt.
    #[cfg(feature = "write")]
    pub async fn upload_attachment(
        &self,
        page_id: &str,
        file_path: &Path,
        comment: Option<String>,
    ) -> Result<Value> {
        let url = self.v1_url(&format!("/content/{}/child/attachment", page_id));
        let file_name = file_path
            .file_name()
            .and_then(|v| v.to_str())
            .context("Invalid file name")?
            .to_string();

        let mut attempts = 0;
        loop {
            if self.verbose > 0 {
                if attempts > 0 {
                    eprintln!("POST {} (upload retry {})", url, attempts);
                } else {
                    eprintln!("POST {} (upload)", url);
                }
            }

            let file = tokio::fs::File::open(file_path)
                .await
                .with_context(|| format!("Failed to open attachment: {}", file_path.display()))?;
            let metadata = file.metadata().await?;
            let size = metadata.len();

            let stream = ReaderStream::new(file);
            let body = Body::wrap_stream(stream);
            let part = multipart::Part::stream_with_length(body, size).file_name(file_name.clone());

            let mut form = multipart::Form::new().part("file", part);
            if let Some(comment) = comment.clone() {
                form = form.text("comment", comment);
            }

            let builder = self
                .http
                .post(url.clone())
                .multipart(form)
                .header("X-Atlassian-Token", "no-check");
            let builder = self.apply_auth(builder)?;

            match builder.send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        return Ok(response.json::<Value>().await?);
                    }

                    let status = response.status();
                    if attempts < MAX_ATTEMPTS && (status == 429 || status.is_server_error()) {
                        attempts += 1;
                        let wait = Self::retry_wait_from_headers(response.headers(), attempts);
                        if self.verbose > 0 {
                            eprintln!("Upload received {}, retrying in {:?}...", status, wait);
                        }
                        tokio::time::sleep(wait).await;
                        continue;
                    }

                    let body = response.text().await.unwrap_or_default();
                    let msg = friendly_error(status, &body);
                    if self.verbose > 0 {
                        return Err(anyhow!(format!(
                            "Upload failed: {msg}\n\nResponse body:\n{body}"
                        )));
                    }
                    bail!("Upload failed: {msg}");
                }
                Err(e) => {
                    if attempts < MAX_ATTEMPTS {
                        attempts += 1;
                        let wait = Self::retry_wait_from_headers(&HeaderMap::new(), attempts);
                        if self.verbose > 0 {
                            eprintln!("Upload request error: {}, retrying in {:?}...", e, wait);
                        }
                        tokio::time::sleep(wait).await;
                        continue;
                    }
                    return Err(e.into());
                }
            }
        }
    }
}

fn resolve_next_page_url(current_url: &str, next: &str) -> Result<String> {
    if let Ok(abs) = Url::parse(next) {
        return Ok(abs.to_string());
    }

    let current = Url::parse(current_url)
        .with_context(|| format!("Invalid pagination URL '{current_url}'"))?;
    current
        .join(next)
        .with_context(|| {
            format!("Failed to resolve pagination next link '{next}' from '{current_url}'")
        })
        .map(|u| u.to_string())
}

fn origin_from_url(site_url: &str) -> Result<String> {
    let url = Url::parse(site_url)?;
    let host = url
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid Confluence URL: missing host"))?;
    let port = url.port().map(|p| format!(":{p}")).unwrap_or_default();
    Ok(format!("{}://{}{}", url.scheme(), host, port))
}

fn jitter(max: Duration) -> Duration {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64;
    let max_ms = max.as_millis() as u64;
    if max_ms == 0 {
        return Duration::from_millis(0);
    }
    let ms = nanos % max_ms;
    Duration::from_millis(ms)
}

/// Produce a human-friendly error message from an HTTP status + body.
///
/// This intentionally avoids printing large raw response bodies by default.
/// For detailed diagnostics, callers can attach the response body as context
/// when `-v/-vv` is enabled.
pub fn friendly_error(status: reqwest::StatusCode, body: &str) -> String {
    fn clean(s: &str, max_chars: usize) -> String {
        // Stream whitespace-collapsing + truncation into a single String.
        // Avoids allocating an intermediate Vec (split_whitespace -> collect -> join).
        let mut out = String::new();
        let mut count = 0usize;

        for word in s.split_whitespace() {
            if !out.is_empty() {
                if count >= max_chars {
                    out.push('…');
                    return out;
                }
                out.push(' ');
                count += 1;
            }

            for ch in word.chars() {
                if count >= max_chars {
                    out.push('…');
                    return out;
                }
                out.push(ch);
                count += 1;
            }
        }

        out
    }

    // Extract a message/title field from Confluence error JSON (best-effort).
    let parsed = serde_json::from_str::<serde_json::Value>(body).ok();
    let extracted = parsed.as_ref().and_then(|v| {
        v.get("errors")
            .and_then(|e| e.as_array())
            .and_then(|a| a.first())
            .and_then(|e| e.get("title"))
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                v.get("message")
                    .and_then(|m| m.as_str())
                    .map(|s| s.to_string())
            })
    });

    if status == reqwest::StatusCode::CONFLICT {
        return "409 Conflict: the page was modified concurrently. Fetch the latest version and retry your update.".to_string();
    }

    let mut msg = if let Some(extracted_msg) = extracted.as_deref() {
        format!("{status}: {}", clean(extracted_msg, 240))
    } else {
        let reason = status
            .canonical_reason()
            .unwrap_or("Request failed")
            .to_string();
        let b = body.trim();
        if b.is_empty() {
            format!("{status}: {reason}")
        } else {
            // Non-JSON or unknown shape: include only a small snippet.
            format!("{status}: {reason}: {}", clean(b, 160))
        }
    };

    // Atlassian Cloud sometimes returns 404 for auth/permission failures.
    let extracted_is_generic_not_found = extracted
        .as_deref()
        .is_some_and(|s| s.eq_ignore_ascii_case("not found"));

    let needs_auth_hint = status == reqwest::StatusCode::UNAUTHORIZED
        || status == reqwest::StatusCode::FORBIDDEN
        || (status == reqwest::StatusCode::NOT_FOUND && extracted_is_generic_not_found);
    if needs_auth_hint {
        msg.push_str(" (this may be an auth/permission issue; run `confcli auth status`)");
    }

    msg
}

fn request_id(headers: &HeaderMap) -> Option<String> {
    for key in [
        "x-request-id",
        "x-arequestid",
        "x-trace-id",
        "x-b3-traceid",
        "traceparent",
    ] {
        if let Some(val) = headers.get(key)
            && let Ok(s) = val.to_str()
        {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthMethod;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;

    struct TestServer {
        base_url: String,
        shutdown: oneshot::Sender<()>,
        hits: Arc<AtomicUsize>,
    }

    impl TestServer {
        fn url(&self, path: &str) -> String {
            format!("{}{}", self.base_url, path)
        }
    }

    async fn start_server<F>(handler: F) -> TestServer
    where
        F: Fn(String, usize, &str) -> (u16, Vec<(String, String)>, Vec<u8>) + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{}", addr);
        let base_url_task = base_url.clone();

        let (tx, mut rx) = oneshot::channel::<()>();
        let hits = Arc::new(AtomicUsize::new(0));
        let hits_task = hits.clone();
        let handler = Arc::new(handler);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut rx => {
                        return;
                    }
                    res = listener.accept() => {
                        let (mut sock, _) = match res {
                            Ok(v) => v,
                            Err(_) => continue,
                        };

                        let mut buf = vec![0u8; 8192];
                        let n = match sock.read(&mut buf).await {
                            Ok(n) => n,
                            Err(_) => continue,
                        };
                        let req = String::from_utf8_lossy(&buf[..n]).to_string();
                        let first = req.lines().next().unwrap_or_default();
                        let raw_target = first.split_whitespace().nth(1).unwrap_or("/");
                        let target = if raw_target.starts_with("http://") || raw_target.starts_with("https://") {
                            Url::parse(raw_target).ok().map(|u| {
                                let mut s = u.path().to_string();
                                if let Some(q) = u.query() {
                                    s.push('?');
                                    s.push_str(q);
                                }
                                s
                            }).unwrap_or_else(|| "/".to_string())
                        } else {
                            raw_target.to_string()
                        };

                        let hit = hits_task.fetch_add(1, Ordering::SeqCst) + 1;
                        let (status, headers, body) = handler(base_url_task.clone(), hit, &target);

                        let reason = match status {
                            200 => "OK",
                            400 => "Bad Request",
                            404 => "Not Found",
                            429 => "Too Many Requests",
                            500 => "Internal Server Error",
                            _ => "OK",
                        };

                        let mut resp = Vec::new();
                        resp.extend_from_slice(format!("HTTP/1.1 {} {}\r\n", status, reason).as_bytes());
                        resp.extend_from_slice(b"Connection: close\r\n");
                        for (k, v) in headers {
                            resp.extend_from_slice(format!("{}: {}\r\n", k, v).as_bytes());
                        }
                        resp.extend_from_slice(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes());
                        resp.extend_from_slice(&body);
                        let _ = sock.write_all(&resp).await;
                        let _ = sock.shutdown().await;
                    }
                }
            }
        });

        TestServer {
            base_url,
            shutdown: tx,
            hits,
        }
    }

    fn test_client(base_url: &str) -> ApiClient {
        ApiClient::new(
            base_url.to_string(),
            base_url.to_string(),
            base_url.to_string(),
            AuthMethod::Bearer {
                token: "test".to_string(),
            },
            0,
        )
        .unwrap()
    }

    #[test]
    fn retry_wait_uses_retry_after_when_present() {
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", "5".parse().unwrap());
        let d = ApiClient::retry_wait_from_headers(&headers, 1);
        assert!(d >= Duration::from_secs(5));
        assert!(d < Duration::from_millis(5250));
    }

    #[test]
    fn retry_wait_falls_back_to_exponential_backoff() {
        let headers = HeaderMap::new();
        let d1 = ApiClient::retry_wait_from_headers(&headers, 1);
        let d2 = ApiClient::retry_wait_from_headers(&headers, 2);
        assert!(d1 >= Duration::from_secs(1) && d1 < Duration::from_millis(1250));
        assert!(d2 >= Duration::from_secs(2) && d2 < Duration::from_millis(2250));
    }

    #[tokio::test]
    async fn pagination_loop_is_detected_before_second_request() {
        let srv = start_server(|_base, _hit, path| {
            assert_eq!(path, "/loop");
            let body = br#"{"results":[{"id":1}]}"#.to_vec();
            let headers = vec![("link".to_string(), "</loop>; rel=next".to_string())];
            (200, headers, body)
        })
        .await;

        let client = test_client(&srv.base_url);
        let url = srv.url("/loop");

        let res = client.get_paginated_results_with_limit(url, true, 10).await;
        assert!(res.is_err());
        let msg = format!("{:#}", res.unwrap_err());
        assert!(msg.contains("Pagination loop detected"));
        assert_eq!(srv.hits.load(Ordering::SeqCst), 1);

        let _ = srv.shutdown.send(());
    }

    #[tokio::test]
    async fn pagination_aborts_at_max_pages_without_fetching_next_page() {
        let srv = start_server(|base, _hit, path| {
            // /pages/<n>
            let n: usize = path.trim_start_matches("/pages/").parse().unwrap_or(0);
            let next = format!("</pages/{}>; rel=next", n + 1);
            let body = format!("{{\"results\":[{{\"n\":{n}}}]}}")
                .as_bytes()
                .to_vec();
            let headers = vec![("link".to_string(), next)];

            // sanity: base is present, ensure it looks like what the client will join against
            assert!(base.starts_with("http://"));
            (200, headers, body)
        })
        .await;

        let client = test_client(&srv.base_url);
        let url = srv.url("/pages/1");

        let res = client.get_paginated_results_with_limit(url, true, 3).await;
        assert!(res.is_err());
        let msg = format!("{:#}", res.unwrap_err());
        assert!(msg.contains("Pagination aborted after 3 pages"));
        assert_eq!(srv.hits.load(Ordering::SeqCst), 3);

        let _ = srv.shutdown.send(());
    }

    #[tokio::test]
    async fn pagination_resolves_query_relative_next_against_current_url() {
        let srv = start_server(|_base, hit, path| match hit {
            1 => {
                assert_eq!(path, "/wiki/api/v2/pages?limit=1");
                (
                    200,
                    vec![("link".to_string(), "<?cursor=abc>; rel=next".to_string())],
                    br#"{"results":[{"id":"1"}]}"#.to_vec(),
                )
            }
            2 => {
                assert_eq!(path, "/wiki/api/v2/pages?cursor=abc");
                (200, vec![], br#"{"results":[{"id":"2"}]}"#.to_vec())
            }
            _ => panic!("unexpected request #{hit}: {path}"),
        })
        .await;

        let client = test_client(&srv.base_url);
        let url = srv.url("/wiki/api/v2/pages?limit=1");

        let res = client
            .get_paginated_results_with_limit(url, true, 10)
            .await
            .unwrap();

        assert_eq!(res.len(), 2);
        assert_eq!(srv.hits.load(Ordering::SeqCst), 2);

        let _ = srv.shutdown.send(());
    }

    #[tokio::test]
    async fn request_retries_on_500_then_succeeds() {
        let srv = start_server(|_base, hit, path| {
            assert_eq!(path, "/flaky");
            if hit < 3 {
                (
                    500,
                    vec![("retry-after".to_string(), "0".to_string())],
                    b"nope".to_vec(),
                )
            } else {
                (
                    200,
                    vec![("content-type".to_string(), "application/json".to_string())],
                    br#"{"ok":true}"#.to_vec(),
                )
            }
        })
        .await;

        let client = test_client(&srv.base_url);
        let url = srv.url("/flaky");

        let (json, _headers) = client.get_json(url).await.unwrap();
        assert_eq!(json.get("ok").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(srv.hits.load(Ordering::SeqCst), 3);

        let _ = srv.shutdown.send(());
    }

    #[tokio::test]
    async fn does_not_retry_on_400() {
        let srv = start_server(|_base, _hit, path| {
            assert_eq!(path, "/bad");
            (
                400,
                vec![("content-type".to_string(), "text/plain".to_string())],
                b"bad".to_vec(),
            )
        })
        .await;

        let client = test_client(&srv.base_url);
        let url = srv.url("/bad");
        let res = client.get_json(url).await;
        assert!(res.is_err());
        assert_eq!(srv.hits.load(Ordering::SeqCst), 1);

        let _ = srv.shutdown.send(());
    }
}
