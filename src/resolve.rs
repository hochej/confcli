use anyhow::{Context, Result};
use confcli::client::ApiClient;
use lru::LruCache;
use serde_json::Value;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::OnceLock;
use tokio::sync::Mutex;
use url::Url;

use crate::helpers::url_with_query;

const SPACE_KEY_CACHE_CAPACITY: usize = 1024;

// Bounded cache to avoid unbounded memory growth in long-running / heavily scripted usage.
// Tokio mutex avoids blocking async runtime worker threads.
static SPACE_KEY_CACHE: OnceLock<Mutex<LruCache<String, String>>> = OnceLock::new();

fn space_key_cache() -> &'static Mutex<LruCache<String, String>> {
    SPACE_KEY_CACHE.get_or_init(|| {
        Mutex::new(LruCache::new(
            NonZeroUsize::new(SPACE_KEY_CACHE_CAPACITY).expect("non-zero cache capacity"),
        ))
    })
}

pub async fn resolve_page_id(client: &ApiClient, page: &str) -> Result<String> {
    if !page.is_empty() && page.chars().all(|c| c.is_ascii_digit()) {
        return Ok(page.to_string());
    }
    if let Ok(url) = Url::parse(page)
        && let Some(id) = extract_page_id_from_url(&url)
    {
        return Ok(id);
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
            .with_context(|| format!("Page '{title}' not found in space {space}"))?;
        return Ok(id.to_string());
    }
    Err(anyhow::anyhow!(
        "Unable to resolve page reference '{page}'. Use a page id, URL, or SPACE:Title."
    ))
}

pub async fn resolve_space_id(client: &ApiClient, space: &str) -> Result<String> {
    if !space.is_empty() && space.chars().all(|c| c.is_ascii_digit()) {
        return Ok(space.to_string());
    }

    // Avoid manual string formatting here: `space` is user input and must be URL-encoded.
    let url = url_with_query(
        &client.v2_url("/spaces"),
        &[("keys", space.to_string()), ("limit", "1".to_string())],
    )?;
    let items = client.get_paginated_results(url, false).await?;
    let id = items
        .first()
        .and_then(|item| item.get("id"))
        .and_then(|v| v.as_str())
        .with_context(|| format!("Space '{space}' not found"))?;
    Ok(id.to_string())
}

pub async fn resolve_space_key(client: &ApiClient, space_id: &str) -> Result<String> {
    // Fast path: serve from cache.
    {
        let mut guard = space_key_cache().lock().await;
        if let Some(key) = guard.get(space_id).cloned() {
            return Ok(key);
        }
    }

    let url = client.v2_url(&format!("/spaces/{}", space_id));
    let (json, _) = client.get_json(url).await?;
    let key = json
        .get("key")
        .and_then(|v| v.as_str())
        .unwrap_or(space_id)
        .to_string();

    {
        let mut guard = space_key_cache().lock().await;
        guard.put(space_id.to_string(), key.clone());
    }

    Ok(key)
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

    // Serve from cache when possible, without holding the lock across awaits.
    let mut out: HashMap<String, String> = HashMap::new();
    let mut missing: Vec<String> = Vec::new();
    {
        let mut guard = space_key_cache().lock().await;
        for id in &unique {
            if let Some(key) = guard.get(id).cloned() {
                out.insert(id.clone(), key);
            } else {
                missing.push(id.clone());
            }
        }
    }

    if missing.is_empty() {
        return Ok(out);
    }

    let mut fetched = HashMap::new();
    for chunk in missing.chunks(250) {
        let ids = chunk.join(",");
        let url = client.v2_url(&format!("/spaces?ids={ids}&limit={}", chunk.len()));
        let items = client.get_paginated_results(url, false).await?;
        for item in items {
            if let (Some(id), Some(key)) = (
                item.get("id").and_then(|v| v.as_str()),
                item.get("key").and_then(|v| v.as_str()),
            ) {
                let display = if key.starts_with('~') {
                    item.get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or(key)
                        .to_string()
                } else {
                    key.to_string()
                };
                fetched.insert(id.to_string(), display);
            }
        }
    }

    // Update cache.
    {
        let mut guard = space_key_cache().lock().await;
        for (id, key) in &fetched {
            guard.put(id.clone(), key.clone());
        }
    }

    out.extend(fetched);
    Ok(out)
}

pub fn extract_page_id_from_url(url: &Url) -> Option<String> {
    if let Some(segments) = url.path_segments() {
        let mut iter = segments;
        while let Some(seg) = iter.next() {
            if seg == "pages"
                && let Some(id) = iter.next()
                && !id.is_empty()
                && id.chars().all(|c| c.is_ascii_digit())
            {
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
    #[derive(Debug, Clone)]
    struct NodeView {
        id: String,
        parent_id: String,
        title: String,
        child_position: i64,
    }

    // Avoid cloning full JSON blobs into the tree structure; we only need a few fields.
    let mut roots: Vec<NodeView> = Vec::new();
    let mut children: HashMap<String, Vec<NodeView>> = HashMap::new();

    for item in items {
        let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if id.is_empty() {
            continue;
        }
        let parent_id = item
            .get("parentId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let title = item
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let child_position = item
            .get("childPosition")
            .and_then(|p| p.as_i64())
            .unwrap_or(0);

        let view = NodeView {
            id: id.to_string(),
            parent_id,
            title,
            child_position,
        };

        if view.parent_id.is_empty() {
            roots.push(view);
        } else {
            children
                .entry(view.parent_id.clone())
                .or_default()
                .push(view);
        }
    }

    // Keep output stable/predictable.
    roots.sort_by_key(|n| n.child_position);
    for kids in children.values_mut() {
        kids.sort_by_key(|n| n.child_position);
    }

    // Iterative traversal to avoid deep recursion on large trees.
    let mut lines: Vec<String> = Vec::new();
    let mut stack: Vec<(NodeView, usize)> = Vec::new();

    for root in roots.into_iter().rev() {
        stack.push((root, 0));
    }

    while let Some((node, depth)) = stack.pop() {
        lines.push(format!(
            "{}- {} ({})",
            "  ".repeat(depth),
            node.title,
            node.id
        ));

        if let Some(kids) = children.get(&node.id) {
            for kid in kids.iter().cloned().rev() {
                stack.push((kid, depth + 1));
            }
        }
    }

    lines
}
