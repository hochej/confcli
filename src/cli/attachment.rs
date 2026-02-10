use clap::{Args, Subcommand};
use confcli::output::OutputFormat;
use std::path::PathBuf;

use super::common::parse_positive_limit;

#[derive(Subcommand, Debug)]
pub enum AttachmentCommand {
    #[command(about = "List attachments")]
    List(AttachmentListArgs),
    #[command(about = "Get attachment metadata")]
    Get(AttachmentGetArgs),
    #[command(about = "Download an attachment")]
    Download(AttachmentDownloadArgs),
    #[cfg(feature = "write")]
    #[command(about = "Upload an attachment")]
    Upload(AttachmentUploadArgs),
    #[cfg(feature = "write")]
    #[command(about = "Delete an attachment")]
    Delete(AttachmentDeleteArgs),
}

#[derive(Args, Debug)]
pub struct AttachmentListArgs {
    #[arg(help = "Page id, URL, or SPACE:Title (omit to list all attachments)")]
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

#[derive(Args, Debug)]
pub struct AttachmentGetArgs {
    #[arg(help = "Attachment id")]
    pub attachment: String,
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json, table, or markdown")]
    pub output: OutputFormat,
}

#[derive(Args, Debug)]
pub struct AttachmentDownloadArgs {
    #[arg(help = "Attachment id")]
    pub attachment: String,
    #[arg(long, help = "Destination file path")]
    pub dest: Option<PathBuf>,
}

#[cfg(feature = "write")]
#[derive(Args, Debug)]
pub struct AttachmentUploadArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    pub page: String,
    #[arg(required = true, num_args = 1.., help = "File(s) to upload")]
    pub files: Vec<PathBuf>,
    #[arg(long, help = "Optional attachment comment")]
    pub comment: Option<String>,
    #[arg(
        long,
        default_value = "4",
        value_parser = parse_positive_limit,
        help = "Max concurrent uploads"
    )]
    pub concurrency: usize,
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json, table, or markdown")]
    pub output: OutputFormat,
}

#[cfg(feature = "write")]
#[derive(Args, Debug)]
pub struct AttachmentDeleteArgs {
    #[arg(help = "Attachment id")]
    pub attachment: String,
    #[arg(long, help = "Permanently purge the attachment")]
    pub purge: bool,
    #[arg(short = 'y', long, help = "Skip confirmation prompt")]
    pub yes: bool,
    #[arg(short = 'o', long, help = "Output format: json, table, or markdown")]
    pub output: Option<OutputFormat>,
}
