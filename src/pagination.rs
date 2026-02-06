use reqwest::header::HeaderMap;

pub fn next_link_from_headers(headers: &HeaderMap) -> Option<String> {
    let link = headers.get("link")?.to_str().ok()?;
    for part in link.split(',') {
        let trimmed = part.trim();
        let url_start = trimmed.find('<')?;
        let url_end = trimmed.find('>')?;
        let url = &trimmed[url_start + 1..url_end];
        let rel = trimmed[url_end + 1..].trim();
        if rel.contains("rel=\"next\"") || rel.contains("rel=next") {
            return Some(url.to_string());
        }
    }
    None
}

pub fn next_link_from_body(value: &serde_json::Value) -> Option<String> {
    value
        .get("_links")
        .and_then(|links| links.get("next"))
        .and_then(|next| next.as_str())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;

    #[test]
    fn parses_next_link_from_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "link",
            HeaderValue::from_static(
                "<https://example.com/api?page=2>; rel=next, <https://example.com/api?page=5>; rel=last",
            ),
        );
        assert_eq!(
            next_link_from_headers(&headers),
            Some("https://example.com/api?page=2".to_string())
        );
    }

    #[test]
    fn returns_none_when_no_next_link() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "link",
            HeaderValue::from_static("<https://example.com/api?page=5>; rel=last"),
        );
        assert_eq!(next_link_from_headers(&headers), None);
    }
}
