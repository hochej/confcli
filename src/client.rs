use crate::auth::AuthMethod;
use crate::pagination::{next_link_from_body, next_link_from_headers};
#[cfg(feature = "write")]
use anyhow::Context;
use anyhow::{bail, Result};
use base64::Engine;
use http::HeaderMap;
#[cfg(feature = "write")]
use reqwest::{multipart, Body};
use reqwest::{Client as HttpClient, Method, Response};
use serde_json::Value;
#[cfg(feature = "write")]
use std::path::Path;
use std::time::Duration;
#[cfg(feature = "write")]
use tokio_util::io::ReaderStream;

const MAX_ATTEMPTS: u32 = 3;
const USER_AGENT: &str = concat!("confcli/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Clone)]
pub struct ApiClient {
    base_url: String,
    auth: AuthMethod,
    http: HttpClient,
    verbose: u8,
}

impl ApiClient {
    pub fn new(base_url: String, auth: AuthMethod, verbose: u8) -> Result<Self> {
        let base_url = base_url.trim_end_matches('/').to_string();
        let http = HttpClient::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .build()?;
        Ok(Self {
            base_url,
            auth,
            http,
            verbose,
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn http(&self) -> &HttpClient {
        &self.http
    }

    pub fn v2_url(&self, path: &str) -> String {
        format!("{}/api/v2{}", self.base_url, path)
    }

    pub fn v1_url(&self, path: &str) -> String {
        format!("{}/rest/api{}", self.base_url, path)
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
    fn retry_wait(headers: &HeaderMap, attempt: u32) -> Duration {
        if let Some(val) = headers.get("retry-after") {
            if let Ok(s) = val.to_str() {
                if let Ok(secs) = s.trim().parse::<u64>() {
                    return Duration::from_secs(secs);
                }
            }
        }
        Duration::from_secs(2u64.pow(attempt - 1))
    }

    async fn send(&self, method: Method, url: String) -> Result<Response> {
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
            let builder = self.http.request(method.clone(), url.clone());
            let builder = self.apply_auth(builder)?;

            match builder.send().await {
                Ok(response) => {
                    if self.verbose > 1 {
                        eprintln!("<- {} ({:?})", response.status(), start.elapsed());
                    }

                    if response.status().is_success() {
                        return Ok(response);
                    }

                    let status = response.status();
                    if attempts < MAX_ATTEMPTS && (status == 429 || status.is_server_error()) {
                        attempts += 1;
                        let wait = Self::retry_wait(response.headers(), attempts);
                        if self.verbose > 0 {
                            eprintln!("Received {}, retrying in {:?}...", status, wait);
                        }
                        tokio::time::sleep(wait).await;
                        continue;
                    }

                    let body = response.text().await.unwrap_or_default();
                    bail!("Request failed: {status} {body}");
                }
                Err(e) => {
                    if attempts < MAX_ATTEMPTS {
                        attempts += 1;
                        let wait = Duration::from_secs(2u64.pow(attempts - 1));
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

    /// Send a request with a JSON body, using the same retry logic as `send()`.
    #[cfg(feature = "write")]
    async fn send_with_json_body(
        &self,
        method: Method,
        url: String,
        body: &Value,
    ) -> Result<Response> {
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
            let builder = self.http.request(method.clone(), url.clone()).json(body);
            let builder = self.apply_auth(builder)?;

            match builder.send().await {
                Ok(response) => {
                    if self.verbose > 1 {
                        eprintln!("<- {} ({:?})", response.status(), start.elapsed());
                    }

                    if response.status().is_success() {
                        return Ok(response);
                    }

                    let status = response.status();
                    if attempts < MAX_ATTEMPTS && (status == 429 || status.is_server_error()) {
                        attempts += 1;
                        let wait = Self::retry_wait(response.headers(), attempts);
                        if self.verbose > 0 {
                            eprintln!("Received {}, retrying in {:?}...", status, wait);
                        }
                        tokio::time::sleep(wait).await;
                        continue;
                    }

                    let body = response.text().await.unwrap_or_default();
                    bail!("Request failed: {status} {body}");
                }
                Err(e) => {
                    if attempts < MAX_ATTEMPTS {
                        attempts += 1;
                        let wait = Duration::from_secs(2u64.pow(attempts - 1));
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

    pub async fn get_json(&self, url: String) -> Result<(Value, HeaderMap)> {
        let response = self.send(Method::GET, url).await?;
        let headers = response.headers().clone();
        let json = response.json::<Value>().await?;
        Ok((json, headers))
    }

    pub async fn get_paginated_results(&self, url: String, all: bool) -> Result<Vec<Value>> {
        let mut results = Vec::new();
        let mut next_url: Option<String> = Some(url);
        while let Some(url) = next_url {
            let (json, headers) = self.get_json(url).await?;
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
            next_url = next_link_from_headers(&headers).or_else(|| next_link_from_body(&json));
            if let Some(next) = &next_url {
                if next.starts_with("http") {
                    continue;
                }
                next_url = Some(format!("{}{}", self.base_url, next));
            }
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
    /// No retry logic: the stream body is consumed on the first attempt and
    /// cannot be replayed.
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
            .context("Invalid file name")?;
        let file = tokio::fs::File::open(file_path).await?;
        let metadata = file.metadata().await?;
        let size = metadata.len();
        let stream = ReaderStream::new(file);
        let body = Body::wrap_stream(stream);
        let part = multipart::Part::stream_with_length(body, size).file_name(file_name.to_string());
        let mut form = multipart::Form::new().part("file", part);
        if let Some(comment) = comment {
            form = form.text("comment", comment);
        }
        let builder = self
            .http
            .post(url)
            .multipart(form)
            .header("X-Atlassian-Token", "no-check");
        let builder = self.apply_auth(builder)?;
        let response = builder.send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("Upload failed: {status} {body}");
        }
        Ok(response.json::<Value>().await?)
    }
}
