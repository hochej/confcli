use anyhow::Result;
use htmd::HtmlToMarkdown;
use regex::Regex;

#[derive(Debug, Clone, Copy, Default)]
pub struct MarkdownOptions {
    pub keep_empty_list_items: bool,
}

pub fn html_to_markdown(html: &str, base_url: &str) -> Result<String> {
    html_to_markdown_with_options(html, base_url, MarkdownOptions::default())
}

pub fn html_to_markdown_with_options(
    html: &str,
    base_url: &str,
    options: MarkdownOptions,
) -> Result<String> {
    let cleaned = preprocess_html(html, base_url)?;
    let markdown = HtmlToMarkdown::new().convert(&cleaned)?;
    let markdown = postprocess_markdown(&markdown, options);
    Ok(markdown.trim().to_string())
}

pub fn decode_unicode_escapes_str(input: &str) -> String {
    decode_unicode_escapes(input)
}

fn preprocess_html(html: &str, base_url: &str) -> Result<String> {
    let mut content = html.to_string();
    let base_root = base_url.trim_end_matches("/wiki");

    let style_re = Regex::new(r"(?s)<style[^>]*>.*?</style>")?;
    content = style_re.replace_all(&content, "").to_string();

    let panel_re = Regex::new(
        r#"(?s)<div class="panel[^"]*"[^>]*>\s*<div class="panelContent[^"]*"[^>]*>(.*?)</div>\s*</div>"#,
    )?;
    content = panel_re
        .replace_all(&content, "<blockquote>$1</blockquote>")
        .to_string();

    let status_re =
        Regex::new(r#"(?s)<span[^>]*class="[^"]*status-macro[^"]*"[^>]*>(.*?)</span>"#)?;
    content = status_re.replace_all(&content, "[$1]").to_string();

    let href_re = Regex::new(r#"href="(/wiki[^"]*)""#)?;
    content = href_re
        .replace_all(&content, format!("href=\"{}$1\"", base_root))
        .to_string();

    let src_re = Regex::new(r#"src="(/wiki[^"]*)""#)?;
    content = src_re
        .replace_all(&content, format!("src=\"{}$1\"", base_root))
        .to_string();

    content = add_image_alt_text(&content);
    content = decode_unicode_escapes(&content);

    Ok(content)
}

fn add_image_alt_text(html: &str) -> String {
    let img_re = Regex::new(r#"<img([^>]*?)(/?)>"#).unwrap();
    let alias_re = Regex::new(r#"data-linked-resource-default-alias="([^"]+)""#).unwrap();
    let src_re = Regex::new(r#"(?:data-image-src|src)="([^"]+)""#).unwrap();

    img_re
        .replace_all(html, |caps: &regex::Captures| {
            let attrs = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let closing = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            if attrs.contains(" alt=") {
                return format!("<img{attrs}{closing}>");
            }
            let alt = if let Some(cap) = alias_re.captures(attrs) {
                cap.get(1)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default()
            } else if let Some(cap) = src_re.captures(attrs) {
                let raw = cap.get(1).map(|m| m.as_str()).unwrap_or("");
                extract_filename(raw)
            } else {
                String::new()
            };
            if alt.is_empty() {
                return format!("<img{attrs}{closing}>");
            }
            let alt = alt.replace('"', "&quot;");
            format!("<img{attrs} alt=\"{alt}\"{closing}>")
        })
        .to_string()
}

fn extract_filename(value: &str) -> String {
    let trimmed = value.split('?').next().unwrap_or(value);
    let name = trimmed.rsplit('/').next().unwrap_or("");
    name.to_string()
}

fn decode_unicode_escapes(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 5 < chars.len() && chars[i + 1] == 'u' {
            if let Some(code) = parse_hex4(&chars[i + 2..i + 6]) {
                if (0xD800..=0xDBFF).contains(&code)
                    && i + 11 < chars.len()
                    && chars[i + 6] == '\\'
                    && chars[i + 7] == 'u'
                {
                    if let Some(low) = parse_hex4(&chars[i + 8..i + 12]) {
                        if (0xDC00..=0xDFFF).contains(&low) {
                            let high_ten = (code - 0xD800) as u32;
                            let low_ten = (low - 0xDC00) as u32;
                            let scalar = 0x10000 + ((high_ten << 10) | low_ten);
                            if let Some(ch) = char::from_u32(scalar) {
                                out.push(ch);
                                i += 12;
                                continue;
                            }
                        }
                    }
                }

                if let Some(ch) = char::from_u32(code as u32) {
                    out.push(ch);
                    i += 6;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn parse_hex4(slice: &[char]) -> Option<u16> {
    if slice.len() != 4 {
        return None;
    }
    let mut value: u16 = 0;
    for ch in slice {
        value = value.checked_mul(16)?;
        let digit = ch.to_digit(16)? as u16;
        value = value.checked_add(digit)?;
    }
    Some(value)
}

fn table_cells(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

fn separator_like_row(line: &str) -> bool {
    let cell_re = Regex::new(r"^:?-{3,}:?$").unwrap();
    let cells = table_cells(line);
    if cells.is_empty() {
        return false;
    }
    cells
        .iter()
        .all(|cell| cell.is_empty() || cell_re.is_match(cell))
}

fn is_image_only_cell(cell: &str) -> bool {
    let image_re = Regex::new(r"^!\[[^\]]*\]\([^)]*\)$").unwrap();
    image_re.is_match(cell.trim())
}

fn postprocess_markdown(markdown: &str, options: MarkdownOptions) -> String {
    let empty_list_re = Regex::new(r"^\s*(?:[-*+]|\d+\.)\s*$").unwrap();
    let table_sep_re = Regex::new(r"^\s*\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)+\|?\s*$").unwrap();

    let mut lines = Vec::new();
    for line in markdown.lines() {
        if !options.keep_empty_list_items && empty_list_re.is_match(line) {
            continue;
        }
        lines.push(line.to_string());
    }

    let mut out = Vec::new();
    let mut idx = 0;
    while idx < lines.len() {
        let line = &lines[idx];
        if line.trim().starts_with('|') {
            let mut block = Vec::new();
            while idx < lines.len() && lines[idx].trim().starts_with('|') {
                block.push(lines[idx].clone());
                idx += 1;
            }
            if !block.is_empty() {
                if block.len() == 2 && separator_like_row(&block[1]) {
                    let cells = table_cells(&block[0]);
                    if cells.len() == 1 && is_image_only_cell(&cells[0]) {
                        out.push(cells[0].clone());
                        continue;
                    }
                }

                let data_rows: Vec<&String> = block
                    .iter()
                    .filter(|row| !table_sep_re.is_match(row.as_str()))
                    .collect();
                if data_rows.len() == 1 {
                    let cells = table_cells(data_rows[0]);
                    if cells.len() == 1 && is_image_only_cell(&cells[0]) {
                        out.push(cells[0].clone());
                        continue;
                    }
                }

                let needs_separator = block.len() < 2
                    || !table_sep_re.is_match(block.get(1).map(String::as_str).unwrap_or(""));
                if needs_separator {
                    let header = block[0].trim().trim_matches('|');
                    let columns = header.split('|').count().max(1);
                    let sep = format!("|{}|", vec![" --- "; columns].join("|"));
                    out.push(block[0].clone());
                    out.push(sep);
                    if block.len() > 1 && separator_like_row(&block[1]) {
                        for row in block.iter().skip(2) {
                            out.push(row.clone());
                        }
                    } else {
                        for row in block.iter().skip(1) {
                            out.push(row.clone());
                        }
                    }
                } else {
                    out.extend(block);
                }
            }
        } else {
            out.push(line.clone());
            idx += 1;
        }
    }

    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_panel_to_blockquote() {
        let html = r#"<div class="panel"><div class="panelContent">Hello</div></div>"#;
        let md = html_to_markdown(html, "https://example.com").unwrap();
        assert_eq!(md.trim(), "> Hello");
    }

    #[test]
    fn strips_style_tags() {
        let html = r#"<style>body { color: red; }</style><p>Hi</p>"#;
        let md = html_to_markdown(html, "https://example.com").unwrap();
        assert_eq!(md.trim(), "Hi");
    }

    #[test]
    fn decodes_unicode_escape_sequences() {
        let html = r#"<p>\uD83D\uDDD3 Date</p>"#;
        let md = html_to_markdown(html, "https://example.com").unwrap();
        assert_eq!(md.trim(), "🗓 Date");
    }

    #[test]
    fn inserts_table_separator() {
        let md = postprocess_markdown(
            "| **Driver** | |\n| **Approver** | |",
            MarkdownOptions::default(),
        );
        assert_eq!(md, "| **Driver** | |\n| --- | --- |\n| **Approver** | |");
    }

    #[test]
    fn replaces_separator_like_row() {
        let md = postprocess_markdown(
            "| **Notes** | |\n| ------------------- | |",
            MarkdownOptions::default(),
        );
        assert_eq!(md, "| **Notes** | |\n| --- | --- |");
    }

    #[test]
    fn collapses_single_image_table() {
        let md = postprocess_markdown("| ![](image.webp) |\n| --- |", MarkdownOptions::default());
        assert_eq!(md, "![](image.webp)");
    }

    #[test]
    fn adds_alt_text_from_alias() {
        let html = r#"<img data-linked-resource-default-alias="diagram.png" src="/wiki/download/diagram.png">"#;
        let md = html_to_markdown(html, "https://example.com/wiki").unwrap();
        assert_eq!(
            md.trim(),
            "![diagram.png](https://example.com/wiki/download/diagram.png)"
        );
    }
}
