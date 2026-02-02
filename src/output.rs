use anyhow::{bail, Result};
use comfy_table::{presets::UTF8_FULL, ContentArrangement, Table};
use serde::Serialize;

#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    Json,
    Table,
    Markdown,
}

impl OutputFormat {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "json" => Ok(OutputFormat::Json),
            "table" => Ok(OutputFormat::Table),
            "markdown" | "md" => Ok(OutputFormat::Markdown),
            _ => bail!("Invalid output format: {value}. Use json, table, or markdown."),
        }
    }
}

pub fn print_json<T: Serialize>(value: &T) -> Result<()> {
    let data = serde_json::to_string_pretty(value)?;
    println!("{data}");
    Ok(())
}

pub fn print_table(headers: &[&str], rows: Vec<Vec<String>>) {
    if rows.is_empty() {
        println!("No results found.");
        return;
    }
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(headers.to_vec());
    for row in rows {
        table.add_row(row);
    }
    println!("{table}");
}
