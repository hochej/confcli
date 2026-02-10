use anyhow::{Context, Result};
use confcli::client::ApiClient;
use confcli::json_util::json_str;
use confcli::output::OutputFormat;
use dialoguer::Confirm;
use serde_json::{Value, json};
use similar::TextDiff;
use tempfile::TempDir;

use crate::cli::{PageCreateArgs, PageDeleteArgs, PageEditArgs, PageUpdateArgs};
use crate::context::AppContext;
use crate::helpers::*;
use crate::resolve::*;

pub(super) async fn page_edit(
    client: &ApiClient,
    ctx: &AppContext,
    args: PageEditArgs,
) -> Result<()> {
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
        let pretty = match serde_json::from_str::<serde_json::Value>(&original_body) {
            Ok(v) => serde_json::to_string_pretty(&v).unwrap_or(original_body.clone()),
            Err(_) => original_body.clone(),
        };
        (pretty, "json")
    } else {
        (original_body.clone(), "html")
    };

    let tmp = TempDir::new().context("Failed to create temp directory")?;
    let orig_path = tmp.path().join(format!("original.{ext}"));
    let edit_path = tmp.path().join(format!("edited.{ext}"));

    tokio::fs::write(&orig_path, orig_for_file.as_bytes()).await?;
    tokio::fs::write(&edit_path, orig_for_file.as_bytes()).await?;

    let editor_str = std::env::var("EDITOR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            std::env::var("VISUAL")
                .ok()
                .filter(|s| !s.trim().is_empty())
        })
        .unwrap_or_else(|| "vi".to_string());

    let mut parts = shell_words::split(&editor_str).unwrap_or_else(|_| vec![editor_str.clone()]);
    if parts.is_empty() {
        parts.push("vi".to_string());
    }
    let editor_cmd = parts.remove(0);

    let status_code = std::process::Command::new(editor_cmd)
        .args(parts)
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
        let diff = TextDiff::from_lines(&orig_for_file, &edited);
        let unified = diff
            .unified_diff()
            .context_radius(3)
            .header("original", "edited")
            .to_string();
        if !ctx.quiet {
            print!("{unified}");
        }
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

pub(super) async fn page_create(
    client: &ApiClient,
    ctx: &AppContext,
    args: PageCreateArgs,
) -> Result<()> {
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
        fmt => {
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
            maybe_print_kv_fmt(ctx, fmt, rows);
            Ok(())
        }
    }
}

pub(super) async fn page_update(
    client: &ApiClient,
    ctx: &AppContext,
    args: PageUpdateArgs,
) -> Result<()> {
    let nothing_to_update = args.title.is_none()
        && args.parent.is_none()
        && args.status.is_none()
        && args.body.is_none()
        && args.body_file.is_none()
        && args.message.is_none();
    if nothing_to_update {
        return Err(anyhow::anyhow!(
            "Nothing to update. Provide at least one of --title, --parent, --status, --body/--body-file, or --message (or use `confcli page edit`)."
        ));
    }

    let page_id = resolve_page_id(client, &args.page).await?;

    let get_url = client.v2_url(&format!(
        "/pages/{page_id}?body-format={}",
        args.body_format
    ));
    let (current, _) = client.get_json(get_url).await?;

    let url = client.v2_url(&format!("/pages/{page_id}"));
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
        current
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
        fmt => {
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
            maybe_print_kv_fmt(ctx, fmt, rows);
            Ok(())
        }
    }
}

pub(super) async fn page_delete(
    client: &ApiClient,
    ctx: &AppContext,
    args: PageDeleteArgs,
) -> Result<()> {
    let page_id = resolve_page_id(client, &args.page).await?;

    let action = if args.purge { "purge" } else { "delete" };

    if ctx.dry_run {
        return print_write_action_result(
            ctx,
            args.output,
            &format!("Would {action} page {page_id}"),
            &json!({
                "dryRun": true,
                "action": action,
                "deleted": false,
                "id": page_id,
            }),
            vec![
                vec!["DryRun".to_string(), "true".to_string()],
                vec!["Action".to_string(), action.to_string()],
                vec!["Deleted".to_string(), "false".to_string()],
                vec!["ID".to_string(), page_id.clone()],
            ],
        );
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
    } else {
        let url = client.v2_url(&format!("/pages/{page_id}"));
        client.delete(url).await?;
    }

    let past = if args.purge { "Purged" } else { "Deleted" };
    print_write_action_result(
        ctx,
        args.output,
        &format!("{past} page {page_id}"),
        &json!({
            "action": action,
            "deleted": true,
            "id": page_id,
        }),
        vec![
            vec!["Action".to_string(), action.to_string()],
            vec!["Deleted".to_string(), "true".to_string()],
            vec!["ID".to_string(), page_id],
        ],
    )
}
