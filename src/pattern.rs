use anyhow::Result;
use regex::Regex;

/// Convert a simple glob pattern (`*` / `?`) into a case-insensitive regex.
///
/// This intentionally supports only a small, predictable subset of glob syntax.
pub fn glob_to_regex_ci(glob: &str) -> Result<Regex> {
    let mut re = String::from("^");
    for ch in glob.chars() {
        match ch {
            '*' => re.push_str(".*"),
            '?' => re.push('.'),
            // Regex metacharacters that must be escaped.
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
