use anyhow::Result;
use clap::ValueEnum;
use comfy_table::{Attribute, Cell, ContentArrangement, Table, presets::NOTHING};
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
        .load_preset(NOTHING)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(
            headers
                .iter()
                .map(|h| Cell::new(h).add_attribute(Attribute::Bold))
                .collect::<Vec<_>>(),
        );
    for row in rows {
        table.add_row(row);
    }
    if let Some(col) = table.column_mut(0) {
        col.set_padding((0, 1));
    }
    print_trimmed(&table);
}

pub fn print_table_with_count(headers: &[&str], rows: Vec<Vec<String>>) {
    let count = rows.len();
    print_table(headers, rows);
    if count > 0 {
        let label = if count == 1 { "result" } else { "results" };
        println!("\x1b[2m{count} {label}\x1b[0m");
    }
}

pub fn print_kv(rows: Vec<Vec<String>>) {
    if rows.is_empty() {
        return;
    }
    let mut table = Table::new();
    table
        .load_preset(NOTHING)
        .set_content_arrangement(ContentArrangement::Dynamic);
    for row in rows {
        let mut cells = row.into_iter();
        let mut cell_row = Vec::new();
        if let Some(key) = cells.next() {
            cell_row.push(Cell::new(key).add_attribute(Attribute::Bold));
        }
        for val in cells {
            cell_row.push(Cell::new(val));
        }
        table.add_row(cell_row);
    }
    if let Some(col) = table.column_mut(0) {
        col.set_padding((0, 1));
    }
    print_trimmed(&table);
}

fn print_trimmed(table: &Table) {
    for line in table.to_string().lines() {
        println!("{}", line.trim_end());
    }
}

// --- Markdown output ---

fn escape_md_cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

pub fn print_markdown_table(headers: &[&str], rows: Vec<Vec<String>>) {
    if rows.is_empty() {
        println!("No results found.");
        return;
    }
    let header_line = format!("| {} |", headers.join(" | "));
    let sep_line = format!(
        "| {} |",
        headers
            .iter()
            .map(|_| "---")
            .collect::<Vec<_>>()
            .join(" | ")
    );
    println!("{header_line}");
    println!("{sep_line}");
    for row in rows {
        let escaped: Vec<String> = row.iter().map(|c| escape_md_cell(c)).collect();
        println!("| {} |", escaped.join(" | "));
    }
}

pub fn print_markdown_table_with_count(headers: &[&str], rows: Vec<Vec<String>>) {
    let count = rows.len();
    print_markdown_table(headers, rows);
    if count > 0 {
        let label = if count == 1 { "result" } else { "results" };
        println!("\n*{count} {label}*");
    }
}

pub fn print_markdown_kv(rows: Vec<Vec<String>>) {
    for row in rows {
        if row.len() >= 2 {
            println!(
                "**{}** {}",
                escape_md_cell(&row[0]),
                escape_md_cell(&row[1])
            );
        }
    }
}
