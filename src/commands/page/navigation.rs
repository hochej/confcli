use anyhow::{Context, Result};
use confcli::client::ApiClient;
use confcli::json_util::json_str;
use confcli::output::OutputFormat;

use crate::cli::{PageChildrenArgs, PageHistoryArgs, PageOpenArgs};
use crate::context::AppContext;
use crate::helpers::*;
use crate::resolve::*;

pub(super) async fn page_children(
    client: &ApiClient,
    ctx: &AppContext,
    args: PageChildrenArgs,
) -> Result<()> {
    let page_id = resolve_page_id(client, &args.page).await?;

    let items = if args.recursive {
        confcli::tree::fetch_descendants_via_direct_children(
            client, &page_id, args.limit, args.all, None,
        )
        .await?
    } else {
        let url = url_with_query(
            &client.v2_url(&format!("/pages/{page_id}/direct-children")),
            &[("limit", args.limit.to_string())],
        )?;
        client.get_paginated_results(url, args.all).await?
    };

    match args.output {
        OutputFormat::Json => maybe_print_json(ctx, &items),
        fmt => {
            if args.recursive {
                let rows = items
                    .iter()
                    .map(|item| {
                        vec![
                            json_str(item, "id"),
                            json_str(item, "title"),
                            json_str(item, "parentId"),
                        ]
                    })
                    .collect();
                maybe_print_rows(ctx, fmt, &["ID", "Title", "Parent"], rows);
            } else {
                let rows = items
                    .iter()
                    .map(|item| vec![json_str(item, "id"), json_str(item, "title")])
                    .collect();
                maybe_print_rows(ctx, fmt, &["ID", "Title"], rows);
            }
            Ok(())
        }
    }
}

pub(super) async fn page_history(
    client: &ApiClient,
    ctx: &AppContext,
    args: PageHistoryArgs,
) -> Result<()> {
    let page_id = resolve_page_id(client, &args.page).await?;
    let url = url_with_query(
        &client.v2_url(&format!("/pages/{page_id}/versions")),
        &[("limit", args.limit.to_string())],
    )?;
    let items = client.get_paginated_results(url, false).await?;
    match args.output {
        OutputFormat::Json => maybe_print_json(ctx, &items),
        fmt => {
            let rows = items
                .iter()
                .map(|item| {
                    let number = item
                        .get("number")
                        .map(|v| v.to_string())
                        .unwrap_or_default();
                    let message = json_str(item, "message");
                    let created_at = format_timestamp(&json_str(item, "createdAt"));
                    let minor_edit = item
                        .get("minorEdit")
                        .and_then(|v| v.as_bool())
                        .map(|b| if b { "yes" } else { "no" })
                        .unwrap_or("")
                        .to_string();
                    vec![number, message, created_at, minor_edit]
                })
                .collect();
            maybe_print_rows(ctx, fmt, &["Version", "Message", "Created", "Minor"], rows);
            Ok(())
        }
    }
}

pub(super) async fn page_open(
    client: &ApiClient,
    ctx: &AppContext,
    args: PageOpenArgs,
) -> Result<()> {
    let page_id = resolve_page_id(client, &args.page).await?;
    let url = client.v2_url(&format!("/pages/{page_id}"));
    let (json, _) = client.get_json(url).await?;
    let webui = json
        .get("_links")
        .and_then(|v| v.get("webui"))
        .and_then(|v| v.as_str())
        .context("Missing webui link for page")?;
    let full_url = format!("{}{webui}", client.base_url());

    if ctx.dry_run {
        print_line(ctx, &format!("Would open {full_url}"));
        return Ok(());
    }

    print_line(ctx, &format!("Opening {full_url}"));
    open_url(&full_url)?;
    Ok(())
}
