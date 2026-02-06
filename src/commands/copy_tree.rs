use anyhow::{Context, Result};
use confcli::client::ApiClient;
use confcli::json_util::json_str;
use confcli::output::OutputFormat;
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Semaphore;

use crate::cli::CopyTreeArgs;
use crate::context::AppContext;
use crate::download::fetch_page_with_body_format;
use crate::helpers::*;
use crate::resolve::resolve_page_id;

pub async fn handle(ctx: &AppContext, args: CopyTreeArgs) -> Result<()> {
    let client = crate::context::load_client(ctx)?;
    copy_tree(&client, ctx, args).await
}

#[derive(Debug, Clone)]
struct Node {
    id: String,
    parent_id: Option<String>,
    title: String,
    child_position: i64,
    body_storage: Option<String>,
}

async fn copy_tree(client: &ApiClient, ctx: &AppContext, args: CopyTreeArgs) -> Result<()> {
    let source_id = resolve_page_id(client, &args.source).await?;
    let target_parent_id = resolve_page_id(client, &args.target_parent).await?;

    let exclude = args.exclude.as_deref().map(glob_to_regex_ci).transpose()?;

    // SpaceId: inferred from target parent.
    let target_parent_url = client.v2_url(&format!("/pages/{target_parent_id}"));
    let (target_parent_json, _) = client.get_json(target_parent_url).await?;
    let target_space_id = target_parent_json
        .get("spaceId")
        .and_then(|v| v.as_str())
        .context("Target parent missing spaceId")?
        .to_string();

    // Descendants (no root).
    // NOTE: Confluence's `/pages/{id}/descendants` endpoint appears to only include a limited
    // depth on Cloud. Walk `direct-children` instead so deep trees are copied correctly.
    let max_depth = if args.max_depth == 0 {
        None
    } else {
        Some(args.max_depth)
    };
    let descendants = confcli::tree::fetch_descendants_via_direct_children(
        client, &source_id, 50, true, max_depth,
    )
    .await?;

    let mut nodes: HashMap<String, Node> = HashMap::new();

    // Root node (fetch with storage body).
    let (root_json, root_body) = fetch_page_with_body_format(client, &source_id, "storage").await?;
    let root_title = json_str(&root_json, "title");
    nodes.insert(
        source_id.clone(),
        Node {
            id: source_id.clone(),
            parent_id: None,
            title: root_title,
            child_position: 0,
            body_storage: Some(root_body),
        },
    );

    for item in descendants {
        let id = item
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if id.is_empty() {
            continue;
        }
        let title = item
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let parent_id = item
            .get("parentId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let child_position = item
            .get("childPosition")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        nodes.insert(
            id.clone(),
            Node {
                id,
                parent_id,
                title,
                child_position,
                body_storage: None,
            },
        );
    }

    // Exclusion: skip nodes whose title matches, and their descendants.
    let mut blocked: HashSet<String> = HashSet::new();
    if let Some(re) = &exclude {
        for (id, node) in &nodes {
            if id == &source_id {
                continue;
            }
            if re.is_match(&node.title) {
                blocked.insert(id.clone());
            }
        }
    }
    if !blocked.is_empty() {
        // Propagate to descendants.
        let mut changed = true;
        while changed {
            changed = false;
            for (id, node) in &nodes {
                if blocked.contains(id) {
                    continue;
                }
                if let Some(parent) = &node.parent_id
                    && blocked.contains(parent)
                {
                    blocked.insert(id.clone());
                    changed = true;
                }
            }
        }
    }

    // Build child lists.
    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    for (id, node) in &nodes {
        if blocked.contains(id) {
            continue;
        }
        if let Some(parent) = &node.parent_id {
            children.entry(parent.clone()).or_default().push(id.clone());
        }
    }
    for ids in children.values_mut() {
        ids.sort_by_key(|id| nodes.get(id).map(|n| n.child_position).unwrap_or(0));
    }

    // Fetch bodies for descendants (storage) concurrently.
    let client_arc = Arc::new(client.clone());
    let sem = Arc::new(Semaphore::new(args.concurrency.max(1)));
    let total_to_fetch = nodes
        .iter()
        .filter(|(id, node)| {
            *id != &source_id && !blocked.contains(*id) && node.body_storage.is_none()
        })
        .count();
    let fetch_bar = if ctx.quiet {
        None
    } else {
        let bar = indicatif::ProgressBar::new(total_to_fetch as u64);
        bar.set_style(
            indicatif::ProgressStyle::with_template("{spinner:.green} {pos}/{len} {wide_msg}")
                .unwrap(),
        );
        bar.set_message("page bodies");
        Some(bar)
    };
    let mut tasks = Vec::new();
    for (id, node) in nodes.iter() {
        if id == &source_id {
            continue;
        }
        if blocked.contains(id) {
            continue;
        }
        if node.body_storage.is_some() {
            continue;
        }
        let id = id.clone();
        let client = client_arc.clone();
        let permit = sem.clone().acquire_owned().await?;
        let bar = fetch_bar.clone();
        tasks.push(tokio::spawn(async move {
            let _permit = permit;
            let res = fetch_page_with_body_format(&client, &id, "storage")
                .await
                .map(|(_, body)| (id, body));
            if res.is_ok()
                && let Some(bar) = &bar
            {
                bar.inc(1);
            }
            res
        }));
    }
    for task in tasks {
        let (id, body) = task.await.context("Fetch task failed")??;
        if let Some(node) = nodes.get_mut(&id) {
            node.body_storage = Some(body);
        }
    }
    if let Some(bar) = fetch_bar {
        bar.finish_and_clear();
    }

    // Traversal + create.
    let mut mapping: HashMap<String, String> = HashMap::new();
    let mut created: Vec<Value> = Vec::new();

    #[allow(clippy::too_many_arguments)]
    fn walk<'a>(
        client: &'a ApiClient,
        ctx: &'a AppContext,
        nodes: &'a HashMap<String, Node>,
        children: &'a HashMap<String, Vec<String>>,
        mapping: &'a mut HashMap<String, String>,
        created: &'a mut Vec<Value>,
        source_id: &'a str,
        target_parent_id: &'a str,
        target_space_id: &'a str,
        args: &'a CopyTreeArgs,
        depth: usize,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            if args.max_depth > 0 && depth > args.max_depth {
                return Ok(());
            }

            let node = nodes.get(source_id).context("Missing node")?;
            let new_parent = if depth == 0 {
                target_parent_id.to_string()
            } else {
                let parent_old = node.parent_id.as_ref().context("Missing parentId")?;
                mapping
                    .get(parent_old)
                    .cloned()
                    .context("Missing parent mapping")?
            };

            let title = if depth == 0 {
                args.new_title
                    .clone()
                    .unwrap_or_else(|| format!("{}{}", node.title, args.copy_suffix))
            } else {
                format!("{}{}", node.title, args.copy_suffix)
            };

            if ctx.dry_run {
                let new_parent_display = if depth == 0 {
                    new_parent.clone()
                } else {
                    // In dry-run mode we don't have real IDs for newly-created pages.
                    // Show the source parent id to make the plan easier to read.
                    let parent_old = node.parent_id.as_ref().context("Missing parentId")?;
                    format!("(copy of {parent_old})")
                };

                print_line(
                    ctx,
                    &format!("Would create '{title}' under {new_parent_display}"),
                );
                mapping.insert(node.id.clone(), format!("<dry-run:{}>", node.id));
            } else {
                let body = node.body_storage.as_ref().cloned().unwrap_or_default();
                let payload = json!({
                    "spaceId": target_space_id,
                    "title": title,
                    "parentId": new_parent,
                    "status": "current",
                    "body": { "representation": "storage", "value": body }
                });
                let url = client.v2_url("/pages");
                let result = client.post_json(url, payload).await?;
                let new_id = result
                    .get("id")
                    .and_then(|v| v.as_str())
                    .context("Missing created page id")?
                    .to_string();
                mapping.insert(node.id.clone(), new_id);
                created.push(result);

                if args.delay_ms > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(args.delay_ms)).await;
                }
            }

            if let Some(kids) = children.get(source_id) {
                for kid in kids {
                    walk(
                        client,
                        ctx,
                        nodes,
                        children,
                        mapping,
                        created,
                        kid,
                        target_parent_id,
                        target_space_id,
                        args,
                        depth + 1,
                    )
                    .await?;
                }
            }
            Ok(())
        })
    }

    walk(
        client,
        ctx,
        &nodes,
        &children,
        &mut mapping,
        &mut created,
        &source_id,
        &target_parent_id,
        &target_space_id,
        &args,
        0,
    )
    .await?;

    match args.output {
        OutputFormat::Json => {
            maybe_print_json(ctx, &json!({ "mapping": mapping, "created": created }))
        }
        fmt => {
            let rows = vec![
                vec!["Source".to_string(), source_id.clone()],
                vec!["TargetParent".to_string(), target_parent_id.clone()],
                vec!["Created".to_string(), created.len().to_string()],
            ];
            maybe_print_kv_fmt(ctx, fmt, rows);
            Ok(())
        }
    }
}

fn glob_to_regex_ci(glob: &str) -> Result<Regex> {
    let mut re = String::from("^");
    for ch in glob.chars() {
        match ch {
            '*' => re.push_str(".*"),
            '?' => re.push('.'),
            '.' | '+' | '(' | ')' | '|' | '^' | '$' | '{' | '}' | '[' | ']' | '\\' => {
                re.push('\\');
                re.push(ch);
            }
            _ => re.push(ch),
        }
    }
    re.push('$');
    regex::RegexBuilder::new(&re)
        .case_insensitive(true)
        .build()
        .map_err(|e| anyhow::anyhow!("Invalid glob pattern: {e}"))
}
