use clap::Args;
use confcli::output::OutputFormat;

use super::common::parse_positive_limit;

#[derive(Args, Debug)]
#[command(
    about = "Search content (CQL or plain text)",
    after_help = "EXAMPLES:\n  confcli search \"confluence\"\n  confcli search \"type=page AND title ~ \\\"Template\\\"\"\n"
)]
pub struct SearchCommand {
    #[arg(help = "Search query. If no CQL operators are detected, defaults to text ~ \"query\"")]
    pub query: String,
    #[arg(long, help = "Filter by space key")]
    pub space: Option<String>,
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
