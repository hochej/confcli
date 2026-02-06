use clap::{Args, Parser, Subcommand, ValueEnum};
use confcli::output::OutputFormat;
use std::path::PathBuf;

fn parse_space_key(s: &str) -> Result<String, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("space key cannot be empty".to_string());
    }
    if s.len() < 2 || s.len() > 32 {
        return Err("space key must be 2-32 characters".to_string());
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_uppercase() {
        return Err("space key must start with an uppercase letter (A-Z)".to_string());
    }
    if !chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
        return Err("space key must contain only A-Z and 0-9".to_string());
    }
    Ok(s.to_string())
}

#[derive(Parser, Debug)]
#[command(
    name = "confcli",
    version,
    about = "A scrappy little Confluence CLI for you and your clanker",
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
    #[command(subcommand, about = "List and inspect spaces")]
    Space(SpaceCommand),
    #[command(subcommand, about = "List, view, create, and manage pages")]
    Page(PageCommand),
    #[command(about = "Search content (CQL or plain text)")]
    Search(SearchCommand),
    #[command(subcommand, about = "List, download, upload, and manage attachments")]
    Attachment(AttachmentCommand),
    #[command(subcommand, about = "List, add, and remove page labels")]
    Label(LabelCommand),
    #[command(subcommand, about = "List, add, and delete comments")]
    Comment(CommentCommand),
    #[command(about = "Export a page and its attachments to a folder")]
    Export(ExportArgs),
    #[cfg(feature = "write")]
    #[command(about = "Copy a page tree to a new parent")]
    CopyTree(CopyTreeArgs),
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
        help = "Confluence site URL or domain (e.g. yourcompany.atlassian.net or https://yourcompany.atlassian.net/wiki)"
    )]
    pub domain: Option<String>,
    #[arg(long, env = "CONFLUENCE_EMAIL", help = "Email address for basic auth")]
    pub email: Option<String>,
    #[arg(
        long,
        env = "CONFLUENCE_TOKEN",
        hide_env_values = true,
        help = "API token for basic auth (also accepts CONFLUENCE_API_TOKEN)"
    )]
    pub token: Option<String>,
    #[arg(
        long,
        env = "CONFLUENCE_API_PATH",
        help = "Override v1 API path (e.g. /wiki/rest/api or /rest/api)"
    )]
    pub api_path: Option<String>,
    #[arg(long, help = "Override v2 API path (e.g. /wiki/api/v2 or /api/v2)")]
    pub api_v2_path: Option<String>,
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
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json or table")]
    pub output: OutputFormat,
}

#[cfg(feature = "write")]
#[derive(Args, Debug)]
pub struct SpaceDeleteArgs {
    #[arg(help = "Space key or id")]
    pub space: String,
    #[arg(short = 'y', long, help = "Skip confirmation prompt")]
    pub yes: bool,
    #[arg(short = 'o', long, help = "Output format: json or table")]
    pub output: Option<OutputFormat>,
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
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json or table (json wraps body in a JSON object)")]
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
    #[arg(short = 'o', long, help = "Output format: json or table")]
    pub output: Option<OutputFormat>,
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
        help = "Maximum number of results"
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
    about = "Search content (CQL or plain text)",
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
        help = "Maximum number of results"
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
        help = "Maximum number of results"
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
    #[arg(short = 'o', long, help = "Output format: json or table")]
    pub output: Option<OutputFormat>,
}

// --- Label ---

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
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json or table")]
    pub output: OutputFormat,
    #[arg(short = 'a', long, help = "Fetch all pages of results")]
    pub all: bool,
    #[arg(
        short = 'n',
        long,
        default_value = "50",
        help = "Maximum number of results"
    )]
    pub limit: usize,
}

// --- Comment ---

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
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json or table")]
    pub output: OutputFormat,
    #[arg(short = 'a', long, help = "Fetch all pages of results")]
    pub all: bool,
    #[arg(
        short = 'n',
        long,
        default_value = "25",
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
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json or table")]
    pub output: OutputFormat,
}

#[cfg(feature = "write")]
#[derive(Args, Debug)]
pub struct CommentDeleteArgs {
    #[arg(help = "Comment id")]
    pub comment: String,
    #[arg(short = 'y', long, help = "Skip confirmation prompt")]
    pub yes: bool,
    #[arg(short = 'o', long, help = "Output format: json or table")]
    pub output: Option<OutputFormat>,
}

// --- Export ---

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
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json or table")]
    pub output: OutputFormat,
}

// --- Copy Tree ---

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
    #[arg(short = 'o', long, default_value_t = OutputFormat::Table, help = "Output format: json or table")]
    pub output: OutputFormat,
}
