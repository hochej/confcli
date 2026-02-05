use anyhow::Result;
use confcli::client::ApiClient;
use confcli::json_util::json_str;
use confcli::output::OutputFormat;
use serde_json::Value;
#[cfg(feature = "write")]
use serde_json::json;

use crate::cli::*;
use crate::context::AppContext;
#[cfg(feature = "write")]
use crate::helpers::print_line;
use crate::helpers::{maybe_print_json, maybe_print_rows, url_with_query};
use crate::resolve::resolve_page_id;

pub async fn handle(ctx: &AppContext, cmd: LabelCommand) -> Result<()> {
    let client = crate::context::load_client(ctx)?;
    match cmd {
        LabelCommand::List(args) => label_list(&client, ctx, args).await,
        #[cfg(feature = "write")]
        LabelCommand::Add(args) => label_add(&client, ctx, args).await,
        #[cfg(feature = "write")]
        LabelCommand::Remove(args) => label_remove(&client, ctx, args).await,
        LabelCommand::Pages(args) => label_pages(&client, ctx, args).await,
    }
}

async fn label_list(client: &ApiClient, ctx: &AppContext, args: LabelListArgs) -> Result<()> {
    if let Some(page) = &args.page {
        // Page-scoped: list labels on a specific page via v1 API.
        let page_id = resolve_page_id(client, page).await?;
        let url = url_with_query(
            &client.v1_url(&format!("/content/{page_id}/label")),
            &[("limit", args.limit.to_string())],
        )?;
        let items = client.get_paginated_results(url, args.all).await?;
        match args.output {
            OutputFormat::Json => maybe_print_json(ctx, &items),
            fmt => {
                let rows = items
                    .iter()
                    .map(|item| {
                        vec![
                            json_str(item, "id"),
                            json_str(item, "name"),
                            json_str(item, "prefix"),
                        ]
                    })
                    .collect();
                maybe_print_rows(ctx, fmt, &["ID", "Name", "Prefix"], rows);
                Ok(())
            }
        }
    } else {
        // Global: list all labels in the instance.
        let url = url_with_query(
            &client.v2_url("/labels"),
            &[("limit", args.limit.to_string())],
        )?;
        let items = client.get_paginated_results(url, args.all).await?;
        match args.output {
            OutputFormat::Json => maybe_print_json(ctx, &items),
            fmt => {
                let rows = items
                    .iter()
                    .map(|item| {
                        vec![
                            json_str(item, "id"),
                            json_str(item, "name"),
                            json_str(item, "prefix"),
                        ]
                    })
                    .collect();
                maybe_print_rows(ctx, fmt, &["ID", "Name", "Prefix"], rows);
                Ok(())
            }
        }
    }
}

#[cfg(feature = "write")]
async fn label_add(client: &ApiClient, ctx: &AppContext, args: LabelAddArgs) -> Result<()> {
    let page_id = resolve_page_id(client, &args.page).await?;

    if ctx.dry_run {
        let names = args.labels.join(", ");
        print_line(
            ctx,
            &format!("Would add label(s) '{names}' to page {page_id}"),
        );
        return Ok(());
    }

    let url = client.v1_url(&format!("/content/{page_id}/label"));
    let body: serde_json::Value = args
        .labels
        .iter()
        .map(|l| json!({ "prefix": "global", "name": l }))
        .collect::<Vec<_>>()
        .into();
    client.post_json(url, body).await?;
    let noun = if args.labels.len() == 1 {
        "label"
    } else {
        "labels"
    };
    print_line(ctx, &format!("Added {} {}.", args.labels.len(), noun));
    Ok(())
}

#[cfg(feature = "write")]
async fn label_remove(client: &ApiClient, ctx: &AppContext, args: LabelRemoveArgs) -> Result<()> {
    let page_id = resolve_page_id(client, &args.page).await?;

    if ctx.dry_run {
        let names = args.labels.join(", ");
        print_line(
            ctx,
            &format!("Would remove label(s) '{names}' from page {page_id}"),
        );
        return Ok(());
    }

    for label in &args.labels {
        let url = client.v1_url(&format!(
            "/content/{page_id}/label?name={}&prefix=global",
            urlencoding::encode(label)
        ));
        client.delete(url).await?;
    }
    let noun = if args.labels.len() == 1 {
        "label"
    } else {
        "labels"
    };
    print_line(ctx, &format!("Removed {} {}.", args.labels.len(), noun));
    Ok(())
}

async fn label_pages(client: &ApiClient, ctx: &AppContext, args: LabelPagesArgs) -> Result<()> {
    let cql = label_cql(&args.label);
    let url = url_with_query(
        &client.v1_url("/search"),
        &[("cql", cql), ("limit", args.limit.to_string())],
    )?;
    let (json, _) = client.get_json(url).await?;
    match args.output {
        OutputFormat::Json => maybe_print_json(ctx, &json),
        fmt => {
            let results = json
                .get("results")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let rows = results.iter().map(label_result_row).collect();
            maybe_print_rows(ctx, fmt, &["ID", "Type", "Title"], rows);
            Ok(())
        }
    }
}

fn escape_cql_text(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\n', '\r', '\t'], " ")
}

fn label_cql(label: &str) -> String {
    let label = escape_cql_text(label);
    if label.contains(':') {
        format!("label = \"{label}\"")
    } else {
        format!("label in (\"{label}\", \"team:{label}\", \"my:{label}\")")
    }
}

fn label_result_row(item: &Value) -> Vec<String> {
    if let Some(content) = item.get("content") {
        let id = json_str(content, "id");
        let typ = json_str(content, "type");
        let title = json_str(content, "title");
        return vec![id, typ, title];
    }

    let entity_type = item
        .get("entityType")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if entity_type == "space" {
        let key = item
            .get("space")
            .and_then(|v| v.get("key"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("");
        return vec![key.to_string(), "space".to_string(), title.to_string()];
    }

    let id = json_str(item, "id");
    let typ = item
        .get("type")
        .or_else(|| item.get("entityType"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let title = json_str(item, "title");
    vec![id, typ, title]
}
