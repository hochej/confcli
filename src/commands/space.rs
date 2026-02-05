use anyhow::Result;
use confcli::client::ApiClient;
use confcli::json_util::json_str;
use confcli::output::OutputFormat;

use crate::cli::{SpaceCommand, SpaceGetArgs, SpaceListArgs, SpacePagesArgs};
use crate::context::AppContext;
use crate::helpers::{
    markdown_not_supported, maybe_print_json, maybe_print_kv, maybe_print_table_with_count,
    url_with_query,
};
use crate::resolve::{build_page_tree, resolve_space_id};

pub async fn handle(ctx: &AppContext, cmd: SpaceCommand) -> Result<()> {
    let client = crate::context::load_client(ctx)?;
    match cmd {
        SpaceCommand::List(args) => space_list(&client, ctx, args).await,
        SpaceCommand::Get(args) => space_get(&client, ctx, args).await,
        SpaceCommand::Pages(args) => space_pages(&client, ctx, args).await,
    }
}

async fn space_list(client: &ApiClient, ctx: &AppContext, args: SpaceListArgs) -> Result<()> {
    let mut pairs = vec![("limit", args.limit.to_string())];
    if let Some(keys) = args.keys {
        pairs.push(("keys", keys));
    }
    if let Some(space_type) = args.r#type {
        pairs.push(("type", space_type));
    }
    if let Some(status) = args.status {
        pairs.push(("status", status));
    }
    if let Some(labels) = args.labels {
        pairs.push(("labels", labels));
    }
    let url = url_with_query(&client.v2_url("/spaces"), &pairs)?;
    let items = client.get_paginated_results(url, args.all).await?;
    match args.output {
        OutputFormat::Json => maybe_print_json(ctx, &items),
        OutputFormat::Table => {
            let rows = items
                .iter()
                .map(|item| {
                    vec![
                        json_str(item, "id"),
                        json_str(item, "key"),
                        json_str(item, "name"),
                        json_str(item, "type"),
                        json_str(item, "status"),
                    ]
                })
                .collect();
            maybe_print_table_with_count(ctx, &["ID", "Key", "Name", "Type", "Status"], rows);
            Ok(())
        }
        OutputFormat::Markdown => markdown_not_supported(),
    }
}

async fn space_get(client: &ApiClient, ctx: &AppContext, args: SpaceGetArgs) -> Result<()> {
    let space_id = resolve_space_id(client, &args.space).await?;
    let url = client.v2_url(&format!("/spaces/{space_id}"));
    let (json, _) = client.get_json(url).await?;
    match args.output {
        OutputFormat::Json => maybe_print_json(ctx, &json),
        OutputFormat::Table => {
            let rows = vec![
                vec!["ID".to_string(), json_str(&json, "id")],
                vec!["Key".to_string(), json_str(&json, "key")],
                vec!["Name".to_string(), json_str(&json, "name")],
                vec!["Type".to_string(), json_str(&json, "type")],
                vec!["Status".to_string(), json_str(&json, "status")],
            ];
            maybe_print_kv(ctx, rows);
            Ok(())
        }
        OutputFormat::Markdown => markdown_not_supported(),
    }
}

async fn space_pages(client: &ApiClient, ctx: &AppContext, args: SpacePagesArgs) -> Result<()> {
    let space_id = resolve_space_id(client, &args.space).await?;
    let mut pairs = vec![("limit", args.limit.to_string()), ("depth", args.depth)];
    if let Some(status) = args.status {
        pairs.push(("status", status));
    }
    if let Some(title) = args.title {
        pairs.push(("title", title));
    }
    let url = url_with_query(&client.v2_url(&format!("/spaces/{space_id}/pages")), &pairs)?;
    let items = client.get_paginated_results(url, args.all).await?;

    if args.tree {
        match args.output {
            OutputFormat::Json => maybe_print_json(ctx, &items),
            OutputFormat::Table => {
                let tree = build_page_tree(&items);
                for line in tree {
                    println!("{line}");
                }
                Ok(())
            }
            OutputFormat::Markdown => markdown_not_supported(),
        }
    } else {
        match args.output {
            OutputFormat::Json => maybe_print_json(ctx, &items),
            OutputFormat::Table => {
                let rows = items
                    .iter()
                    .map(|item| {
                        vec![
                            json_str(item, "id"),
                            json_str(item, "title"),
                            json_str(item, "status"),
                            json_str(item, "parentId"),
                        ]
                    })
                    .collect();
                maybe_print_table_with_count(ctx, &["ID", "Title", "Status", "Parent"], rows);
                Ok(())
            }
            OutputFormat::Markdown => markdown_not_supported(),
        }
    }
}
