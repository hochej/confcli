use anyhow::{Context, Result};
use confcli::client::ApiClient;
use serde_json::Value;
use std::collections::HashMap;
use url::Url;

use crate::helpers::url_with_query;

pub async fn resolve_page_id(client: &ApiClient, page: &str) -> Result<String> {
    if page.chars().all(|c| c.is_ascii_digit()) {
        return Ok(page.to_string());
    }
    if let Ok(url) = Url::parse(page) {
        if let Some(id) = extract_page_id_from_url(&url) {
            return Ok(id);
        }
    }
    if let Some((space, title)) = page.split_once(':') {
        let space_id = resolve_space_id(client, space).await?;
        let url = url_with_query(
            &client.v2_url("/pages"),
            &[
                ("space-id", space_id),
                ("title", title.to_string()),
                ("limit", "1".to_string()),
            ],
        )?;
        let items = client.get_paginated_results(url, false).await?;
        let id = items
            .first()
            .and_then(|item| item.get("id"))
            .and_then(|v| v.as_str())
            .context("Page not found")?;
        return Ok(id.to_string());
    }
    Err(anyhow::anyhow!(
        "Unable to resolve page reference '{page}'. Use a page id, URL, or SPACE:Title."
    ))
}

pub async fn resolve_space_id(client: &ApiClient, space: &str) -> Result<String> {
    if space.chars().all(|c| c.is_ascii_digit()) {
        return Ok(space.to_string());
    }
    let url = client.v2_url(&format!("/spaces?keys={space}&limit=1"));
    let items = client.get_paginated_results(url, false).await?;
    let id = items
        .first()
        .and_then(|item| item.get("id"))
        .and_then(|v| v.as_str())
        .context("Space not found")?;
    Ok(id.to_string())
}

pub async fn resolve_space_key(client: &ApiClient, space_id: &str) -> Result<String> {
    let url = client.v2_url(&format!("/spaces/{}", space_id));
    let (json, _) = client.get_json(url).await?;
    Ok(json
        .get("key")
        .and_then(|v| v.as_str())
        .unwrap_or(space_id)
        .to_string())
}

pub async fn resolve_space_keys(
    client: &ApiClient,
    space_ids: &[String],
) -> Result<HashMap<String, String>> {
    let mut unique: Vec<String> = space_ids.to_vec();
    unique.sort();
    unique.dedup();
    if unique.is_empty() {
        return Ok(HashMap::new());
    }
    let mut map = HashMap::new();
    for chunk in unique.chunks(250) {
        let ids = chunk.join(",");
        let url = client.v2_url(&format!("/spaces?ids={ids}&limit={}", chunk.len()));
        let items = client.get_paginated_results(url, false).await?;
        for item in items {
            if let (Some(id), Some(key)) = (
                item.get("id").and_then(|v| v.as_str()),
                item.get("key").and_then(|v| v.as_str()),
            ) {
                map.insert(id.to_string(), key.to_string());
            }
        }
    }
    Ok(map)
}

pub fn extract_page_id_from_url(url: &Url) -> Option<String> {
    let segments: Vec<&str> = url.path_segments()?.collect();
    if let Some(pos) = segments.iter().position(|seg| *seg == "pages") {
        if let Some(id) = segments.get(pos + 1) {
            if id.chars().all(|c| c.is_ascii_digit()) {
                return Some(id.to_string());
            }
        }
    }
    url.query_pairs()
        .find(|(key, _)| key == "pageId")
        .map(|(_, value)| value.to_string())
}

#[cfg(feature = "write")]
pub async fn page_status(client: &ApiClient, page_id: &str) -> Result<String> {
    let url = client.v2_url(&format!("/pages/{page_id}"));
    let (json, _) = client.get_json(url).await?;
    Ok(json
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("current")
        .to_string())
}

pub fn build_page_tree(items: &[Value]) -> Vec<String> {
    let mut children: HashMap<String, Vec<Value>> = HashMap::new();
    let mut roots = Vec::new();
    for item in items {
        let parent = item.get("parentId").and_then(|v| v.as_str()).unwrap_or("");
        if parent.is_empty() {
            roots.push(item.clone());
        } else {
            children
                .entry(parent.to_string())
                .or_default()
                .push(item.clone());
        }
    }

    let mut lines = Vec::new();
    for root in roots {
        walk_tree(&root, &children, 0, &mut lines);
    }
    lines
}

fn walk_tree(
    node: &Value,
    children: &HashMap<String, Vec<Value>>,
    depth: usize,
    lines: &mut Vec<String>,
) {
    let title = node.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let id = node.get("id").and_then(|v| v.as_str()).unwrap_or("");
    lines.push(format!("{}- {} ({})", "  ".repeat(depth), title, id));
    if let Some(children_nodes) = children.get(id) {
        let mut sorted = children_nodes.clone();
        sorted.sort_by_key(|v| v.get("childPosition").and_then(|p| p.as_i64()).unwrap_or(0));
        for child in sorted {
            walk_tree(&child, children, depth + 1, lines);
        }
    }
}
