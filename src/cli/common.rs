#[cfg(feature = "write")]
pub(super) fn parse_space_key(s: &str) -> Result<String, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("space key cannot be empty".to_string());
    }
    if s.len() < 2 || s.len() > 32 {
        return Err("space key must be 2-32 characters".to_string());
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_uppercase() {
        return Err("space key must start with an uppercase letter (A-Z)".to_string());
    }
    if !chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
        return Err("space key must contain only A-Z and 0-9".to_string());
    }
    Ok(s.to_string())
}

pub(super) fn parse_positive_limit(s: &str) -> Result<usize, String> {
    let value = s
        .trim()
        .parse::<usize>()
        .map_err(|_| "limit must be a positive integer".to_string())?;
    if value == 0 {
        return Err("limit must be at least 1".to_string());
    }
    Ok(value)
}
