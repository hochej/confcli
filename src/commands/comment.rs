use anyhow::{Context, Result};
use confcli::client::ApiClient;
use confcli::json_util::json_str;
use confcli::markdown::markdown_to_storage;
use confcli::output::OutputFormat;
#[cfg(feature = "write")]
use dialoguer::Confirm;
#[cfg(feature = "write")]
use serde_json::{Value, json};

use crate::cli::*;
use crate::context::AppContext;
use crate::helpers::*;
use crate::resolve::resolve_page_id;

pub async fn handle(ctx: &AppContext, cmd: CommentCommand) -> Result<()> {
    let client = crate::context::load_client(ctx)?;
    match cmd {
        CommentCommand::List(args) => comment_list(&client, ctx, args).await,
        #[cfg(feature = "write")]
        CommentCommand::Add(args) => comment_add(&client, ctx, args).await,
        #[cfg(feature = "write")]
        CommentCommand::Delete(args) => comment_delete(&client, ctx, args).await,
    }
}

async fn comment_list(client: &ApiClient, ctx: &AppContext, args: CommentListArgs) -> Result<()> {
    let page_id = resolve_page_id(client, &args.page).await?;
    // Keep expansions minimal for list output; allow opting into heavier expansions.
    // The default is intentionally small to keep payload sizes reasonable.
    let expand = args
        .expand
        .unwrap_or_else(|| "history,extensions,ancestors".to_string());

    let mut pairs = vec![("limit", args.limit.to_string()), ("expand", expand)];
    if let Some(location) = args.location {
        for value in location
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            pairs.push(("location", value.to_string()));
        }
    }

    // Use the descendant endpoint to fetch top-level comments and replies without N+1 requests.
    let url = url_with_query(
        &client.v1_url(&format!("/content/{page_id}/descendant/comment")),
        &pairs,
    )?;
    let all_items = client.get_paginated_results(url, args.all).await?;

    match args.output {
        OutputFormat::Json => maybe_print_json(ctx, &all_items),
        fmt => {
            let rows = all_items
                .iter()
                .map(|item| {
                    let created = item
                        .get("history")
                        .and_then(|v| v.get("createdDate"))
                        .and_then(|v| v.as_str())
                        .map(format_timestamp)
                        .unwrap_or_default();
                    let author = item
                        .get("history")
                        .and_then(|v| v.get("createdBy"))
                        .and_then(|v| v.get("displayName"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    vec![
                        json_str(item, "id"),
                        comment_location(item),
                        author.to_string(),
                        created,
                        comment_parent_id(item).unwrap_or_default(),
                    ]
                })
                .collect();
            maybe_print_rows(
                ctx,
                fmt,
                &["ID", "Location", "Author", "Created", "Parent"],
                rows,
            );
            Ok(())
        }
    }
}

#[cfg(feature = "write")]
async fn comment_add(client: &ApiClient, ctx: &AppContext, args: CommentAddArgs) -> Result<()> {
    let page_id = resolve_page_id(client, &args.page).await?;

    if ctx.dry_run {
        print_line(ctx, &format!("Would add comment on page {page_id}"));
        return Ok(());
    }

    let body_text = args.body.or(args.message);
    let body = read_body(body_text, args.body_file.as_ref()).await?;
    let format = args.body_format.to_lowercase();
    let storage_value = match format.as_str() {
        "storage" => body,
        "html" => body,
        "markdown" | "md" => markdown_to_storage(&body),
        _ => {
            return Err(anyhow::anyhow!(
                "Invalid body format: {}. Use storage, html, or markdown.",
                args.body_format
            ));
        }
    };

    let mut payload = json!({
        "type": "comment",
        "container": { "id": page_id, "type": "page" },
        "body": { "storage": { "value": storage_value, "representation": "storage" } }
    });

    if let Some(parent) = args.parent {
        payload["ancestors"] = Value::Array(vec![json!({ "id": parent })]);
    }

    let mut extensions = serde_json::Map::new();
    if let Some(location) = args.location
        && !location.trim().is_empty()
    {
        extensions.insert("location".to_string(), Value::String(location));
    }
    if let Some(inline) = args.inline_properties {
        let parsed: Value =
            serde_json::from_str(&inline).context("Invalid --inline-properties JSON")?;
        extensions.insert("inlineProperties".to_string(), parsed);
        // If the user provides inlineProperties but not location, hint inline.
        extensions
            .entry("location".to_string())
            .or_insert_with(|| Value::String("inline".to_string()));
    }
    if !extensions.is_empty() {
        payload["extensions"] = Value::Object(extensions);
    }

    let url = client.v1_url("/content");
    let result = client.post_json(url, payload).await?;
    match args.output {
        OutputFormat::Json => maybe_print_json(ctx, &result),
        fmt => {
            let rows = vec![
                vec!["ID".to_string(), json_str(&result, "id")],
                vec!["Status".to_string(), json_str(&result, "status")],
            ];
            maybe_print_kv_fmt(ctx, fmt, rows);
            Ok(())
        }
    }
}

#[cfg(feature = "write")]
async fn comment_delete(
    client: &ApiClient,
    ctx: &AppContext,
    args: CommentDeleteArgs,
) -> Result<()> {
    if ctx.dry_run {
        if let Some(fmt) = args.output {
            match fmt {
                OutputFormat::Json => {
                    return maybe_print_json(
                        ctx,
                        &json!({
                            "dryRun": true,
                            "deleted": false,
                            "id": args.comment,
                        }),
                    );
                }
                other => {
                    maybe_print_kv_fmt(
                        ctx,
                        other,
                        vec![
                            vec!["DryRun".to_string(), "true".to_string()],
                            vec!["Deleted".to_string(), "false".to_string()],
                            vec!["ID".to_string(), args.comment],
                        ],
                    );
                    return Ok(());
                }
            }
        }

        print_line(ctx, &format!("Would delete comment {}", args.comment));
        return Ok(());
    }

    if !args.yes {
        let confirm = Confirm::new()
            .with_prompt(format!("Delete comment {}?", args.comment))
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

    let url = client.v1_url(&format!("/content/{}", args.comment));
    client.delete(url).await?;

    if let Some(fmt) = args.output {
        match fmt {
            OutputFormat::Json => maybe_print_json(
                ctx,
                &json!({
                    "deleted": true,
                    "id": args.comment,
                }),
            ),
            other => {
                maybe_print_kv_fmt(
                    ctx,
                    other,
                    vec![
                        vec!["Deleted".to_string(), "true".to_string()],
                        vec!["ID".to_string(), args.comment],
                    ],
                );
                Ok(())
            }
        }
    } else {
        print_line(ctx, &format!("Deleted comment {}", args.comment));
        Ok(())
    }
}

fn comment_location(item: &serde_json::Value) -> String {
    let ext = item.get("extensions");
    let loc = ext
        .and_then(|v| v.get("location"))
        .and_then(|v| v.as_str())
        .or_else(|| {
            ext.and_then(|v| v.get("location"))
                .and_then(|v| v.get("value"))
                .and_then(|v| v.as_str())
        })
        .unwrap_or("");
    loc.to_string()
}

fn comment_parent_id(item: &serde_json::Value) -> Option<String> {
    let ancestors = item.get("ancestors")?.as_array()?;
    let mut last: Option<String> = None;
    for a in ancestors {
        let ty = a
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();
        if ty == "comment"
            && let Some(id) = a.get("id").and_then(|v| v.as_str())
        {
            last = Some(id.to_string());
        }
    }
    last
}
