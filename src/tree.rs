use crate::client::ApiClient;
use anyhow::Result;
use serde_json::{Number, Value};
use std::collections::{HashSet, VecDeque};
use url::Url;

/// Fetch all descendants of a page by recursively walking the `direct-children` endpoint.
///
/// Why this exists: Confluence's `/pages/{id}/descendants` endpoint appears to only include a
/// limited depth (at least on Cloud), which breaks deep trees.
///
/// The returned items are the raw `direct-children` items, augmented with:
/// - `parentId`: the parent page id
/// - `depth`: 1-based depth relative to the root (children of root are depth=1)
///
/// Notes on `limit`/`all`:
/// - `limit` is used as the API page size.
/// - If `all == false`, the function will stop after collecting `limit` total descendants.
///
/// `max_depth` limits traversal depth (root depth=0). `None` or `Some(0)` means unlimited.
pub async fn fetch_descendants_via_direct_children(
    client: &ApiClient,
    root_id: &str,
    limit: usize,
    all: bool,
    max_depth: Option<usize>,
) -> Result<Vec<Value>> {
    let mut out: Vec<Value> = Vec::new();
    let mut q: VecDeque<(String, usize)> = VecDeque::new();
    let mut seen: HashSet<String> = HashSet::new();

    q.push_back((root_id.to_string(), 0));
    seen.insert(root_id.to_string());

    let unlimited_depth = max_depth.unwrap_or(0) == 0;

    while let Some((parent_id, depth)) = q.pop_front() {
        if !unlimited_depth {
            let max_depth = max_depth.unwrap_or(0);
            if depth >= max_depth {
                // Don't expand further.
                continue;
            }
        }

        if !all {
            // Global cap on total results when not paginating.
            if out.len() >= limit {
                break;
            }
        }

        let page_size = if all {
            limit.max(1)
        } else {
            // Try to not fetch more than we still plan to return.
            (limit.saturating_sub(out.len())).max(1)
        };

        let url = with_query(
            &client.v2_url(&format!("/pages/{parent_id}/direct-children")),
            &[("limit", page_size.to_string())],
        )?;

        let children = client.get_paginated_results(url, all).await?;
        for mut child in children {
            let id = child
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if id.is_empty() {
                continue;
            }

            if let Some(obj) = child.as_object_mut() {
                obj.insert("parentId".to_string(), Value::String(parent_id.clone()));
                obj.insert(
                    "depth".to_string(),
                    Value::Number(Number::from((depth + 1) as u64)),
                );
            }

            out.push(child);

            if seen.insert(id.clone()) {
                q.push_back((id, depth + 1));
            }

            if !all && out.len() >= limit {
                break;
            }
        }
    }

    Ok(out)
}

fn with_query(base: &str, params: &[(&str, String)]) -> Result<String> {
    let mut url = Url::parse(base)?;
    {
        let mut qp = url.query_pairs_mut();
        for (k, v) in params {
            qp.append_pair(k, v);
        }
    }
    Ok(url.to_string())
}
