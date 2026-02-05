use serde_json::Value;

/// Extract a field from a JSON Value as a string, returning "" if missing.
/// Handles both string and numeric values.
pub fn json_str(v: &Value, key: &str) -> String {
    match v.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}
