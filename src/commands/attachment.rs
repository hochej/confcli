use anyhow::{Context, Result};
use confcli::client::ApiClient;
use confcli::json_util::json_str;
use confcli::output::OutputFormat;
#[cfg(feature = "write")]
use dialoguer::Confirm;
use indicatif::{ProgressBar, ProgressStyle};
use url::Url;

use crate::cli::*;
use crate::context::AppContext;
use crate::helpers::*;
use crate::resolve::resolve_page_id;

pub async fn handle(ctx: &AppContext, cmd: AttachmentCommand) -> Result<()> {
    let client = crate::context::load_client(ctx)?;
    match cmd {
        AttachmentCommand::List(args) => attachment_list(&client, ctx, args).await,
        AttachmentCommand::Get(args) => attachment_get(&client, ctx, args).await,
        AttachmentCommand::Download(args) => attachment_download(&client, ctx, args).await,
        #[cfg(feature = "write")]
        AttachmentCommand::Upload(args) => attachment_upload(&client, ctx, args).await,
        #[cfg(feature = "write")]
        AttachmentCommand::Delete(args) => attachment_delete(&client, ctx, args).await,
    }
}

async fn attachment_list(
    client: &ApiClient,
    ctx: &AppContext,
    args: AttachmentListArgs,
) -> Result<()> {
    let url = if let Some(page) = args.page {
        let page_id = resolve_page_id(client, &page).await?;
        client.v2_url(&format!(
            "/pages/{page_id}/attachments?limit={}",
            args.limit
        ))
    } else {
        client.v2_url(&format!("/attachments?limit={}", args.limit))
    };
    let items = client.get_paginated_results(url, args.all).await?;
    match args.output {
        OutputFormat::Json => maybe_print_json(ctx, &items),
        OutputFormat::Table => {
            let rows = items
                .iter()
                .map(|item| {
                    vec![
                        json_str(item, "id"),
                        json_str(item, "title"),
                        json_str(item, "mediaType"),
                        human_size(item.get("fileSize").and_then(|v| v.as_i64()).unwrap_or(0)),
                    ]
                })
                .collect();
            maybe_print_table(ctx, &["ID", "Title", "Type", "Size"], rows);
            Ok(())
        }
        OutputFormat::Markdown => markdown_not_supported(),
    }
}

async fn attachment_get(
    client: &ApiClient,
    ctx: &AppContext,
    args: AttachmentGetArgs,
) -> Result<()> {
    let url = client.v2_url(&format!("/attachments/{}", args.attachment));
    let (json, _) = client.get_json(url).await?;
    match args.output {
        OutputFormat::Json => maybe_print_json(ctx, &json),
        OutputFormat::Table => {
            let rows = vec![
                vec!["ID".to_string(), json_str(&json, "id")],
                vec!["Title".to_string(), json_str(&json, "title")],
                vec!["Type".to_string(), json_str(&json, "mediaType")],
                vec![
                    "Size".to_string(),
                    human_size(json.get("fileSize").and_then(|v| v.as_i64()).unwrap_or(0)),
                ],
            ];
            maybe_print_table(ctx, &["Field", "Value"], rows);
            Ok(())
        }
        OutputFormat::Markdown => markdown_not_supported(),
    }
}

async fn attachment_download(
    client: &ApiClient,
    ctx: &AppContext,
    args: AttachmentDownloadArgs,
) -> Result<()> {
    let url = client.v2_url(&format!("/attachments/{}", args.attachment));
    let (json, _) = client.get_json(url).await?;
    let download = json
        .get("downloadLink")
        .and_then(|v| v.as_str())
        .or_else(|| {
            json.get("_links")
                .and_then(|v| v.get("download"))
                .and_then(|v| v.as_str())
        })
        .context("Missing download link")?;
    let base = Url::parse(client.base_url())?;
    let full_url = if download.starts_with("http") {
        Url::parse(download)?
    } else {
        base.join(download)?
    };
    let response = client
        .apply_auth(client.http().get(full_url))?
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("Download failed: {}", response.status()));
    }
    let total = response.content_length();
    let file_name = resolve_download_path(&args.output, &json)?;
    let mut file = tokio::fs::File::create(&file_name).await?;
    let mut stream = response.bytes_stream();

    let progress = if ctx.quiet {
        None
    } else if let Some(total) = total {
        let bar = ProgressBar::new(total);
        bar.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} {bytes}/{total_bytes} {bar:40.cyan/blue} {eta}",
            )
            .unwrap(),
        );
        Some(bar)
    } else {
        let bar = ProgressBar::new_spinner();
        bar.set_style(ProgressStyle::with_template("{spinner:.green} {bytes} {elapsed}").unwrap());
        bar.enable_steady_tick(std::time::Duration::from_millis(120));
        Some(bar)
    };

    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
        if let Some(bar) = &progress {
            bar.inc(chunk.len() as u64);
        }
    }
    if let Some(bar) = progress {
        bar.finish_and_clear();
    }
    print_line(ctx, &format!("Downloaded to {}", file_name.display()));
    Ok(())
}

#[cfg(feature = "write")]
async fn attachment_upload(
    client: &ApiClient,
    ctx: &AppContext,
    args: AttachmentUploadArgs,
) -> Result<()> {
    let page_id = resolve_page_id(client, &args.page).await?;

    if ctx.dry_run {
        print_line(
            ctx,
            &format!("Would upload {} to page {page_id}", args.file.display()),
        );
        return Ok(());
    }

    let metadata = tokio::fs::metadata(&args.file).await?;
    let size = metadata.len();
    if size > 5 * 1024 * 1024 {
        let confirm = Confirm::new()
            .with_prompt(format!(
                "Upload {:.2} MB attachment?",
                size as f64 / 1_048_576.0
            ))
            .default(false)
            .interact()?;
        if !confirm {
            print_line(ctx, "Cancelled.");
            return Ok(());
        }
    }

    let result = client
        .upload_attachment(&page_id, &args.file, args.comment)
        .await?;
    let attachment = result
        .get("results")
        .and_then(|v| v.as_array())
        .and_then(|items| items.first())
        .cloned()
        .unwrap_or(result);
    match args.output {
        OutputFormat::Json => maybe_print_json(ctx, &attachment),
        OutputFormat::Table => {
            let rows = vec![
                vec!["ID".to_string(), json_str(&attachment, "id")],
                vec!["Title".to_string(), json_str(&attachment, "title")],
            ];
            maybe_print_table(ctx, &["Field", "Value"], rows);
            Ok(())
        }
        OutputFormat::Markdown => markdown_not_supported(),
    }
}

#[cfg(feature = "write")]
async fn attachment_delete(
    client: &ApiClient,
    ctx: &AppContext,
    args: AttachmentDeleteArgs,
) -> Result<()> {
    if ctx.dry_run {
        let action = if args.purge { "purge" } else { "delete" };
        print_line(
            ctx,
            &format!("Would {action} attachment {}", args.attachment),
        );
        return Ok(());
    }

    if !args.yes {
        let confirm = Confirm::new()
            .with_prompt(format!("Delete attachment {}?", args.attachment))
            .default(false)
            .interact()
            .map_err(|err| {
                anyhow::anyhow!("{err}. Use --yes to skip confirmation in non-interactive shells.")
            })?;
        if !confirm {
            print_line(ctx, "Cancelled.");
            return Ok(());
        }
    }
    let mut url = client.v2_url(&format!("/attachments/{}", args.attachment));
    if args.purge {
        url.push_str("?purge=true");
    }
    client.delete(url).await?;
    print_line(ctx, &format!("Deleted attachment {}", args.attachment));
    Ok(())
}
