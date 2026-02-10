#[cfg(feature = "write")]
use anyhow::anyhow;
use anyhow::{Context, Result};
use confcli::client::ApiClient;
use confcli::json_util::json_str;
use confcli::output::OutputFormat;
#[cfg(feature = "write")]
use dialoguer::Confirm;
use indicatif::{ProgressBar, ProgressStyle};
#[cfg(feature = "write")]
use serde_json::json;
#[cfg(feature = "write")]
use std::sync::Arc;
#[cfg(feature = "write")]
use tokio::sync::Semaphore;
#[cfg(feature = "write")]
use tokio::task::JoinSet;
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
        fmt => {
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
            maybe_print_rows(ctx, fmt, &["ID", "Title", "Type", "Size"], rows);
            Ok(())
        }
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
        fmt => {
            let rows = vec![
                vec!["ID".to_string(), json_str(&json, "id")],
                vec!["Title".to_string(), json_str(&json, "title")],
                vec!["Type".to_string(), json_str(&json, "mediaType")],
                vec![
                    "Size".to_string(),
                    human_size(json.get("fileSize").and_then(|v| v.as_i64()).unwrap_or(0)),
                ],
            ];
            maybe_print_kv_fmt(ctx, fmt, rows);
            Ok(())
        }
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
    let full_url = crate::download::attachment_download_url(&base, download)?;
    let file_name = resolve_download_path(&args.dest, &json)?;

    let progress = if ctx.quiet {
        None
    } else {
        let bar = ProgressBar::new_spinner();
        bar.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} {bytes}/{total_bytes} {bar:40.cyan/blue} {eta}",
            )
            .unwrap(),
        );
        bar.enable_steady_tick(std::time::Duration::from_millis(120));
        Some(bar)
    };

    crate::download::download_to_file_with_retry(
        client,
        full_url,
        &file_name,
        &format!("attachment {}", args.attachment),
        crate::download::DownloadToFileOptions {
            retry: crate::download::DownloadRetry::default(),
            progress: progress.as_ref(),
            verbose: ctx.verbose,
            quiet: ctx.quiet,
        },
    )
    .await?;

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
        let names: Vec<_> = args.files.iter().map(|f| f.display().to_string()).collect();
        print_line(
            ctx,
            &format!("Would upload {} to page {page_id}", names.join(", ")),
        );
        return Ok(());
    }

    let mut approved_files = Vec::new();
    for file in &args.files {
        let metadata = tokio::fs::metadata(file).await?;
        let size = metadata.len();
        if size > 5 * 1024 * 1024 {
            let confirm = Confirm::new()
                .with_prompt(format!(
                    "Upload {} ({:.2} MB)?",
                    file.display(),
                    size as f64 / 1_048_576.0
                ))
                .default(false)
                .interact()?;
            if !confirm {
                print_line(ctx, &format!("Skipped {}.", file.display()));
                continue;
            }
        }
        approved_files.push(file.clone());
    }

    if approved_files.is_empty() {
        return Ok(());
    }

    let comment = args.comment.clone();
    let sem = Arc::new(Semaphore::new(args.concurrency.max(1)));
    let client = Arc::new(client.clone());
    let mut tasks = JoinSet::new();

    for (idx, file) in approved_files.into_iter().enumerate() {
        let permit = sem.clone().acquire_owned().await?;
        let client = client.clone();
        let page_id = page_id.clone();
        let comment = comment.clone();

        tasks.spawn(async move {
            let _permit = permit;
            let result = client.upload_attachment(&page_id, &file, comment).await?;
            let attachment = result
                .get("results")
                .and_then(|v| v.as_array())
                .and_then(|items| items.first())
                .cloned()
                .unwrap_or(result);
            Ok::<_, anyhow::Error>((idx, attachment))
        });
    }

    let mut ordered_results = Vec::new();
    while let Some(res) = tasks.join_next().await {
        match res {
            Ok(Ok((idx, attachment))) => ordered_results.push((idx, attachment)),
            Ok(Err(err)) => {
                tasks.abort_all();
                while tasks.join_next().await.is_some() {}
                return Err(err.context("Attachment upload failed"));
            }
            Err(join_err) => {
                tasks.abort_all();
                while tasks.join_next().await.is_some() {}
                return Err(anyhow!("Attachment upload task failed: {join_err}"));
            }
        }
    }

    ordered_results.sort_by_key(|(idx, _)| *idx);
    let all_attachments: Vec<_> = ordered_results.into_iter().map(|(_, a)| a).collect();

    match args.output {
        OutputFormat::Json => maybe_print_json(ctx, &all_attachments)?,
        _ => {
            for attachment in &all_attachments {
                let rows = vec![
                    vec!["ID".to_string(), json_str(attachment, "id")],
                    vec!["Title".to_string(), json_str(attachment, "title")],
                ];
                maybe_print_kv(ctx, rows);
            }
        }
    }

    Ok(())
}

#[cfg(feature = "write")]
async fn attachment_delete(
    client: &ApiClient,
    ctx: &AppContext,
    args: AttachmentDeleteArgs,
) -> Result<()> {
    let action = if args.purge { "purge" } else { "delete" };

    if ctx.dry_run {
        if let Some(fmt) = args.output {
            match fmt {
                OutputFormat::Json => {
                    return maybe_print_json(
                        ctx,
                        &json!({
                            "dryRun": true,
                            "action": action,
                            "deleted": false,
                            "id": args.attachment,
                        }),
                    );
                }
                other => {
                    maybe_print_kv_fmt(
                        ctx,
                        other,
                        vec![
                            vec!["DryRun".to_string(), "true".to_string()],
                            vec!["Action".to_string(), action.to_string()],
                            vec!["Deleted".to_string(), "false".to_string()],
                            vec!["ID".to_string(), args.attachment],
                        ],
                    );
                    return Ok(());
                }
            }
        }

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

    if let Some(fmt) = args.output {
        match fmt {
            OutputFormat::Json => maybe_print_json(
                ctx,
                &json!({
                    "action": action,
                    "deleted": true,
                    "id": args.attachment,
                }),
            ),
            other => {
                maybe_print_kv_fmt(
                    ctx,
                    other,
                    vec![
                        vec!["Action".to_string(), action.to_string()],
                        vec!["Deleted".to_string(), "true".to_string()],
                        vec!["ID".to_string(), args.attachment],
                    ],
                );
                Ok(())
            }
        }
    } else {
        let past = if args.purge { "Purged" } else { "Deleted" };
        print_line(ctx, &format!("{past} attachment {}", args.attachment));
        Ok(())
    }
}
