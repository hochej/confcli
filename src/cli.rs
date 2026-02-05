use clap::{Args, Parser, Subcommand, ValueEnum};
use confcli::output::OutputFormat;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "confcli",
    version,
    about = "Confluence Cloud CLI (v2-first)",
    long_about = "A fast CLI for Confluence Cloud with v2-first APIs and v1 fallbacks for search, labels, and uploads.",
    after_help = "EXAMPLES:\n  confcli auth login --domain yourcompany.atlassian.net --email you@example.com --token <token>\n  confcli space list --all\n  confcli space pages MFS --tree\n  confcli page get MFS:Overview\n  confcli search \"confluence\"\n  echo '<p>Hello</p>' | confcli page create --space MFS --title Hello --body-file -\n"
)]
pub struct Cli {
    #[arg(
        short = 'q',
        long,
        global = true,
        help = "Suppress non-essential output"
    )]
    pub quiet: bool,
    #[arg(short = 'v', long, global = true, action = clap::ArgAction::Count, help = "Increase verbosity (-v, -vv)")]
    pub verbose: u8,
    #[arg(long, global = true, help = "Show what would happen without executing")]
    pub dry_run: bool,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(subcommand, about = "Manage authentication")]
    Auth(AuthCommand),
    #[command(subcommand, about = "Work with spaces")]
    Space(SpaceCommand),
    #[command(subcommand, about = "Work with pages")]
    Page(PageCommand),
    #[command(about = "Search content (CQL or plain text)")]
    Search(SearchCommand),
    #[command(subcommand, about = "Work with attachments")]
    Attachment(AttachmentCommand),
    #[command(subcommand, about = "Work with labels")]
    Label(LabelCommand),
    #[command(about = "Generate shell completions")]
    Completions(CompletionsArgs),
}

#[derive(Args, Debug)]
pub struct CompletionsArgs {
    #[arg(value_enum, help = "Shell to generate completions for")]
    pub shell: Shell,
}

#[derive(ValueEnum, Debug, Clone)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    #[value(name = "powershell")]
    Pwsh,
}

// --- Auth ---

#[derive(Subcommand, Debug)]
pub enum AuthCommand {
    #[command(about = "Log in and store credentials")]
    Login(AuthLoginArgs),
    #[command(about = "Show current authentication status")]
    Status,
    #[command(about = "Clear stored credentials")]
    Logout,
}

#[derive(Args, Debug)]
pub struct AuthLoginArgs {
    #[arg(
        long,
        env = "CONFLUENCE_DOMAIN",
        help = "Confluence domain (e.g. yourcompany.atlassian.net)"
    )]
    pub domain: Option<String>,
    #[arg(long, env = "CONFLUENCE_EMAIL", help = "Email address for basic auth")]
    pub email: Option<String>,
    #[arg(
        long,
        env = "CONFLUENCE_TOKEN",
        hide_env_values = true,
        help = "API token for basic auth"
    )]
    pub token: Option<String>,
    #[arg(
        long,
        env = "CONFLUENCE_BEARER_TOKEN",
        hide_env_values = true,
        help = "Bearer token for OAuth"
    )]
    pub bearer: Option<String>,
}

// --- Space ---

#[derive(Subcommand, Debug)]
pub enum SpaceCommand {
    #[command(about = "List spaces")]
    List(SpaceListArgs),
    #[command(about = "Get a space by key or id")]
    Get(SpaceGetArgs),
    #[command(about = "List pages in a space")]
    Pages(SpacePagesArgs),
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
        help = "Page size for pagination"
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
        help = "Page size for pagination"
    )]
    pub limit: usize,
}

// --- Page ---

#[derive(Subcommand, Debug)]
pub enum PageCommand {
    #[command(about = "List pages")]
    List(PageListArgs),
    #[command(about = "Get a page by id, URL, or SPACE:Title")]
    Get(PageGetArgs),
    #[command(about = "Show only the page body (markdown by default)")]
    Body(PageBodyArgs),
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
        help = "Page size for pagination"
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
    #[arg(short = 'o', long, default_value_t = OutputFormat::Markdown, help = "Output format: markdown, json, or table")]
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
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json or table")]
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
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json or table")]
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
}

#[derive(Args, Debug)]
pub struct PageChildrenArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    pub page: String,
    #[arg(long, help = "List all descendants instead of direct children")]
    pub recursive: bool,
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json or table")]
    pub output: OutputFormat,
    #[arg(short = 'a', long, help = "Fetch all pages of results")]
    pub all: bool,
    #[arg(
        short = 'n',
        long,
        default_value = "50",
        help = "Page size for pagination"
    )]
    pub limit: usize,
}

#[derive(Args, Debug)]
pub struct PageHistoryArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    pub page: String,
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json or table")]
    pub output: OutputFormat,
    #[arg(
        short = 'n',
        long,
        default_value = "25",
        help = "Number of versions to show"
    )]
    pub limit: usize,
}

#[derive(Args, Debug)]
pub struct PageOpenArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    pub page: String,
}

// --- Search ---

#[derive(Args, Debug)]
#[command(
    about = "Search content with CQL (defaults to text search)",
    after_help = "EXAMPLES:\n  confcli search \"confluence\"\n  confcli search \"type=page AND title ~ \\\"Template\\\"\"\n"
)]
pub struct SearchCommand {
    #[arg(help = "Search query. If no CQL operators are detected, defaults to text ~ \"query\"")]
    pub query: String,
    #[arg(long, help = "Filter by space key")]
    pub space: Option<String>,
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json or table")]
    pub output: OutputFormat,
    #[arg(short = 'a', long, help = "Fetch all pages of results")]
    pub all: bool,
    #[arg(
        short = 'n',
        long,
        default_value = "50",
        help = "Page size for pagination"
    )]
    pub limit: usize,
}

// --- Attachment ---

#[derive(Subcommand, Debug)]
pub enum AttachmentCommand {
    #[command(about = "List attachments")]
    List(AttachmentListArgs),
    #[command(about = "Get attachment metadata")]
    Get(AttachmentGetArgs),
    #[command(about = "Download an attachment")]
    Download(AttachmentDownloadArgs),
    #[cfg(feature = "write")]
    #[command(about = "Upload an attachment (v1 endpoint)")]
    Upload(AttachmentUploadArgs),
    #[cfg(feature = "write")]
    #[command(about = "Delete an attachment")]
    Delete(AttachmentDeleteArgs),
}

#[derive(Args, Debug)]
pub struct AttachmentListArgs {
    #[arg(long, help = "Page id, URL, or SPACE:Title to scope attachments")]
    pub page: Option<String>,
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json or table")]
    pub output: OutputFormat,
    #[arg(short = 'a', long, help = "Fetch all pages of results")]
    pub all: bool,
    #[arg(
        short = 'n',
        long,
        default_value = "50",
        help = "Page size for pagination"
    )]
    pub limit: usize,
}

#[derive(Args, Debug)]
pub struct AttachmentGetArgs {
    #[arg(help = "Attachment id")]
    pub attachment: String,
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json or table")]
    pub output: OutputFormat,
}

#[derive(Args, Debug)]
pub struct AttachmentDownloadArgs {
    #[arg(help = "Attachment id")]
    pub attachment: String,
    #[arg(short, long, help = "Output path")]
    pub output: Option<PathBuf>,
}

#[cfg(feature = "write")]
#[derive(Args, Debug)]
pub struct AttachmentUploadArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    pub page: String,
    #[arg(help = "File to upload")]
    pub file: PathBuf,
    #[arg(long, help = "Optional attachment comment")]
    pub comment: Option<String>,
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json or table")]
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
}

// --- Label ---

#[derive(Subcommand, Debug)]
pub enum LabelCommand {
    #[command(about = "List labels")]
    List(LabelListArgs),
    #[cfg(feature = "write")]
    #[command(about = "Add a label to a page (v1 endpoint)")]
    Add(LabelAddArgs),
    #[cfg(feature = "write")]
    #[command(about = "Remove a label from a page (v1 endpoint)")]
    Remove(LabelRemoveArgs),
    #[command(about = "List pages with a label (v1 search)")]
    Pages(LabelPagesArgs),
}

#[derive(Args, Debug)]
pub struct LabelListArgs {
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json or table")]
    pub output: OutputFormat,
    #[arg(short = 'a', long, help = "Fetch all pages of results")]
    pub all: bool,
    #[arg(
        short = 'n',
        long,
        default_value = "50",
        help = "Page size for pagination"
    )]
    pub limit: usize,
}

#[cfg(feature = "write")]
#[derive(Args, Debug)]
pub struct LabelAddArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    pub page: String,
    #[arg(help = "Label name")]
    pub label: String,
}

#[cfg(feature = "write")]
#[derive(Args, Debug)]
pub struct LabelRemoveArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    pub page: String,
    #[arg(help = "Label name")]
    pub label: String,
}

#[derive(Args, Debug)]
pub struct LabelPagesArgs {
    #[arg(help = "Label name")]
    pub label: String,
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json or table")]
    pub output: OutputFormat,
    #[arg(short = 'a', long, help = "Fetch all pages of results")]
    pub all: bool,
    #[arg(
        short = 'n',
        long,
        default_value = "50",
        help = "Page size for pagination"
    )]
    pub limit: usize,
}
