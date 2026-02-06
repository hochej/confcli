use anyhow::{Context, Result};
use confcli::client::ApiClient;
use confcli::json_util::json_str;
use confcli::markdown::{MarkdownOptions, html_to_markdown_with_options};
use confcli::output::OutputFormat;
use regex::Regex;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Semaphore;
use url::Url;

use crate::cli::ExportArgs;
use crate::context::AppContext;
use crate::download::{
    DownloadRetry, DownloadToFileOptions, attachment_download_url, download_to_file_with_retry,
    fetch_page_with_body_format, sanitize_filename, unique_path,
};
use crate::helpers::*;
use crate::resolve::{resolve_page_id, resolve_space_key};

pub async fn handle(ctx: &AppContext, args: ExportArgs) -> Result<()> {
    let client = crate::context::load_client(ctx)?;
    export_page(&client, ctx, args).await
}

async fn export_page(client: &ApiClient, ctx: &AppContext, args: ExportArgs) -> Result<()> {
    let page_id = resolve_page_id(client, &args.page).await?;
    let format = args.format.to_lowercase();

    let (page_json, body_bytes, content_file) = match format.as_str() {
        "md" | "markdown" => {
            let (json, html) = fetch_page_with_body_format(client, &page_id, "view").await?;
            let markdown = html_to_markdown_with_options(
                &html,
                client.base_url(),
                MarkdownOptions {
                    keep_empty_list_items: false,
                },
            )?;
            (json, markdown.into_bytes(), PathBuf::from("page.md"))
        }
        "storage" => {
            let (json, body) = fetch_page_with_body_format(client, &page_id, "storage").await?;
            let bytes = body.into_bytes();
            (json, bytes, PathBuf::from("page.storage.html"))
        }
        "adf" | "atlas_doc_format" => {
            let (json, body) =
                fetch_page_with_body_format(client, &page_id, "atlas_doc_format").await?;
            let pretty = match serde_json::from_str::<serde_json::Value>(&body) {
                Ok(value) => serde_json::to_vec_pretty(&value)?,
                Err(_) => body.into_bytes(),
            };
            (json, pretty, PathBuf::from("page.adf.json"))
        }
        _ => {
            return Err(anyhow::anyhow!(
                "Invalid --format: {}. Use md, storage, or adf.",
                args.format
            ));
        }
    };

    let title = json_str(&page_json, "title");
    let folder_name = format!("{}--{}", sanitize_filename(&title), page_id);
    let out_dir = args.dest.join(folder_name);
    tokio::fs::create_dir_all(&out_dir).await?;

    // Write metadata + content.
    let meta_path = out_dir.join("meta.json");
    let space_id = json_str(&page_json, "spaceId");
    let space_key = if !space_id.is_empty() {
        resolve_space_key(client, &space_id)
            .await
            .unwrap_or_default()
    } else {
        String::new()
    };
    let meta = json!({
        "id": page_id,
        "title": title,
        "spaceId": space_id,
        "spaceKey": space_key,
        "siteUrl": client.base_url(),
    });
    tokio::fs::write(&meta_path, serde_json::to_vec_pretty(&meta)?).await?;

    let content_path = out_dir.join(content_file);
    tokio::fs::write(&content_path, body_bytes).await?;

    let mut attachments_written = Vec::<PathBuf>::new();
    if !args.skip_attachments {
        let attachments_dir = out_dir.join("attachments");
        tokio::fs::create_dir_all(&attachments_dir).await?;

        let url = client.v2_url(&format!("/pages/{page_id}/attachments?limit=50"));
        let items = client.get_paginated_results(url, true).await?;

        let matcher = args.pattern.as_deref().map(glob_to_regex).transpose()?;

        let selected: Vec<serde_json::Value> = items
            .into_iter()
            .filter(|item| {
                if let Some(re) = &matcher {
                    let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("");
                    re.is_match(title)
                } else {
                    true
                }
            })
            .collect();

        let sem = Arc::new(Semaphore::new(args.concurrency.max(1)));
        let client = Arc::new(client.clone());
        let origin = Url::parse(client.base_url())?;
        let quiet = ctx.quiet;

        let total_bar = if ctx.quiet {
            None
        } else {
            let bar = indicatif::ProgressBar::new(selected.len() as u64);
            bar.set_style(
                indicatif::ProgressStyle::with_template("{spinner:.green} {pos}/{len} {wide_msg}")
                    .unwrap(),
            );
            bar.set_message("attachments");
            Some(bar)
        };

        let verbose = ctx.verbose;
        let mut tasks = Vec::new();
        for item in selected {
            let permit = sem.clone().acquire_owned().await?;
            let client = client.clone();
            let origin = origin.clone();
            let attachments_dir = attachments_dir.clone();
            let bar = total_bar.clone();
            tasks.push(tokio::spawn(async move {
                let _permit = permit;
                let path = download_attachment_item(
                    &client,
                    &origin,
                    &attachments_dir,
                    &item,
                    verbose,
                    quiet,
                )
                .await?;
                if let Some(bar) = &bar {
                    bar.inc(1);
                }
                Ok::<_, anyhow::Error>(path)
            }));
        }

        for task in tasks {
            let path = task.await.context("Attachment download task failed")??;
            attachments_written.push(path);
        }

        if let Some(bar) = total_bar {
            bar.finish_and_clear();
        }
    }

    match args.output {
        OutputFormat::Json => {
            let out = json!({
                "dir": out_dir,
                "meta": meta_path,
                "content": content_path,
                "attachments": attachments_written,
            });
            maybe_print_json(ctx, &out)
        }
        fmt => {
            let rows = vec![
                vec!["Dir".to_string(), out_dir.display().to_string()],
                vec!["Content".to_string(), content_path.display().to_string()],
                vec![
                    "Attachments".to_string(),
                    attachments_written.len().to_string(),
                ],
            ];
            maybe_print_kv_fmt(ctx, fmt, rows);
            Ok(())
        }
    }
}

async fn download_attachment_item(
    client: &ApiClient,
    origin: &Url,
    attachments_dir: &std::path::Path,
    item: &serde_json::Value,
    verbose: u8,
    quiet: bool,
) -> Result<PathBuf> {
    let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let download = item
        .get("downloadLink")
        .and_then(|v| v.as_str())
        .or_else(|| {
            item.get("_links")
                .and_then(|v| v.get("download"))
                .and_then(|v| v.as_str())
        })
        .context("Missing attachment download link")?;

    let target_name = sanitize_filename(title);
    if target_name.is_empty() {
        return Err(anyhow::anyhow!("Unsafe attachment title: {title}"));
    }
    let target_path = unique_path(attachments_dir.join(target_name));

    let url = attachment_download_url(origin, download)?;
    let opts = DownloadToFileOptions {
        retry: DownloadRetry::default(),
        progress: None,
        verbose,
        quiet,
    };
    download_to_file_with_retry(client, url, &target_path, title, opts).await?;

    Ok(target_path)
}

fn glob_to_regex(glob: &str) -> Result<Regex> {
    let mut re = String::from("^");
    for ch in glob.chars() {
        match ch {
            '*' => re.push_str(".*"),
            '?' => re.push('.'),
            '.' | '+' | '(' | ')' | '|' | '^' | '$' | '{' | '}' | '[' | ']' | '\\' => {
                re.push('\\');
                re.push(ch);
            }
            _ => re.push(ch),
        }
    }
    re.push('$');
    regex::RegexBuilder::new(&re)
        .case_insensitive(true)
        .build()
        .map_err(|e| anyhow::anyhow!("Invalid glob pattern: {e}"))
}
