use clap::Args;
use confcli::output::OutputFormat;

#[cfg(feature = "write")]
#[derive(Args, Debug)]
pub struct CopyTreeArgs {
    #[arg(help = "Source page id, URL, or SPACE:Title")]
    pub source: String,
    #[arg(help = "Target parent page id, URL, or SPACE:Title")]
    pub target_parent: String,
    #[arg(help = "Optional new title for the root copy")]
    pub new_title: Option<String>,
    #[arg(
        long,
        default_value = " (Copy)",
        help = "Suffix appended to copied page titles"
    )]
    pub copy_suffix: String,
    #[arg(
        long,
        help = "Exclude pages whose titles match this glob (case-insensitive)"
    )]
    pub exclude: Option<String>,
    #[arg(long, default_value = "0", help = "Max depth to copy (0 = unlimited)")]
    pub max_depth: usize,
    #[arg(long, default_value = "0", help = "Delay between create requests (ms)")]
    pub delay_ms: u64,
    #[arg(
        long,
        default_value = "8",
        help = "Max concurrent fetches for source bodies"
    )]
    pub concurrency: usize,
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json, table, or markdown")]
    pub output: OutputFormat,
}
