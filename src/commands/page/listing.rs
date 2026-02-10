use anyhow::{Context, Result};
use confcli::client::ApiClient;
use confcli::json_util::json_str;
use confcli::markdown::{
    MarkdownOptions, decode_unicode_escapes_str, html_to_markdown_with_options,
};
use confcli::output::OutputFormat;

use crate::cli::{PageBodyArgs, PageGetArgs, PageListArgs};
use crate::context::AppContext;
use crate::helpers::*;
use crate::resolve::*;

pub(super) async fn page_list(
    client: &ApiClient,
    ctx: &AppContext,
    args: PageListArgs,
) -> Result<()> {
    let mut pairs = vec![("limit", args.limit.to_string())];
    if let Some(space) = args.space {
        let space_id = resolve_space_id(client, &space).await?;
        pairs.push(("space-id", space_id));
    }
    if let Some(status) = args.status {
        pairs.push(("status", status));
    }
    if let Some(title) = args.title {
        pairs.push(("title", title));
    }
    let url = url_with_query(&client.v2_url("/pages"), &pairs)?;
    let items = client.get_paginated_results(url, args.all).await?;
    match args.output {
        OutputFormat::Json => maybe_print_json(ctx, &items),
        fmt => {
            let space_ids: Vec<String> = items
                .iter()
                .filter_map(|item| {
                    item.get("spaceId")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
                .collect();
            let space_keys = resolve_space_keys(client, &space_ids).await?;
            let rows = items
                .iter()
                .map(|item| {
                    let space_id = json_str(item, "spaceId");
                    let space_key = space_keys
                        .get(&space_id)
                        .cloned()
                        .unwrap_or_else(|| space_id.clone());
                    vec![
                        json_str(item, "id"),
                        json_str(item, "title"),
                        space_key,
                        json_str(item, "status"),
                    ]
                })
                .collect();
            maybe_print_rows(ctx, fmt, &["ID", "Title", "Space", "Status"], rows);
            Ok(())
        }
    }
}

pub(super) async fn page_get(
    client: &ApiClient,
    ctx: &AppContext,
    args: PageGetArgs,
) -> Result<()> {
    let page_id = resolve_page_id(client, &args.page).await?;

    match args.output {
        OutputFormat::Json => {
            let mut url = client.v2_url(&format!(
                "/pages/{page_id}?body-format={}",
                args.body_format
            ));
            if let Some(version) = args.version {
                url.push_str(&format!("&version={version}"));
            }
            let (json, _) = client.get_json(url).await?;
            maybe_print_json(ctx, &json)
        }
        OutputFormat::Table => {
            let base = client.v2_url(&format!("/pages/{page_id}"));
            let mut pairs: Vec<(&str, String)> = Vec::new();
            if args.show_body {
                pairs.push(("body-format", args.body_format.clone()));
            }
            if let Some(version) = args.version {
                pairs.push(("version", version.to_string()));
            }
            let url = if pairs.is_empty() {
                base
            } else {
                url_with_query(&base, &pairs)?
            };

            let (json, _) = client.get_json(url).await?;

            let space_id = json_str(&json, "spaceId");
            let space_key = resolve_space_key(client, &space_id)
                .await
                .unwrap_or_else(|_| space_id.clone());
            let webui = json
                .get("_links")
                .and_then(|v| v.get("webui"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let version = json
                .get("version")
                .and_then(|v| v.get("number"))
                .map(|v| v.to_string())
                .unwrap_or_default();

            let mut rows = vec![
                vec!["ID".to_string(), json_str(&json, "id")],
                vec!["Title".to_string(), json_str(&json, "title")],
                vec!["Space".to_string(), space_key],
                vec!["Status".to_string(), json_str(&json, "status")],
                vec!["Version".to_string(), version],
                vec!["Parent".to_string(), json_str(&json, "parentId")],
                vec!["URL".to_string(), format!("{}{webui}", client.base_url())],
            ];

            if args.show_body
                && let Some(body_value) = json
                    .get("body")
                    .and_then(|body| body.get(&args.body_format))
                    .and_then(|fmt| fmt.get("value"))
                    .and_then(|v| v.as_str())
            {
                rows.push(vec!["Body".to_string(), body_value.to_string()]);
            }

            maybe_print_kv_fmt(ctx, OutputFormat::Table, rows);
            Ok(())
        }
        OutputFormat::Markdown => {
            let view_url = client.v2_url(&format!("/pages/{page_id}?body-format=view"));
            let (view_json, _) = client.get_json(view_url).await?;
            let html = view_json
                .get("body")
                .and_then(|body| body.get("view"))
                .and_then(|view| view.get("value"))
                .and_then(|value| value.as_str())
                .context("Missing view body content")?;
            let markdown = html_to_markdown_with_options(
                html,
                client.base_url(),
                MarkdownOptions {
                    keep_empty_list_items: args.keep_empty_list_items,
                },
            )?;
            let output = if ctx.quiet {
                markdown
            } else {
                add_markdown_header(client.base_url(), &view_json, &markdown)
            };
            if !ctx.quiet {
                println!("{output}");
            }
            Ok(())
        }
    }
}

pub(super) async fn page_body(
    client: &ApiClient,
    ctx: &AppContext,
    args: PageBodyArgs,
) -> Result<()> {
    let page_id = resolve_page_id(client, &args.page).await?;
    let format = args.format.to_lowercase();
    let body_value: String = match format.as_str() {
        "markdown" | "md" => {
            let url = client.v2_url(&format!("/pages/{page_id}?body-format=view"));
            let (json, _) = client.get_json(url).await?;
            let html = json
                .get("body")
                .and_then(|body| body.get("view"))
                .and_then(|view| view.get("value"))
                .and_then(|value| value.as_str())
                .context("Missing view body content")?;
            let markdown = html_to_markdown_with_options(
                html,
                client.base_url(),
                MarkdownOptions {
                    keep_empty_list_items: args.keep_empty_list_items,
                },
            )?;
            if ctx.quiet {
                markdown
            } else {
                add_markdown_header(client.base_url(), &json, &markdown)
            }
        }
        "view" => {
            let url = client.v2_url(&format!("/pages/{page_id}?body-format=view"));
            let (json, _) = client.get_json(url).await?;
            let html = json
                .get("body")
                .and_then(|body| body.get("view"))
                .and_then(|view| view.get("value"))
                .and_then(|value| value.as_str())
                .context("Missing view body content")?;
            decode_unicode_escapes_str(html)
        }
        "storage" => {
            let url = client.v2_url(&format!("/pages/{page_id}?body-format=storage"));
            let (json, _) = client.get_json(url).await?;
            json.get("body")
                .and_then(|body| body.get("storage"))
                .and_then(|storage| storage.get("value"))
                .and_then(|value| value.as_str())
                .context("Missing storage body content")?
                .to_string()
        }
        "atlas_doc_format" | "adf" => {
            let url = client.v2_url(&format!("/pages/{page_id}?body-format=atlas_doc_format"));
            let (json, _) = client.get_json(url).await?;
            let body = json
                .get("body")
                .and_then(|body| body.get("atlas_doc_format"))
                .and_then(|adf| adf.get("value"))
                .and_then(|value| value.as_str())
                .context("Missing ADF body content")?;
            match serde_json::from_str::<serde_json::Value>(body) {
                Ok(value) => {
                    serde_json::to_string_pretty(&value).unwrap_or_else(|_| body.to_string())
                }
                Err(_) => body.to_string(),
            }
        }
        _ => {
            return Err(anyhow::anyhow!(
                "Invalid body format: {}. Use markdown, view, storage, atlas_doc_format, or adf.",
                args.format
            ));
        }
    };

    match args.output {
        OutputFormat::Json => {
            let obj = serde_json::json!({
                "pageId": page_id,
                "format": args.format,
                "body": body_value,
            });
            maybe_print_json(ctx, &obj)
        }
        _ => {
            if !ctx.quiet {
                println!("{body_value}");
            }
            Ok(())
        }
    }
}
