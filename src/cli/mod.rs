use clap::{Args, Parser, Subcommand, ValueEnum};

mod attachment;
mod auth;
mod comment;
mod common;
#[cfg(feature = "write")]
mod copy_tree;
mod export;
mod label;
mod page;
mod search;
mod space;

pub use attachment::*;
pub use auth::*;
pub use comment::*;
#[cfg(feature = "write")]
pub use copy_tree::*;
pub use export::*;
pub use label::*;
pub use page::*;
pub use search::*;
pub use space::*;

#[cfg(feature = "write")]
const CLI_AFTER_HELP: &str = "EXAMPLES:\n  confcli auth login --domain yourcompany.atlassian.net --email you@example.com --token <token>\n  confcli space list --all\n  confcli space pages MFS --tree\n  confcli page get MFS:Overview\n  confcli search \"confluence\"\n  echo '<p>Hello</p>' | confcli page create --space MFS --title Hello --body-file -\n";

#[cfg(not(feature = "write"))]
const CLI_AFTER_HELP: &str = "EXAMPLES:\n  confcli auth login --domain yourcompany.atlassian.net --email you@example.com --token <token>\n  confcli space list --all\n  confcli space pages MFS --tree\n  confcli page get MFS:Overview\n  confcli search \"confluence\"\n";

#[cfg(feature = "write")]
const PAGE_ABOUT: &str = "List, view, create, and manage pages";
#[cfg(not(feature = "write"))]
const PAGE_ABOUT: &str = "List and view pages";

#[cfg(feature = "write")]
const ATTACHMENT_ABOUT: &str = "List, download, upload, and manage attachments";
#[cfg(not(feature = "write"))]
const ATTACHMENT_ABOUT: &str = "List, download, and inspect attachments";

#[cfg(feature = "write")]
const LABEL_ABOUT: &str = "List, add, and remove page labels";
#[cfg(not(feature = "write"))]
const LABEL_ABOUT: &str = "List page labels";

#[cfg(feature = "write")]
const COMMENT_ABOUT: &str = "List, add, and delete comments";
#[cfg(not(feature = "write"))]
const COMMENT_ABOUT: &str = "List comments";

#[derive(Parser, Debug)]
#[command(
    name = "confcli",
    version,
    about = "A scrappy little Confluence CLI for you and your clanker",
    after_help = CLI_AFTER_HELP
)]
pub struct Cli {
    #[arg(short = 'q', long, global = true, help = "Suppress all output")]
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
    #[command(subcommand, about = PAGE_ABOUT)]
    Page(PageCommand),
    #[command(about = "Search content (CQL or plain text)")]
    Search(SearchCommand),
    #[command(subcommand, about = ATTACHMENT_ABOUT)]
    Attachment(AttachmentCommand),
    #[command(subcommand, about = LABEL_ABOUT)]
    Label(LabelCommand),
    #[command(subcommand, about = COMMENT_ABOUT)]
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
