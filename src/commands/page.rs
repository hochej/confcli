use anyhow::{Context, Result};
use confcli::client::ApiClient;
use confcli::json_util::json_str;
use confcli::markdown::{
    MarkdownOptions, decode_unicode_escapes_str, html_to_markdown_with_options,
};
use confcli::output::{OutputFormat, print_json};
#[cfg(feature = "write")]
use dialoguer::Confirm;
#[cfg(feature = "write")]
use serde_json::{Value, json};

use crate::cli::*;
use crate::context::AppContext;
use crate::helpers::*;
use crate::resolve::*;

pub async fn handle(ctx: &AppContext, cmd: PageCommand) -> Result<()> {
    let client = crate::context::load_client(ctx)?;
    match cmd {
        PageCommand::List(args) => page_list(&client, ctx, args).await,
        PageCommand::Get(args) => page_get(&client, ctx, args).await,
        PageCommand::Body(args) => page_body(&client, ctx, args).await,
        #[cfg(feature = "write")]
        PageCommand::Edit(args) => page_edit(&client, ctx, args).await,
        #[cfg(feature = "write")]
        PageCommand::Create(args) => page_create(&client, ctx, args).await,
        #[cfg(feature = "write")]
        PageCommand::Update(args) => page_update(&client, ctx, args).await,
        #[cfg(feature = "write")]
        PageCommand::Delete(args) => page_delete(&client, ctx, args).await,
        PageCommand::Children(args) => page_children(&client, ctx, args).await,
        PageCommand::History(args) => page_history(&client, ctx, args).await,
        PageCommand::Open(args) => page_open(&client, ctx, args).await,
    }
}

async fn page_list(client: &ApiClient, ctx: &AppContext, args: PageListArgs) -> Result<()> {
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
        OutputFormat::Table => {
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
            maybe_print_table_with_count(ctx, &["ID", "Title", "Space", "Status"], rows);
            Ok(())
        }
        OutputFormat::Markdown => markdown_not_supported(),
    }
}

async fn page_get(client: &ApiClient, ctx: &AppContext, args: PageGetArgs) -> Result<()> {
    let page_id = resolve_page_id(client, &args.page).await?;
    let mut url = client.v2_url(&format!(
        "/pages/{page_id}?body-format={}",
        args.body_format
    ));
    if let Some(version) = args.version {
        url.push_str(&format!("&version={version}"));
    }
    let (json, _) = client.get_json(url).await?;
    match args.output {
        OutputFormat::Json => maybe_print_json(ctx, &json),
        OutputFormat::Table => {
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
            let rows = vec![
                vec!["ID".to_string(), json_str(&json, "id")],
                vec!["Title".to_string(), json_str(&json, "title")],
                vec!["Space".to_string(), space_key],
                vec!["Status".to_string(), json_str(&json, "status")],
                vec!["Version".to_string(), version],
                vec!["Parent".to_string(), json_str(&json, "parentId")],
                vec!["URL".to_string(), format!("{}{webui}", client.base_url())],
            ];
            maybe_print_kv(ctx, rows);
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
            println!("{output}");
            Ok(())
        }
    }
}

async fn page_body(client: &ApiClient, ctx: &AppContext, args: PageBodyArgs) -> Result<()> {
    let page_id = resolve_page_id(client, &args.page).await?;
    let format = args.format.to_lowercase();
    match format.as_str() {
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
            let output = if ctx.quiet {
                markdown
            } else {
                add_markdown_header(client.base_url(), &json, &markdown)
            };
            println!("{output}");
            Ok(())
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
            let decoded = decode_unicode_escapes_str(html);
            println!("{decoded}");
            Ok(())
        }
        "storage" => {
            let url = client.v2_url(&format!("/pages/{page_id}?body-format=storage"));
            let (json, _) = client.get_json(url).await?;
            let body = json
                .get("body")
                .and_then(|body| body.get("storage"))
                .and_then(|storage| storage.get("value"))
                .and_then(|value| value.as_str())
                .context("Missing storage body content")?;
            println!("{body}");
            Ok(())
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
                Ok(value) => print_json(&value)?,
                Err(_) => println!("{body}"),
            }
            Ok(())
        }
        _ => Err(anyhow::anyhow!(
            "Invalid body format: {}. Use markdown, view, storage, atlas_doc_format, or adf.",
            args.format
        )),
    }
}

#[cfg(feature = "write")]
async fn page_edit(client: &ApiClient, ctx: &AppContext, args: PageEditArgs) -> Result<()> {
    let page_id = resolve_page_id(client, &args.page).await?;
    let format = args.format.to_lowercase();
    let body_format = match format.as_str() {
        "storage" => "storage",
        "atlas_doc_format" | "adf" => "atlas_doc_format",
        _ => {
            return Err(anyhow::anyhow!(
                "Invalid --format: {}. Use storage or adf.",
                args.format
            ));
        }
    };

    let url = client.v2_url(&format!("/pages/{page_id}?body-format={body_format}"));
    let (json, _) = client.get_json(url).await?;
    let current_version = json
        .get("version")
        .and_then(|v| v.get("number"))
        .and_then(|v| v.as_i64())
        .context("Missing current version number")?;
    let title = json
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let status = json
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("current")
        .to_string();

    let original_body = json
        .get("body")
        .and_then(|body| body.get(body_format))
        .and_then(|body| body.get("value"))
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();

    let (orig_for_file, ext) = if body_format == "atlas_doc_format" {
        // Value is a JSON string; write it pretty for editing.
        let pretty = match serde_json::from_str::<serde_json::Value>(&original_body) {
            Ok(v) => serde_json::to_string_pretty(&v).unwrap_or(original_body.clone()),
            Err(_) => original_body.clone(),
        };
        (pretty, "json")
    } else {
        (original_body.clone(), "html")
    };

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let tmp_dir = std::env::temp_dir();
    let orig_path = tmp_dir.join(format!("confcli-edit-{page_id}-{stamp}.orig.{ext}"));
    let edit_path = tmp_dir.join(format!("confcli-edit-{page_id}-{stamp}.{ext}"));

    tokio::fs::write(&orig_path, orig_for_file.as_bytes()).await?;
    tokio::fs::write(&edit_path, orig_for_file.as_bytes()).await?;

    // Open $EDITOR (or $VISUAL, or vi) and wait.
    let editor = std::env::var("EDITOR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            std::env::var("VISUAL")
                .ok()
                .filter(|s| !s.trim().is_empty())
        })
        .unwrap_or_else(|| "vi".to_string());
    let status_code = std::process::Command::new(editor)
        .arg(&edit_path)
        .status()
        .context("Failed to launch editor")?;
    if !status_code.success() {
        return Err(anyhow::anyhow!("Editor exited with status {status_code}"));
    }

    let edited = tokio::fs::read_to_string(&edit_path).await?;
    if edited == orig_for_file {
        print_line(ctx, "No changes.");
        return Ok(());
    }

    if args.diff {
        // Best-effort: use system `diff -u`.
        let _ = std::process::Command::new("diff")
            .args(["-u"])
            .arg(&orig_path)
            .arg(&edit_path)
            .status();
    }

    if !args.yes {
        let confirm = Confirm::new()
            .with_prompt("Save changes?")
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

    // Version conflict guard.
    let check_url = client.v2_url(&format!("/pages/{page_id}"));
    let (latest, _) = client.get_json(check_url).await?;
    let latest_version = latest
        .get("version")
        .and_then(|v| v.get("number"))
        .and_then(|v| v.as_i64())
        .context("Missing latest version number")?;
    if latest_version != current_version {
        return Err(anyhow::anyhow!(
            "Version conflict: page is now at v{latest_version} (was v{current_version}). Re-run `confcli page edit`."
        ));
    }

    let new_value = if body_format == "atlas_doc_format" {
        // Send compact JSON string if possible.
        match serde_json::from_str::<serde_json::Value>(&edited) {
            Ok(v) => serde_json::to_string(&v).unwrap_or(edited),
            Err(_) => edited,
        }
    } else {
        edited
    };

    let payload = json!({
        "id": page_id,
        "title": title,
        "status": status,
        "body": { "representation": body_format, "value": new_value },
        "version": { "number": current_version + 1 }
    });
    let put_url = client.v2_url(&format!("/pages/{page_id}"));
    let result = client.put_json(put_url, payload).await?;
    let webui = result
        .get("_links")
        .and_then(|v| v.get("webui"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let rows = vec![
        vec!["ID".to_string(), json_str(&result, "id")],
        vec!["Title".to_string(), json_str(&result, "title")],
        vec!["Status".to_string(), json_str(&result, "status")],
        vec!["Web".to_string(), webui.to_string()],
    ];
    maybe_print_kv(ctx, rows);
    Ok(())
}

#[cfg(feature = "write")]
async fn page_create(client: &ApiClient, ctx: &AppContext, args: PageCreateArgs) -> Result<()> {
    let title = match &args.title {
        Some(title) => title.clone(),
        None => derive_title_from_file(args.body_file.as_ref())
            .context("Title is required when reading from stdin")?,
    };

    if ctx.dry_run {
        print_line(
            ctx,
            &format!("Would create page '{title}' in space {}", args.space),
        );
        return Ok(());
    }

    let space_id = resolve_space_id(client, &args.space).await?;
    let body = read_body(args.body, args.body_file.as_ref()).await?;

    let mut payload = json!({
        "spaceId": space_id,
        "title": title,
        "body": { "representation": args.body_format, "value": body },
        "status": args.status.unwrap_or_else(|| "current".to_string()),
    });
    if let Some(parent) = args.parent {
        let parent_id = resolve_page_id(client, &parent).await?;
        payload["parentId"] = Value::String(parent_id);
    }
    let url = client.v2_url("/pages");
    let result = client.post_json(url, payload).await?;
    match args.output {
        OutputFormat::Json => maybe_print_json(ctx, &result),
        OutputFormat::Table => {
            let space_key = resolve_space_key(
                client,
                result.get("spaceId").and_then(|v| v.as_str()).unwrap_or(""),
            )
            .await
            .unwrap_or_default();
            let webui = result
                .get("_links")
                .and_then(|v| v.get("webui"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let rows = vec![
                vec!["ID".to_string(), json_str(&result, "id")],
                vec!["Title".to_string(), json_str(&result, "title")],
                vec!["Space".to_string(), space_key],
                vec!["Web".to_string(), webui.to_string()],
            ];
            maybe_print_kv(ctx, rows);
            Ok(())
        }
        OutputFormat::Markdown => markdown_not_supported(),
    }
}

#[cfg(feature = "write")]
async fn page_update(client: &ApiClient, ctx: &AppContext, args: PageUpdateArgs) -> Result<()> {
    let page_id = resolve_page_id(client, &args.page).await?;
    let url = client.v2_url(&format!("/pages/{page_id}"));
    let (current, _) = client.get_json(url.clone()).await?;
    let current_version = current
        .get("version")
        .and_then(|v| v.get("number"))
        .and_then(|v| v.as_i64())
        .context("Missing current version number")?;
    let title = args
        .title
        .or_else(|| {
            current
                .get("title")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .context("Title is required")?;
    let status = args
        .status
        .or_else(|| {
            current
                .get("status")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "current".to_string());

    if ctx.dry_run {
        print_line(
            ctx,
            &format!(
                "Would update page {page_id} to version {}",
                current_version + 1
            ),
        );
        return Ok(());
    }

    let body = if args.body.is_none() && args.body_file.is_none() {
        let body_url = client.v2_url(&format!(
            "/pages/{page_id}?body-format={}",
            args.body_format
        ));
        let (current_body, _) = client.get_json(body_url).await?;
        current_body
            .get("body")
            .and_then(|body| body.get(&args.body_format))
            .and_then(|body| body.get("value"))
            .and_then(|value| value.as_str())
            .context("Missing body content for update")?
            .to_string()
    } else {
        read_body(args.body, args.body_file.as_ref()).await?
    };

    let mut payload = json!({
        "id": page_id,
        "title": title,
        "status": status,
        "body": { "representation": args.body_format, "value": body },
        "version": { "number": current_version + 1 }
    });
    if let Some(message) = args.message {
        payload["version"]["message"] = Value::String(message);
    }
    if let Some(parent) = args.parent {
        let parent_id = resolve_page_id(client, &parent).await?;
        payload["parentId"] = Value::String(parent_id);
    }
    let result = client.put_json(url, payload).await?;
    match args.output {
        OutputFormat::Json => maybe_print_json(ctx, &result),
        OutputFormat::Table => {
            let webui = result
                .get("_links")
                .and_then(|v| v.get("webui"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let rows = vec![
                vec!["ID".to_string(), json_str(&result, "id")],
                vec!["Title".to_string(), json_str(&result, "title")],
                vec!["Status".to_string(), json_str(&result, "status")],
                vec!["Web".to_string(), webui.to_string()],
            ];
            maybe_print_kv(ctx, rows);
            Ok(())
        }
        OutputFormat::Markdown => markdown_not_supported(),
    }
}

#[cfg(feature = "write")]
async fn page_delete(client: &ApiClient, ctx: &AppContext, args: PageDeleteArgs) -> Result<()> {
    let page_id = resolve_page_id(client, &args.page).await?;

    if ctx.dry_run {
        let action = if args.purge { "purge" } else { "delete" };
        print_line(ctx, &format!("Would {action} page {page_id}"));
        return Ok(());
    }

    if !args.yes {
        let confirm = Confirm::new()
            .with_prompt(format!("Delete page {page_id}?"))
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
    if args.purge {
        let status = page_status(client, &page_id).await?;
        if status != "trashed" {
            if !args.force {
                return Err(anyhow::anyhow!(
                    "Page {page_id} is not trashed. Delete first or use --force to trash then purge."
                ));
            }
            let url = client.v2_url(&format!("/pages/{page_id}"));
            client.delete(url).await?;
        }
        let mut url = client.v2_url(&format!("/pages/{page_id}"));
        url.push_str("?purge=true");
        client.delete(url).await?;
        print_line(ctx, &format!("Purged page {page_id}"));
        Ok(())
    } else {
        let url = client.v2_url(&format!("/pages/{page_id}"));
        client.delete(url).await?;
        print_line(ctx, &format!("Deleted page {page_id}"));
        Ok(())
    }
}

async fn page_children(client: &ApiClient, ctx: &AppContext, args: PageChildrenArgs) -> Result<()> {
    let page_id = resolve_page_id(client, &args.page).await?;
    let endpoint = if args.recursive {
        "descendants"
    } else {
        "direct-children"
    };
    let url = url_with_query(
        &client.v2_url(&format!("/pages/{page_id}/{endpoint}")),
        &[("limit", args.limit.to_string())],
    )?;
    let items = client.get_paginated_results(url, args.all).await?;
    match args.output {
        OutputFormat::Json => maybe_print_json(ctx, &items),
        OutputFormat::Table => {
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
                maybe_print_table_with_count(ctx, &["ID", "Title", "Parent"], rows);
            } else {
                let rows = items
                    .iter()
                    .map(|item| vec![json_str(item, "id"), json_str(item, "title")])
                    .collect();
                maybe_print_table_with_count(ctx, &["ID", "Title"], rows);
            }
            Ok(())
        }
        OutputFormat::Markdown => markdown_not_supported(),
    }
}

async fn page_history(client: &ApiClient, ctx: &AppContext, args: PageHistoryArgs) -> Result<()> {
    let page_id = resolve_page_id(client, &args.page).await?;
    let url = url_with_query(
        &client.v2_url(&format!("/pages/{page_id}/versions")),
        &[("limit", args.limit.to_string())],
    )?;
    let items = client.get_paginated_results(url, false).await?;
    match args.output {
        OutputFormat::Json => maybe_print_json(ctx, &items),
        OutputFormat::Table => {
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
            maybe_print_table_with_count(ctx, &["Version", "Message", "Created", "Minor"], rows);
            Ok(())
        }
        OutputFormat::Markdown => markdown_not_supported(),
    }
}

async fn page_open(client: &ApiClient, ctx: &AppContext, args: PageOpenArgs) -> Result<()> {
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
