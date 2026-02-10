use clap::{Args, Subcommand};
use confcli::output::OutputFormat;
#[cfg(feature = "write")]
use std::path::PathBuf;

use super::common::parse_positive_limit;

#[derive(Subcommand, Debug)]
pub enum CommentCommand {
    #[command(about = "List comments on a page")]
    List(CommentListArgs),
    #[cfg(feature = "write")]
    #[command(about = "Add a comment to a page")]
    Add(CommentAddArgs),
    #[cfg(feature = "write")]
    #[command(about = "Delete a comment")]
    Delete(CommentDeleteArgs),
}

#[derive(Args, Debug)]
pub struct CommentListArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    pub page: String,
    #[arg(
        long,
        help = "Filter by location: footer, inline, resolved (comma-separated)"
    )]
    pub location: Option<String>,
    #[arg(
        long,
        help = "Confluence expand fields (advanced). Defaults to a minimal set suitable for list output."
    )]
    pub expand: Option<String>,
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json, table, or markdown")]
    pub output: OutputFormat,
    #[arg(short = 'a', long, help = "Fetch all pages of results")]
    pub all: bool,
    #[arg(
        short = 'n',
        long,
        default_value = "25",
        value_parser = parse_positive_limit,
        help = "Maximum number of results"
    )]
    pub limit: usize,
}

#[cfg(feature = "write")]
#[derive(Args, Debug)]
pub struct CommentAddArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    pub page: String,
    #[arg(help = "Comment body text (alternative to --body/--body-file)")]
    pub message: Option<String>,
    #[arg(long, help = "Reply to an existing comment id")]
    pub parent: Option<String>,
    #[arg(long, help = "Comment location: footer or inline")]
    pub location: Option<String>,
    #[arg(
        long,
        help = "Inline properties JSON for inline comments (best-effort)"
    )]
    pub inline_properties: Option<String>,
    #[arg(long, help = "Path to body file, or '-' to read from stdin")]
    pub body_file: Option<PathBuf>,
    #[arg(long, help = "Inline body content (for small comments)")]
    pub body: Option<String>,
    #[arg(
        long,
        default_value = "storage",
        help = "Body format: storage, html, markdown"
    )]
    pub body_format: String,
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json, table, or markdown")]
    pub output: OutputFormat,
}

#[cfg(feature = "write")]
#[derive(Args, Debug)]
pub struct CommentDeleteArgs {
    #[arg(help = "Comment id")]
    pub comment: String,
    #[arg(short = 'y', long, help = "Skip confirmation prompt")]
    pub yes: bool,
    #[arg(short = 'o', long, help = "Output format: json, table, or markdown")]
    pub output: Option<OutputFormat>,
}
