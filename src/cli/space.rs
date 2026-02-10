use clap::{Args, Subcommand};
use confcli::output::OutputFormat;

use super::common::parse_positive_limit;
#[cfg(feature = "write")]
use super::common::parse_space_key;

#[derive(Subcommand, Debug)]
pub enum SpaceCommand {
    #[command(about = "List spaces")]
    List(SpaceListArgs),
    #[command(about = "Get a space by key or id")]
    Get(SpaceGetArgs),
    #[command(about = "List pages in a space")]
    Pages(SpacePagesArgs),
    #[cfg(feature = "write")]
    #[command(about = "Create a space")]
    Create(SpaceCreateArgs),
    #[cfg(feature = "write")]
    #[command(about = "Delete a space")]
    Delete(SpaceDeleteArgs),
}

#[derive(Args, Debug)]
pub struct SpaceListArgs {
    #[arg(long, help = "Filter by space keys (comma-separated)")]
    pub keys: Option<String>,
    #[arg(long, help = "Filter by space type")]
    pub r#type: Option<String>,
    #[arg(long, help = "Filter by space status")]
    pub status: Option<String>,
    #[arg(long, help = "Filter by labels (comma-separated)")]
    pub labels: Option<String>,
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
pub struct SpaceGetArgs {
    #[arg(help = "Space key or id")]
    pub space: String,
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json, table, or markdown")]
    pub output: OutputFormat,
}

#[derive(Args, Debug)]
pub struct SpacePagesArgs {
    #[arg(help = "Space key or id")]
    pub space: String,
    #[arg(long, default_value = "all", help = "Depth filter: all or root")]
    pub depth: String,
    #[arg(long, help = "Render a tree view")]
    pub tree: bool,
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

#[cfg(feature = "write")]
#[derive(Args, Debug)]
pub struct SpaceCreateArgs {
    #[arg(long, value_parser = parse_space_key, help = "Space key (uppercase letters/numbers, e.g. PROJ)")]
    pub key: String,
    #[arg(long, help = "Space name")]
    pub name: String,
    #[arg(long, help = "Space description")]
    pub description: Option<String>,
    #[arg(
        long,
        help = "When outputting JSON, print a small human-friendly object instead of the full API response"
    )]
    pub compact_json: bool,
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json, table, or markdown")]
    pub output: OutputFormat,
}

#[cfg(feature = "write")]
#[derive(Args, Debug)]
pub struct SpaceDeleteArgs {
    #[arg(help = "Space key or id")]
    pub space: String,
    #[arg(short = 'y', long, help = "Skip confirmation prompt")]
    pub yes: bool,
    #[arg(short = 'o', long, help = "Output format: json, table, or markdown")]
    pub output: Option<OutputFormat>,
}
