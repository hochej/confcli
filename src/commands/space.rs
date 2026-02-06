use anyhow::Result;
use confcli::client::ApiClient;
use confcli::json_util::json_str;
use confcli::output::OutputFormat;
#[cfg(feature = "write")]
use dialoguer::Confirm;
#[cfg(feature = "write")]
use serde_json::json;

use crate::cli::{SpaceCommand, SpaceGetArgs, SpaceListArgs, SpacePagesArgs};
#[cfg(feature = "write")]
use crate::cli::{SpaceCreateArgs, SpaceDeleteArgs};
use crate::context::AppContext;
use crate::helpers::print_line;
use crate::helpers::{maybe_print_json, maybe_print_kv_fmt, maybe_print_rows, url_with_query};
#[cfg(feature = "write")]
use crate::resolve::resolve_space_key;
use crate::resolve::{build_page_tree, resolve_space_id};

pub async fn handle(ctx: &AppContext, cmd: SpaceCommand) -> Result<()> {
    let client = crate::context::load_client(ctx)?;
    match cmd {
        SpaceCommand::List(args) => space_list(&client, ctx, args).await,
        SpaceCommand::Get(args) => space_get(&client, ctx, args).await,
        SpaceCommand::Pages(args) => space_pages(&client, ctx, args).await,
        #[cfg(feature = "write")]
        SpaceCommand::Create(args) => space_create(&client, ctx, args).await,
        #[cfg(feature = "write")]
        SpaceCommand::Delete(args) => space_delete(&client, ctx, args).await,
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
        fmt => {
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
            maybe_print_rows(ctx, fmt, &["ID", "Key", "Name", "Type", "Status"], rows);
            Ok(())
        }
    }
}

async fn space_get(client: &ApiClient, ctx: &AppContext, args: SpaceGetArgs) -> Result<()> {
    let space_id = resolve_space_id(client, &args.space).await?;
    let url = client.v2_url(&format!("/spaces/{space_id}"));
    let (json, _) = client.get_json(url).await?;
    match args.output {
        OutputFormat::Json => maybe_print_json(ctx, &json),
        fmt => {
            let rows = vec![
                vec!["ID".to_string(), json_str(&json, "id")],
                vec!["Key".to_string(), json_str(&json, "key")],
                vec!["Name".to_string(), json_str(&json, "name")],
                vec!["Type".to_string(), json_str(&json, "type")],
                vec!["Status".to_string(), json_str(&json, "status")],
            ];
            maybe_print_kv_fmt(ctx, fmt, rows);
            Ok(())
        }
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
            _ => {
                let tree = build_page_tree(&items);
                for line in tree {
                    print_line(ctx, &line);
                }
                Ok(())
            }
        }
    } else {
        match args.output {
            OutputFormat::Json => maybe_print_json(ctx, &items),
            fmt => {
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
                maybe_print_rows(ctx, fmt, &["ID", "Title", "Status", "Parent"], rows);
                Ok(())
            }
        }
    }
}

#[cfg(feature = "write")]
async fn space_create(client: &ApiClient, ctx: &AppContext, args: SpaceCreateArgs) -> Result<()> {
    if ctx.dry_run {
        print_line(
            ctx,
            &format!("Would create space '{}' ({})", args.name, args.key),
        );
        return Ok(());
    }

    let mut payload = json!({
        "key": args.key,
        "name": args.name,
    });
    if let Some(desc) = args.description {
        payload["description"] = json!({
            "plain": { "value": desc, "representation": "plain" }
        });
    }

    // Use v1 API because the v2 endpoint ignores the description field.
    let url = client.v1_url("/space");
    let result = client.post_json(url, payload).await?;

    match args.output {
        OutputFormat::Json => {
            if args.compact_json {
                let id = json_str(&result, "id");
                let key = json_str(&result, "key");
                let name = json_str(&result, "name");
                let space_type = json_str(&result, "type");
                let status = json_str(&result, "status");
                let homepage_id = result
                    .get("homepage")
                    .and_then(|v| v.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let webui = result
                    .get("_links")
                    .and_then(|v| v.get("webui"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let url = if !webui.is_empty() {
                    format!("{}{webui}", client.base_url())
                } else if !key.is_empty() {
                    format!("{}/spaces/{key}", client.base_url())
                } else {
                    "".to_string()
                };

                let compact = json!({
                    "id": id,
                    "key": key,
                    "name": name,
                    "type": space_type,
                    "status": status,
                    "homepageId": homepage_id,
                    "url": url,
                });
                maybe_print_json(ctx, &compact)
            } else {
                maybe_print_json(ctx, &result)
            }
        }
        fmt => {
            let rows = vec![
                vec!["ID".to_string(), json_str(&result, "id")],
                vec!["Key".to_string(), json_str(&result, "key")],
                vec!["Name".to_string(), json_str(&result, "name")],
                vec!["Type".to_string(), json_str(&result, "type")],
                vec!["Status".to_string(), json_str(&result, "status")],
            ];
            maybe_print_kv_fmt(ctx, fmt, rows);
            Ok(())
        }
    }
}

#[cfg(feature = "write")]
async fn space_delete(client: &ApiClient, ctx: &AppContext, args: SpaceDeleteArgs) -> Result<()> {
    let space_id = resolve_space_id(client, &args.space).await?;
    let space_key = resolve_space_key(client, &space_id).await?;

    if ctx.dry_run {
        if let Some(fmt) = args.output {
            match fmt {
                OutputFormat::Json => {
                    return maybe_print_json(
                        ctx,
                        &json!({
                            "dryRun": true,
                            "deleted": false,
                            "id": space_id,
                            "key": space_key,
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
                            vec!["ID".to_string(), space_id],
                            vec!["Key".to_string(), space_key],
                        ],
                    );
                    return Ok(());
                }
            }
        }

        print_line(ctx, &format!("Would delete space {space_key}"));
        return Ok(());
    }

    if !args.yes {
        let confirm = Confirm::new()
            .with_prompt(format!(
                "Delete space {space_key}? This will trash all content in the space."
            ))
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

    // Use v1 API — the v2 DELETE /spaces/{id} endpoint does not support space deletion.
    let url = client.v1_url(&format!("/space/{space_key}"));
    client.delete(url).await?;

    if let Some(fmt) = args.output {
        match fmt {
            OutputFormat::Json => maybe_print_json(
                ctx,
                &json!({
                    "deleted": true,
                    "id": space_id,
                    "key": space_key,
                }),
            ),
            other => {
                maybe_print_kv_fmt(
                    ctx,
                    other,
                    vec![
                        vec!["Deleted".to_string(), "true".to_string()],
                        vec!["ID".to_string(), space_id],
                        vec!["Key".to_string(), space_key],
                    ],
                );
                Ok(())
            }
        }
    } else {
        print_line(ctx, &format!("Deleted space {space_key}"));
        Ok(())
    }
}
