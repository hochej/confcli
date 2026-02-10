use clap::{Args, Subcommand};
use confcli::output::OutputFormat;

use super::common::parse_positive_limit;

#[derive(Subcommand, Debug)]
pub enum LabelCommand {
    #[command(about = "List labels")]
    List(LabelListArgs),
    #[cfg(feature = "write")]
    #[command(about = "Add a label to a page")]
    Add(LabelAddArgs),
    #[cfg(feature = "write")]
    #[command(about = "Remove a label from a page")]
    Remove(LabelRemoveArgs),
    #[command(about = "List pages with a label")]
    Pages(LabelPagesArgs),
}

#[derive(Args, Debug)]
pub struct LabelListArgs {
    #[arg(help = "Page id, URL, or SPACE:Title (omit to list all labels)")]
    pub page: Option<String>,
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
pub struct LabelAddArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    pub page: String,
    #[arg(required = true, num_args = 1.., help = "Label name(s)")]
    pub labels: Vec<String>,
}

#[cfg(feature = "write")]
#[derive(Args, Debug)]
pub struct LabelRemoveArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    pub page: String,
    #[arg(required = true, num_args = 1.., help = "Label name(s)")]
    pub labels: Vec<String>,
}

#[derive(Args, Debug)]
pub struct LabelPagesArgs {
    #[arg(help = "Label name")]
    pub label: String,
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
