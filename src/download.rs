use anyhow::{Context, Result};
use confcli::client::ApiClient;
use futures_util::StreamExt;
use http::HeaderMap;
use indicatif::ProgressBar;
use serde_json::Value;
use std::path::{Path, PathBuf};
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
        .unwrap_or("")
        .to_string();
    Ok((json, body))
}

pub fn attachment_download_url(origin: &Url, download: &str) -> Result<Url> {
    if download.starts_with("http://") || download.starts_with("https://") {
        return Url::parse(download).context("Invalid attachment download URL");
    }
    origin
        .join(download)
        .with_context(|| format!("Invalid attachment download link '{download}'"))
}

pub async fn download_to_file_with_retry(
    client: &ApiClient,
    url: Url,
    dest: &Path,
    label: &str,
    retry: DownloadRetry,
    progress: Option<&ProgressBar>,
    quiet: bool,
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
                if attempt >= retry.max_attempts {
                    return Err(anyhow::Error::new(err)).with_context(|| {
                        format!(
                            "Download failed after {attempt} attempt(s): {label} -> {}",
                            dest.display()
                        )
                    });
                }
                let wait = ApiClient::retry_wait_from_headers(&HeaderMap::new(), attempt);
                if !quiet {
                    eprintln!(
                        "Retrying download ({attempt}/{}) in {:?}: {label} (request error: {err})",
                        retry.max_attempts, wait
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
            let err = anyhow::anyhow!("Request failed: {status} {body}")
                .context(format!("Download request failed for {url}"));

            let _ = tokio::fs::remove_file(&tmp).await;
            if attempt < retry.max_attempts && (status == 429 || status.is_server_error()) {
                let wait = ApiClient::retry_wait_from_headers(&headers, attempt);
                if !quiet {
                    eprintln!(
                        "Retrying download ({attempt}/{}) in {:?}: {label} (status {status})",
                        retry.max_attempts, wait
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
        if let (Some(bar), Some(total)) = (progress, total)
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
            if let Some(bar) = progress {
                bar.inc(chunk.len() as u64);
            }
        }

        // Atomic-ish on POSIX; on Windows rename can fail if dest exists.
        if dest.exists() {
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

fn tmp_path(dest: &Path) -> PathBuf {
    let base = dest
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("download");
    let stamp = chronoish_now_for_filename();
    dest.with_file_name(format!("{base}.{stamp}.tmp"))
}

fn chronoish_now_for_filename() -> String {
    // Small, dependency-free timestamp for tmp filenames.
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("{ms}")
}
