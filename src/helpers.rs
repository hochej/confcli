#[cfg(feature = "write")]
use anyhow::Context;
use anyhow::Result;
use confcli::output::{print_json, print_table};
use humansize::{format_size, BINARY};
use serde_json::Value;
use std::path::{Path, PathBuf};
use url::Url;

use crate::context::AppContext;

pub fn maybe_print_json<T: serde::Serialize>(ctx: &AppContext, value: &T) -> Result<()> {
    if ctx.quiet {
        return Ok(());
    }
    print_json(value)
}

pub fn maybe_print_table(ctx: &AppContext, headers: &[&str], rows: Vec<Vec<String>>) {
    if ctx.quiet {
        return;
    }
    print_table(headers, rows);
}

pub fn print_line(ctx: &AppContext, message: &str) {
    if ctx.quiet {
        return;
    }
    println!("{message}");
}

pub fn human_size(bytes: i64) -> String {
    if bytes < 0 {
        return bytes.to_string();
    }
    format_size(bytes as u64, BINARY)
}

pub fn resolve_download_path(output: &Option<PathBuf>, json: &Value) -> Result<PathBuf> {
    if let Some(path) = output {
        return Ok(path.clone());
    }
    let title = json.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let file_name = Path::new(title)
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("");
    if file_name.is_empty() {
        return Err(anyhow::anyhow!(
            "Unsafe or missing attachment title. Provide --output to choose a file path."
        ));
    }
    Ok(PathBuf::from(file_name))
}

pub fn add_markdown_header(base_url: &str, json: &Value, markdown: &str) -> String {
    let webui = json
        .get("_links")
        .and_then(|v| v.get("webui"))
        .and_then(|v| v.as_str());
    if let Some(webui) = webui {
        let source = format!("{base_url}{webui}");
        format!("<!-- Source: {source} -->\n\n{markdown}")
    } else {
        markdown.to_string()
    }
}

pub fn markdown_not_supported() -> Result<()> {
    Err(anyhow::anyhow!(
        "Markdown output is only supported for page get/body"
    ))
}

pub fn url_with_query(base: &str, pairs: &[(&str, String)]) -> Result<String> {
    let mut url = Url::parse(base).map_err(|err| anyhow::anyhow!("Invalid URL '{base}': {err}"))?;
    {
        let mut qp = url.query_pairs_mut();
        for (key, value) in pairs {
            qp.append_pair(key, value);
        }
    }
    Ok(url.to_string())
}

#[cfg(feature = "write")]
pub async fn read_body(body: Option<String>, body_file: Option<&PathBuf>) -> Result<String> {
    if body.is_some() && body_file.is_some() {
        return Err(anyhow::anyhow!(
            "Use either --body or --body-file, not both"
        ));
    }
    if let Some(body) = body {
        return Ok(body);
    }
    if let Some(path) = body_file {
        if path == &PathBuf::from("-") {
            let mut input = String::new();
            let mut stdin = tokio::io::stdin();
            use tokio::io::AsyncReadExt;
            stdin.read_to_string(&mut input).await?;
            return Ok(input);
        }
        return tokio::fs::read_to_string(path)
            .await
            .with_context(|| format!("Failed to read {}", path.display()));
    }
    Err(anyhow::anyhow!(
        "Provide --body or --body-file (use '-' for stdin)"
    ))
}

#[cfg(feature = "write")]
pub fn derive_title_from_file(body_file: Option<&PathBuf>) -> Option<String> {
    let path = body_file?;
    if path == &PathBuf::from("-") {
        return None;
    }
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

pub fn open_url(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", url])
            .spawn()?;
    }
    Ok(())
}
