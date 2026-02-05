use anyhow::Result;
use clap::ValueEnum;
use comfy_table::{presets::UTF8_FULL, ContentArrangement, Table};
use serde::Serialize;

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Table,
    #[value(alias = "md")]
    Markdown,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Table => write!(f, "table"),
            OutputFormat::Markdown => write!(f, "markdown"),
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
