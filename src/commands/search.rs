use anyhow::Result;
use confcli::client::ApiClient;
use confcli::json_util::json_str;
use confcli::output::OutputFormat;
use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

use crate::cli::SearchCommand;
use crate::context::AppContext;
use crate::helpers::{maybe_print_json, maybe_print_rows, url_with_query};

pub async fn handle(ctx: &AppContext, cmd: SearchCommand) -> Result<()> {
    if cmd.query.trim().is_empty() {
        return Err(anyhow::anyhow!("Search query cannot be empty"));
    }
    let client = crate::context::load_client(ctx)?;
    let mut cql = to_cql_query(&cmd.query);
    if let Some(space) = cmd.space {
        // Always quote + escape the space key to avoid CQL injection and to support keys like "~user".
        let space = escape_cql_text(&space);
        cql = format!("space = \"{space}\" AND ({cql})");
    }
    if cmd.all {
        let results = search_all(&client, &cql, cmd.limit).await?;
        match cmd.output {
            OutputFormat::Json => maybe_print_json(ctx, &results),
            fmt => {
                let rows = results.iter().map(search_result_row).collect();
                maybe_print_rows(ctx, fmt, &["ID", "Type", "Space", "Title"], rows);
                Ok(())
            }
        }
    } else {
        let url = url_with_query(
            &client.v1_url("/search"),
            &[("cql", cql), ("limit", cmd.limit.to_string())],
        )?;
        let (json, _) = client.get_json(url).await?;
        match cmd.output {
            OutputFormat::Json => maybe_print_json(ctx, &json),
            fmt => {
                let results = json
                    .get("results")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                let rows = results.iter().map(search_result_row).collect();
                maybe_print_rows(ctx, fmt, &["ID", "Type", "Space", "Title"], rows);
                Ok(())
            }
        }
    }
}

fn search_result_row(item: &Value) -> Vec<String> {
    let content = item.get("content").cloned().unwrap_or(Value::Null);
    let space = content
        .get("space")
        .and_then(|s| s.get("key"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            let container = item.get("resultGlobalContainer")?;
            let key = container
                .get("displayUrl")
                .and_then(|v| v.as_str())
                .and_then(|url| url.rsplit('/').next())
                .unwrap_or("");
            if key.starts_with('~') {
                container
                    .get("title")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            } else if !key.is_empty() {
                Some(key.to_string())
            } else {
                container
                    .get("title")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            }
        })
        .unwrap_or_default();
    vec![
        json_str(&content, "id"),
        json_str(&content, "type"),
        space,
        json_str(&content, "title"),
    ]
}

fn escape_cql_text(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\n', '\r', '\t'], " ")
}

static CQL_KEYWORD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(AND|OR|NOT|IN)\b").unwrap());

static CQL_FIELD_OP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\w+\s*[=~!<>]").unwrap());

fn to_cql_query(query: &str) -> String {
    let has_keyword = CQL_KEYWORD_RE.is_match(query);
    let has_field_op = CQL_FIELD_OP_RE.is_match(query);
    let has_parens = query.contains('(') && query.contains(')');

    if has_keyword || has_field_op || has_parens {
        query.to_string()
    } else {
        format!("text ~ \"{}\"", escape_cql_text(query))
    }
}

/// Paginate through all v1 search results using offset-based pagination.
///
/// Note: The v1 search API uses offset-based pagination (`start` parameter).
/// Under concurrent modifications, results may be duplicated or skipped as
/// content shifts between pages. There is no cursor-based alternative in v1.
async fn search_all(client: &ApiClient, cql: &str, limit: usize) -> Result<Vec<Value>> {
    if limit == 0 {
        return Err(anyhow::anyhow!("--limit must be at least 1"));
    }

    const MAX_PAGES: usize = 10_000;

    let mut start = 0usize;
    let mut pages = 0usize;
    let mut results = Vec::new();
    loop {
        pages += 1;
        if pages > MAX_PAGES {
            return Err(anyhow::anyhow!(
                "Search pagination aborted after {MAX_PAGES} pages (possible looping server response)"
            ));
        }
        let url = url_with_query(
            &client.v1_url("/search"),
            &[
                ("cql", cql.to_string()),
                ("limit", limit.to_string()),
                ("start", start.to_string()),
            ],
        )?;
        let (json, _) = client.get_json(url).await?;
        let page = json
            .get("results")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let page_len = page.len();
        if page_len == 0 {
            break;
        }
        results.extend(page);
        if page_len < limit {
            break;
        }
        start += limit;
    }
    Ok(results)
}
