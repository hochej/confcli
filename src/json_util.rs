use serde_json::Value;

/// Extract a string field from a JSON Value, returning "" if missing or non-string.
pub fn json_str(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}
