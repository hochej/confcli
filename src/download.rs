use anyhow::{Context, Result};
use confcli::client::ApiClient;
use futures_util::StreamExt;
use indicatif::ProgressBar;
use reqwest::header::HeaderMap;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use url::Url;

#[derive(Debug, Clone, Copy)]
pub struct DownloadRetry {
    pub max_attempts: u32,
}

impl Default for DownloadRetry {
    fn default() -> Self {
        Self { max_attempts: 3 }
    }
}

pub async fn fetch_page_with_body_format(
    client: &ApiClient,
    page_id: &str,
    body_format: &str,
) -> Result<(Value, String)> {
    let url = client.v2_url(&format!("/pages/{page_id}?body-format={body_format}"));
    let (json, _) = client
        .get_json(url)
        .await
        .with_context(|| format!("Failed to fetch page {page_id} (body-format={body_format})"))?;
    let body = json
        .get("body")
        .and_then(|body| body.get(body_format))
        .and_then(|body| body.get("value"))
        .and_then(|value| value.as_str())
        .with_context(|| {
            format!(
                "Page {page_id} response missing body.{body_format}.value (unexpected API response shape)"
            )
        })?
        .to_string();
    Ok((json, body))
}

/// Build the full download URL for an attachment.
///
/// `base` is typically the site URL (e.g. `https://example.atlassian.net/wiki`).
/// `download` is the relative path from the API (e.g. `/download/attachments/123/file.png`).
///
/// `Url::join` treats paths starting with `/` as relative to the *origin*, which
/// would drop the `/wiki` prefix on Confluence Cloud.  We work around this by
/// prepending the base path prefix when the download link is absolute.
pub fn attachment_download_url(base: &Url, download: &str) -> Result<Url> {
    if download.starts_with("http://") || download.starts_with("https://") {
        return Url::parse(download).context("Invalid attachment download URL");
    }

    // For absolute paths (starting with /), prepend the base URL's path prefix
    // (e.g. "/wiki") so it isn't lost during resolution.
    if download.starts_with('/') {
        let prefix = base.path().trim_end_matches('/');
        if !prefix.is_empty() && !download.starts_with(prefix) {
            let prefixed = format!("{prefix}{download}");
            return base
                .join(&prefixed)
                .with_context(|| format!("Invalid attachment download link '{download}'"));
        }
    }

    base.join(download)
        .with_context(|| format!("Invalid attachment download link '{download}'"))
}

pub struct DownloadToFileOptions<'a> {
    pub retry: DownloadRetry,
    pub progress: Option<&'a ProgressBar>,
    pub verbose: u8,
    pub quiet: bool,
}

pub async fn download_to_file_with_retry(
    client: &ApiClient,
    url: Url,
    dest: &Path,
    label: &str,
    opts: DownloadToFileOptions<'_>,
) -> Result<()> {
    let mut attempt = 0u32;
    loop {
        attempt += 1;

        let tmp = tmp_path(dest);
        // Ensure we don't append to previous failed attempts.
        let _ = tokio::fs::remove_file(&tmp).await;

        let response = match client
            .apply_auth(client.http().get(url.clone()))?
            .send()
            .await
        {
            Ok(r) => r,
            Err(err) => {
                let _ = tokio::fs::remove_file(&tmp).await;
                if attempt >= opts.retry.max_attempts {
                    return Err(anyhow::Error::new(err)).with_context(|| {
                        format!(
                            "Download failed after {attempt} attempt(s): {label} -> {}",
                            dest.display()
                        )
                    });
                }
                let wait = ApiClient::retry_wait_from_headers(&HeaderMap::new(), attempt);
                if !opts.quiet {
                    eprintln!(
                        "Retrying download ({attempt}/{}) in {:?}: {label} (request error: {err})",
                        opts.retry.max_attempts, wait
                    );
                }
                tokio::time::sleep(wait).await;
                continue;
            }
        };

        let status = response.status();
        if !status.is_success() {
            let headers = response.headers().clone();
            let body = response.text().await.unwrap_or_default();
            let msg = confcli::client::friendly_error(status, &body);
            let mut err =
                anyhow::anyhow!(msg).context(format!("Download request failed for {url}"));
            if opts.verbose > 0 {
                err = err.context(format!("Response body: {body}"));
            }

            let _ = tokio::fs::remove_file(&tmp).await;
            if attempt < opts.retry.max_attempts && (status == 429 || status.is_server_error()) {
                let wait = ApiClient::retry_wait_from_headers(&headers, attempt);
                if !opts.quiet {
                    eprintln!(
                        "Retrying download ({attempt}/{}) in {:?}: {label} (status {status})",
                        opts.retry.max_attempts, wait
                    );
                }
                tokio::time::sleep(wait).await;
                continue;
            }

            return Err(err).with_context(|| {
                format!(
                    "Download failed after {attempt} attempt(s): {label} -> {}",
                    dest.display()
                )
            });
        }

        let total = response.content_length();
        if let (Some(bar), Some(total)) = (opts.progress, total)
            && bar.length().is_none()
        {
            bar.set_length(total);
        }

        let mut file = tokio::fs::File::create(&tmp)
            .await
            .with_context(|| format!("Failed to create {}", tmp.display()))?;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Download stream error")?;
            tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
            if let Some(bar) = opts.progress {
                bar.inc(chunk.len() as u64);
            }
        }

        // Atomic-ish on POSIX; on Windows rename can fail if dest exists.
        if tokio::fs::try_exists(dest).await.unwrap_or(false) {
            tokio::fs::remove_file(dest).await.ok();
        }
        tokio::fs::rename(&tmp, dest).await.with_context(|| {
            format!(
                "Failed to move downloaded file into place ({} -> {})",
                tmp.display(),
                dest.display()
            )
        })?;
        return Ok(());
    }
}

pub fn sanitize_filename(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        if ch.is_control() || ch == '/' || ch == '\\' {
            continue;
        }
        out.push(ch);
    }
    out.trim().to_string()
}

pub fn unique_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_string();
    for i in 1..10_000 {
        let name = if ext.is_empty() {
            format!("{stem} ({i})")
        } else {
            format!("{stem} ({i}).{ext}")
        };
        let candidate = parent.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    path
}

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn tmp_path(dest: &Path) -> PathBuf {
    let base = dest
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("download");
    let stamp = unique_stamp_for_tmp_filename();
    dest.with_file_name(format!("{base}.{stamp}.tmp"))
}

fn unique_stamp_for_tmp_filename() -> String {
    // Include time + pid + monotonic counter to avoid collisions under
    // concurrent downloads (and across very fast retries).
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let ns = now.as_nanos();
    let pid = std::process::id();
    let ctr = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{ns}-{pid}-{ctr}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use confcli::auth::AuthMethod;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;

    struct TestServer {
        base_url: String,
        shutdown: oneshot::Sender<()>,
        hits: Arc<AtomicUsize>,
    }

    impl TestServer {
        fn url(&self, path: &str) -> Url {
            Url::parse(&format!("{}{}", self.base_url, path)).unwrap()
        }
    }

    async fn start_server<F>(handler: F) -> TestServer
    where
        F: Fn(usize, &str) -> (u16, Vec<(String, String)>, Vec<u8>) + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{}", addr);

        let hits = Arc::new(AtomicUsize::new(0));
        let hits_task = hits.clone();

        let (tx, mut rx) = oneshot::channel::<()>();
        let handler = Arc::new(handler);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut rx => return,
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
                        let req = String::from_utf8_lossy(&buf[..n]);
                        let first = req.lines().next().unwrap_or_default();
                        let target = first.split_whitespace().nth(1).unwrap_or("/");

                        let hit = hits_task.fetch_add(1, AtomicOrdering::SeqCst) + 1;
                        let (status, headers, body) = handler(hit, target);

                        let reason = match status {
                            200 => "OK",
                            404 => "Not Found",
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
    fn download_url_prepends_wiki_prefix() {
        let base = Url::parse("https://example.atlassian.net/wiki").unwrap();
        let result =
            attachment_download_url(&base, "/download/attachments/123/file.png?version=1&api=v2")
                .unwrap();
        assert_eq!(
            result.as_str(),
            "https://example.atlassian.net/wiki/download/attachments/123/file.png?version=1&api=v2"
        );
    }

    #[test]
    fn download_url_no_prefix() {
        let base = Url::parse("https://example.com").unwrap();
        let result =
            attachment_download_url(&base, "/download/attachments/123/file.png?version=1").unwrap();
        assert_eq!(
            result.as_str(),
            "https://example.com/download/attachments/123/file.png?version=1"
        );
    }

    #[test]
    fn download_url_absolute() {
        let base = Url::parse("https://example.atlassian.net/wiki").unwrap();
        let result = attachment_download_url(&base, "https://cdn.example.com/file.png").unwrap();
        assert_eq!(result.as_str(), "https://cdn.example.com/file.png");
    }

    #[test]
    fn download_url_already_has_prefix() {
        let base = Url::parse("https://example.atlassian.net/wiki").unwrap();
        let result =
            attachment_download_url(&base, "/wiki/download/attachments/123/file.png?version=1")
                .unwrap();
        assert_eq!(
            result.as_str(),
            "https://example.atlassian.net/wiki/download/attachments/123/file.png?version=1"
        );
    }

    #[tokio::test]
    async fn download_retries_on_500_then_succeeds() {
        let srv = start_server(|hit, target| {
            assert_eq!(target, "/file");
            if hit == 1 {
                (
                    500,
                    vec![("retry-after".to_string(), "0".to_string())],
                    b"nope".to_vec(),
                )
            } else {
                (
                    200,
                    vec![(
                        "content-type".to_string(),
                        "application/octet-stream".to_string(),
                    )],
                    b"hello".to_vec(),
                )
            }
        })
        .await;

        let client = test_client(&srv.base_url);
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("out.bin");
        let url = srv.url("/file");

        download_to_file_with_retry(
            &client,
            url,
            &dest,
            "test",
            DownloadToFileOptions {
                retry: DownloadRetry { max_attempts: 3 },
                progress: None,
                verbose: 0,
                quiet: true,
            },
        )
        .await
        .unwrap();
        let bytes = std::fs::read(dir.path().join("out.bin")).unwrap();
        assert_eq!(bytes, b"hello");
        assert_eq!(srv.hits.load(AtomicOrdering::SeqCst), 2);

        let _ = srv.shutdown.send(());
    }

    #[tokio::test]
    async fn download_does_not_retry_on_404() {
        let srv = start_server(|_hit, target| {
            assert_eq!(target, "/missing");
            (
                404,
                vec![("content-type".to_string(), "text/plain".to_string())],
                b"nope".to_vec(),
            )
        })
        .await;

        let client = test_client(&srv.base_url);
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("out.bin");
        let url = srv.url("/missing");

        let res = download_to_file_with_retry(
            &client,
            url,
            &dest,
            "test",
            DownloadToFileOptions {
                retry: DownloadRetry { max_attempts: 3 },
                progress: None,
                verbose: 0,
                quiet: true,
            },
        )
        .await;

        assert!(res.is_err());
        assert!(!dest.exists());
        assert_eq!(srv.hits.load(AtomicOrdering::SeqCst), 1);

        let _ = srv.shutdown.send(());
    }
}
