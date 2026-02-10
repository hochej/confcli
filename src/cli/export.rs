use clap::Args;
use confcli::output::OutputFormat;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct ExportArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    pub page: String,
    #[arg(long, default_value = ".", help = "Destination directory")]
    pub dest: PathBuf,
    #[arg(long, default_value = "md", help = "Content format: md, storage, adf")]
    pub format: String,
    #[arg(long, help = "Only export attachments matching this glob (e.g. *.png)")]
    pub pattern: Option<String>,
    #[arg(long, help = "Skip downloading attachments")]
    pub skip_attachments: bool,
    #[arg(
        long,
        default_value = "4",
        help = "Max concurrent attachment downloads"
    )]
    pub concurrency: usize,
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json, table, or markdown")]
    pub output: OutputFormat,
}
