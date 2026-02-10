use clap::{Args, Subcommand};
use confcli::output::OutputFormat;
#[cfg(feature = "write")]
use std::path::PathBuf;

use super::common::parse_positive_limit;

#[derive(Subcommand, Debug)]
pub enum PageCommand {
    #[command(about = "List pages")]
    List(PageListArgs),
    #[command(about = "Get a page by id, URL, or SPACE:Title")]
    Get(PageGetArgs),
    #[command(about = "Show only the page body (markdown by default)")]
    Body(PageBodyArgs),
    #[cfg(feature = "write")]
    #[command(about = "Edit a page body in $EDITOR")]
    Edit(PageEditArgs),
    #[cfg(feature = "write")]
    #[command(about = "Create a page")]
    Create(PageCreateArgs),
    #[cfg(feature = "write")]
    #[command(about = "Update a page")]
    Update(PageUpdateArgs),
    #[cfg(feature = "write")]
    #[command(about = "Delete a page")]
    Delete(PageDeleteArgs),
    #[command(about = "List children or descendants of a page")]
    Children(PageChildrenArgs),
    #[command(about = "Show page version history")]
    History(PageHistoryArgs),
    #[command(about = "Open a page in the browser")]
    Open(PageOpenArgs),
}

#[derive(Args, Debug)]
pub struct PageListArgs {
    #[arg(long, help = "Filter by space key or id")]
    pub space: Option<String>,
    #[arg(long, help = "Filter by page status")]
    pub status: Option<String>,
    #[arg(long, help = "Filter by page title")]
    pub title: Option<String>,
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json, table, or markdown")]
    pub output: OutputFormat,
    #[arg(short = 'a', long, help = "Fetch all pages of results")]
    pub all: bool,
    #[arg(
        short = 'n',
        long,
        default_value = "50",
        value_parser = parse_positive_limit,
        help = "Maximum number of results"
    )]
    pub limit: usize,
}

#[derive(Args, Debug)]
pub struct PageGetArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    pub page: String,
    #[arg(
        long,
        default_value = "storage",
        help = "Body format: storage, atlas_doc_format, view"
    )]
    pub body_format: String,
    #[arg(long, help = "Fetch a specific version number")]
    pub version: Option<i64>,
    #[arg(long, help = "Preserve empty list items in markdown output")]
    pub keep_empty_list_items: bool,
    #[arg(long, help = "Show the page body in table output (can be very large)")]
    pub show_body: bool,
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: table, json, or markdown")]
    pub output: OutputFormat,
}

#[derive(Args, Debug)]
pub struct PageBodyArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    pub page: String,
    #[arg(long, help = "Preserve empty list items in markdown output")]
    pub keep_empty_list_items: bool,
    #[arg(
        long,
        default_value = "markdown",
        help = "Body format: markdown, view, storage, atlas_doc_format, adf"
    )]
    pub format: String,
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json, table, or markdown (json wraps body in a JSON object)")]
    pub output: OutputFormat,
}

#[cfg(feature = "write")]
#[derive(Args, Debug)]
pub struct PageEditArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    pub page: String,
    #[arg(
        long,
        default_value = "storage",
        help = "Body format to edit: storage or atlas_doc_format (adf)"
    )]
    pub format: String,
    #[arg(long, help = "Show a diff and prompt before saving")]
    pub diff: bool,
    #[arg(short = 'y', long, help = "Skip confirmation prompt")]
    pub yes: bool,
}

#[cfg(feature = "write")]
#[derive(Args, Debug)]
pub struct PageCreateArgs {
    #[arg(long, help = "Space key or id")]
    pub space: String,
    #[arg(long, help = "Page title")]
    pub title: Option<String>,
    #[arg(long, help = "Parent page id, URL, or SPACE:Title")]
    pub parent: Option<String>,
    #[arg(long, help = "Page status: current or draft")]
    pub status: Option<String>,
    #[arg(long, help = "Path to body file, or '-' to read from stdin")]
    pub body_file: Option<PathBuf>,
    #[arg(long, help = "Inline body content (for small pages)")]
    pub body: Option<String>,
    #[arg(
        long,
        default_value = "storage",
        help = "Body format: storage, atlas_doc_format, wiki"
    )]
    pub body_format: String,
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json, table, or markdown")]
    pub output: OutputFormat,
}

#[cfg(feature = "write")]
#[derive(Args, Debug)]
pub struct PageUpdateArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    pub page: String,
    #[arg(long, help = "New title")]
    pub title: Option<String>,
    #[arg(long, help = "New parent page id, URL, or SPACE:Title")]
    pub parent: Option<String>,
    #[arg(long, help = "Status: current or draft")]
    pub status: Option<String>,
    #[arg(long, help = "Path to body file, or '-' to read from stdin")]
    pub body_file: Option<PathBuf>,
    #[arg(long, help = "Inline body content (for small pages)")]
    pub body: Option<String>,
    #[arg(
        long,
        default_value = "storage",
        help = "Body format: storage, atlas_doc_format, wiki"
    )]
    pub body_format: String,
    #[arg(long, help = "Version message")]
    pub message: Option<String>,
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json, table, or markdown")]
    pub output: OutputFormat,
}

#[cfg(feature = "write")]
#[derive(Args, Debug)]
pub struct PageDeleteArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    pub page: String,
    #[arg(long, help = "Permanently purge the page")]
    pub purge: bool,
    #[arg(long, help = "When purging, trash first if needed")]
    pub force: bool,
    #[arg(short = 'y', long, help = "Skip confirmation prompt")]
    pub yes: bool,
    #[arg(short = 'o', long, help = "Output format: json, table, or markdown")]
    pub output: Option<OutputFormat>,
}

#[derive(Args, Debug)]
pub struct PageChildrenArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    pub page: String,
    #[arg(long, help = "List all descendants instead of direct children")]
    pub recursive: bool,
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json, table, or markdown")]
    pub output: OutputFormat,
    #[arg(short = 'a', long, help = "Fetch all pages of results")]
    pub all: bool,
    #[arg(
        short = 'n',
        long,
        default_value = "50",
        value_parser = parse_positive_limit,
        help = "Maximum number of results"
    )]
    pub limit: usize,
}

#[derive(Args, Debug)]
pub struct PageHistoryArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    pub page: String,
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json, table, or markdown")]
    pub output: OutputFormat,
    #[arg(
        short = 'n',
        long,
        default_value = "25",
        value_parser = parse_positive_limit,
        help = "Number of versions to show"
    )]
    pub limit: usize,
}

#[derive(Args, Debug)]
pub struct PageOpenArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    pub page: String,
}
