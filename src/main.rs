use anyhow::{Context, Result};
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use confcli::auth::AuthMethod;
use confcli::client::ApiClient;
use confcli::config::Config;
use confcli::markdown::{decode_unicode_escapes_str, html_to_markdown_with_options, MarkdownOptions};
use confcli::output::{print_json, print_table, OutputFormat};
use dialoguer::{Confirm, Input, Password};
use humansize::{format_size, BINARY};
use indicatif::{ProgressBar, ProgressStyle};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::io;
use url::Url;

#[derive(Parser, Debug)]
#[command(name = "confcli", version, about = "Confluence Cloud CLI (v2-first)", long_about = "A fast CLI for Confluence Cloud with v2-first APIs and v1 fallbacks for search, labels, and uploads.", after_help = "EXAMPLES:\n  confcli auth login --domain yourcompany.atlassian.net --email you@example.com --token <token>\n  confcli space list --all\n  confcli space pages MFS --tree\n  confcli page get MFS:Overview\n  confcli search \"confluence\"\n  echo '<p>Hello</p>' | confcli page create --space MFS --title Hello --body-file -\n")]
struct Cli {
    #[arg(short = 'q', long, global = true, help = "Suppress non-essential output")]
    quiet: bool,
    #[arg(short = 'v', long, global = true, action = clap::ArgAction::Count, help = "Increase verbosity (-v, -vv)")]
    verbose: u8,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Clone, Copy)]
struct AppContext {
    quiet: bool,
    verbose: u8,
}

#[derive(Subcommand, Debug)]
enum Commands {
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
struct CompletionsArgs {
    #[arg(value_enum, help = "Shell to generate completions for")]
    shell: Shell,
}

#[derive(ValueEnum, Debug, Clone)]
enum Shell {
    Bash,
    Zsh,
    Fish,
    #[value(name = "powershell")]
    Pwsh,
}

#[derive(Subcommand, Debug)]
enum AuthCommand {
    #[command(about = "Log in and store credentials")]
    Login(AuthLoginArgs),
    #[command(about = "Show current authentication status")]
    Status,
    #[command(about = "Clear stored credentials")]
    Logout,
}

#[derive(Args, Debug)]
struct AuthLoginArgs {
    #[arg(long, env = "CONFLUENCE_DOMAIN", help = "Confluence domain (e.g. yourcompany.atlassian.net)")]
    domain: Option<String>,
    #[arg(long, env = "CONFLUENCE_EMAIL", help = "Email address for basic auth")]
    email: Option<String>,
    #[arg(long, env = "CONFLUENCE_TOKEN", hide_env_values = true, help = "API token for basic auth")]
    token: Option<String>,
    #[arg(long, env = "CONFLUENCE_BEARER_TOKEN", hide_env_values = true, help = "Bearer token for OAuth")]
    bearer: Option<String>,
}

#[derive(Subcommand, Debug)]
enum SpaceCommand {
    #[command(about = "List spaces")]
    List(SpaceListArgs),
    #[command(about = "Get a space by key or id")]
    Get(SpaceGetArgs),
    #[command(about = "List pages in a space")]
    Pages(SpacePagesArgs),
}

#[derive(Args, Debug)]
struct SpaceListArgs {
    #[arg(long, help = "Filter by space keys (comma-separated)")]
    keys: Option<String>,
    #[arg(long, help = "Filter by space type")]
    r#type: Option<String>,
    #[arg(long, help = "Filter by space status")]
    status: Option<String>,
    #[arg(long, help = "Filter by labels (comma-separated)")]
    labels: Option<String>,
    #[arg(short = 'o', long, default_value = "table", help = "Output format: json or table")]
    output: String,
    #[arg(short = 'a', long, help = "Fetch all pages of results")]
    all: bool,
    #[arg(short = 'n', long, default_value = "50", help = "Page size for pagination")]
    limit: usize,
}

#[derive(Args, Debug)]
struct SpaceGetArgs {
    #[arg(help = "Space key or id")]
    space: String,
    #[arg(short = 'o', long, default_value = "table", help = "Output format: json or table")]
    output: String,
}

#[derive(Args, Debug)]
struct SpacePagesArgs {
    #[arg(help = "Space key or id")]
    space: String,
    #[arg(long, default_value = "all", help = "Depth filter: all or root")]
    depth: String,
    #[arg(long, help = "Render a tree view")]
    tree: bool,
    #[arg(long, help = "Filter by page status")]
    status: Option<String>,
    #[arg(long, help = "Filter by page title")]
    title: Option<String>,
    #[arg(short = 'o', long, default_value = "table", help = "Output format: json or table")]
    output: String,
    #[arg(short = 'a', long, help = "Fetch all pages of results")]
    all: bool,
    #[arg(short = 'n', long, default_value = "50", help = "Page size for pagination")]
    limit: usize,
}

#[derive(Subcommand, Debug)]
enum PageCommand {
    #[command(about = "List pages")]
    List(PageListArgs),
    #[command(about = "Get a page by id, URL, or SPACE:Title")]
    Get(PageGetArgs),
    #[command(about = "Show only the page body (markdown by default)")]
    Body(PageBodyArgs),
    #[command(about = "Create a page")]
    Create(PageCreateArgs),
    #[command(about = "Update a page")]
    Update(PageUpdateArgs),
    #[command(about = "Delete a page")]
    Delete(PageDeleteArgs),
    #[command(about = "List children or descendants of a page")]
    Children(PageChildrenArgs),
}

#[derive(Args, Debug)]
struct PageListArgs {
    #[arg(long, help = "Filter by space key or id")]
    space: Option<String>,
    #[arg(long, help = "Filter by page status")]
    status: Option<String>,
    #[arg(long, help = "Filter by page title")]
    title: Option<String>,
    #[arg(short = 'o', long, default_value = "table", help = "Output format: json or table")]
    output: String,
    #[arg(short = 'a', long, help = "Fetch all pages of results")]
    all: bool,
    #[arg(short = 'n', long, default_value = "50", help = "Page size for pagination")]
    limit: usize,
}

#[derive(Args, Debug)]
struct PageGetArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    page: String,
    #[arg(long, default_value = "storage", help = "Body format: storage, atlas_doc_format, view")]
    body_format: String,
    #[arg(long, help = "Fetch a specific version number")]
    version: Option<i64>,
    #[arg(long, help = "Preserve empty list items in markdown output")]
    keep_empty_list_items: bool,
    #[arg(short = 'o', long, default_value = "markdown", help = "Output format: markdown, json, or table")]
    output: String,
}

#[derive(Args, Debug)]
struct PageBodyArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    page: String,
    #[arg(long, help = "Preserve empty list items in markdown output")]
    keep_empty_list_items: bool,
    #[arg(long, default_value = "markdown", help = "Body format: markdown, view, storage, atlas_doc_format, adf")]
    format: String,
}

#[derive(Args, Debug)]
struct PageCreateArgs {
    #[arg(long, help = "Space key or id")]
    space: String,
    #[arg(long, help = "Page title")]
    title: Option<String>,
    #[arg(long, help = "Parent page id, URL, or SPACE:Title")]
    parent: Option<String>,
    #[arg(long, help = "Page status: current or draft")]
    status: Option<String>,
    #[arg(long, help = "Path to body file, or '-' to read from stdin")]
    body_file: Option<PathBuf>,
    #[arg(long, help = "Inline body content (for small pages)")]
    body: Option<String>,
    #[arg(long, default_value = "storage", help = "Body format: storage, atlas_doc_format, wiki")]
    body_format: String,
    #[arg(short = 'o', long, default_value = "table", help = "Output format: json or table")]
    output: String,
}

#[derive(Args, Debug)]
struct PageUpdateArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    page: String,
    #[arg(long, help = "New title")]
    title: Option<String>,
    #[arg(long, help = "New parent page id, URL, or SPACE:Title")]
    parent: Option<String>,
    #[arg(long, help = "Status: current or draft")]
    status: Option<String>,
    #[arg(long, help = "Path to body file, or '-' to read from stdin")]
    body_file: Option<PathBuf>,
    #[arg(long, help = "Inline body content (for small pages)")]
    body: Option<String>,
    #[arg(long, default_value = "storage", help = "Body format: storage, atlas_doc_format, wiki")]
    body_format: String,
    #[arg(long, help = "Version message")]
    message: Option<String>,
    #[arg(short = 'o', long, default_value = "table", help = "Output format: json or table")]
    output: String,
}

#[derive(Args, Debug)]
struct PageDeleteArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    page: String,
    #[arg(long, help = "Permanently purge the page")]
    purge: bool,
    #[arg(long, help = "When purging, trash first if needed")]
    force: bool,
    #[arg(short = 'y', long, help = "Skip confirmation prompt")]
    yes: bool,
}

#[derive(Args, Debug)]
struct PageChildrenArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    page: String,
    #[arg(long, help = "List all descendants instead of direct children")]
    recursive: bool,
    #[arg(short = 'o', long, default_value = "table", help = "Output format: json or table")]
    output: String,
    #[arg(short = 'a', long, help = "Fetch all pages of results")]
    all: bool,
    #[arg(short = 'n', long, default_value = "50", help = "Page size for pagination")]
    limit: usize,
}

#[derive(Args, Debug)]
#[command(about = "Search content with CQL (defaults to text search)", after_help = "EXAMPLES:\n  confcli search \"confluence\"\n  confcli search \"type=page AND title ~ \\\"Template\\\"\"\n")]
struct SearchCommand {
    #[arg(help = "Search query. If no CQL operators are detected, defaults to text ~ \"query\"")]
    query: String,
    #[arg(long, help = "Filter by space key")]
    space: Option<String>,
    #[arg(short = 'o', long, default_value = "table", help = "Output format: json or table")]
    output: String,
    #[arg(short = 'a', long, help = "Fetch all pages of results")]
    all: bool,
    #[arg(short = 'n', long, default_value = "50", help = "Page size for pagination")]
    limit: usize,
}

#[derive(Subcommand, Debug)]
enum AttachmentCommand {
    #[command(about = "List attachments")]
    List(AttachmentListArgs),
    #[command(about = "Get attachment metadata")]
    Get(AttachmentGetArgs),
    #[command(about = "Download an attachment")]
    Download(AttachmentDownloadArgs),
    #[command(about = "Upload an attachment (v1 endpoint)")]
    Upload(AttachmentUploadArgs),
    #[command(about = "Delete an attachment")]
    Delete(AttachmentDeleteArgs),
}

#[derive(Args, Debug)]
struct AttachmentListArgs {
    #[arg(long, help = "Page id, URL, or SPACE:Title to scope attachments")]
    page: Option<String>,
    #[arg(short = 'o', long, default_value = "table", help = "Output format: json or table")]
    output: String,
    #[arg(short = 'a', long, help = "Fetch all pages of results")]
    all: bool,
    #[arg(short = 'n', long, default_value = "50", help = "Page size for pagination")]
    limit: usize,
}

#[derive(Args, Debug)]
struct AttachmentGetArgs {
    #[arg(help = "Attachment id")]
    attachment: String,
    #[arg(short = 'o', long, default_value = "table", help = "Output format: json or table")]
    output: String,
}

#[derive(Args, Debug)]
struct AttachmentDownloadArgs {
    #[arg(help = "Attachment id")]
    attachment: String,
    #[arg(short, long, help = "Output path")]
    output: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct AttachmentUploadArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    page: String,
    #[arg(help = "File to upload")]
    file: PathBuf,
    #[arg(long, help = "Optional attachment comment")]
    comment: Option<String>,
    #[arg(short = 'o', long, default_value = "table", help = "Output format: json or table")]
    output: String,
}

#[derive(Args, Debug)]
struct AttachmentDeleteArgs {
    #[arg(help = "Attachment id")]
    attachment: String,
    #[arg(long, help = "Permanently purge the attachment")]
    purge: bool,
    #[arg(short = 'y', long, help = "Skip confirmation prompt")]
    yes: bool,
}

#[derive(Subcommand, Debug)]
enum LabelCommand {
    #[command(about = "List labels")]
    List(LabelListArgs),
    #[command(about = "Add a label to a page (v1 endpoint)")]
    Add(LabelAddArgs),
    #[command(about = "Remove a label from a page (v1 endpoint)")]
    Remove(LabelRemoveArgs),
    #[command(about = "List pages with a label (v1 search)")]
    Pages(LabelPagesArgs),
}

#[derive(Args, Debug)]
struct LabelListArgs {
    #[arg(short = 'o', long, default_value = "table", help = "Output format: json or table")]
    output: String,
    #[arg(short = 'a', long, help = "Fetch all pages of results")]
    all: bool,
    #[arg(short = 'n', long, default_value = "50", help = "Page size for pagination")]
    limit: usize,
}

#[derive(Args, Debug)]
struct LabelAddArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    page: String,
    #[arg(help = "Label name")]
    label: String,
}

#[derive(Args, Debug)]
struct LabelRemoveArgs {
    #[arg(help = "Page id, URL, or SPACE:Title")]
    page: String,
    #[arg(help = "Label name")]
    label: String,
}

#[derive(Args, Debug)]
struct LabelPagesArgs {
    #[arg(help = "Label name")]
    label: String,
    #[arg(short = 'o', long, default_value = "table", help = "Output format: json or table")]
    output: String,
    #[arg(short = 'a', long, help = "Fetch all pages of results")]
    all: bool,
    #[arg(short = 'n', long, default_value = "50", help = "Page size for pagination")]
    limit: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();
    let ctx = AppContext {
        quiet: cli.quiet,
        verbose: cli.verbose,
    };

    let result = match cli.command {
        Commands::Auth(cmd) => handle_auth(&ctx, cmd).await,
        Commands::Space(cmd) => handle_space(&ctx, cmd).await,
        Commands::Page(cmd) => handle_page(&ctx, cmd).await,
        Commands::Search(cmd) => handle_search(&ctx, cmd).await,
        Commands::Attachment(cmd) => handle_attachment(&ctx, cmd).await,
        Commands::Label(cmd) => handle_label(&ctx, cmd).await,
        Commands::Completions(args) => generate_completions(args),
    };

    if let Err(err) = result {
        if ctx.verbose > 0 {
            eprintln!("{err:?}");
        } else {
            eprintln!("{err}");
        }
        std::process::exit(1);
    }

    Ok(())
}

fn generate_completions(args: CompletionsArgs) -> Result<()> {
    let mut cmd = Cli::command();
    let shell = match args.shell {
        Shell::Bash => clap_complete::Shell::Bash,
        Shell::Zsh => clap_complete::Shell::Zsh,
        Shell::Fish => clap_complete::Shell::Fish,
        Shell::Pwsh => clap_complete::Shell::PowerShell,
    };
    clap_complete::generate(shell, &mut cmd, "confcli", &mut io::stdout());
    Ok(())
}

async fn handle_auth(ctx: &AppContext, cmd: AuthCommand) -> Result<()> {
    match cmd {
        AuthCommand::Login(args) => auth_login(ctx, args).await,
        AuthCommand::Status => auth_status(ctx).await,
        AuthCommand::Logout => {
            Config::clear()?;
            print_line(ctx, "Logged out.");
            Ok(())
        }
    }
}

async fn auth_login(ctx: &AppContext, args: AuthLoginArgs) -> Result<()> {
    let domain = if let Some(domain) = args.domain {
        domain
    } else {
        Input::new()
            .with_prompt("Confluence domain (e.g. yourcompany.atlassian.net)")
            .interact_text()?
    };
    let base_url = format!("https://{domain}/wiki");

    let auth = if let Some(token) = args.bearer {
        AuthMethod::Bearer { token }
    } else {
        let email = if let Some(email) = args.email {
            email
        } else {
            Input::new().with_prompt("Email").interact_text()?
        };
        let token = if let Some(token) = args.token {
            token
        } else {
            Password::new()
                .with_prompt("API token")
                .with_confirmation("Confirm token", "Tokens do not match")
                .interact()?
        };
        AuthMethod::Basic { email, token }
    };

    let config = Config { base_url, auth };
    let client = ApiClient::new(config.base_url.clone(), config.auth.clone(), ctx.verbose)?;
    let url = client.v2_url("/spaces?limit=1");
    client
        .get_json(url)
        .await
        .context("Failed to validate credentials")?;
    config.save()?;
    print_line(ctx, "Saved credentials.");
    Ok(())
}

async fn auth_status(ctx: &AppContext) -> Result<()> {
    if let Some(config) = Config::from_env()? {
        let client = ApiClient::new(config.base_url.clone(), config.auth.clone(), ctx.verbose)?;
        let url = client.v2_url("/spaces?limit=1");
        client
            .get_json(url)
            .await
            .context("Failed to validate auth")?;
        print_line(
            ctx,
            &format!(
                "Logged in to {} using {} auth (from env)",
                config.base_url,
                config.auth.description()
            ),
        );
        return Ok(());
    }

    if !Config::exists()? {
        print_line(ctx, "Not logged in.");
        return Ok(());
    }
    let config = Config::load()?;
    let client = ApiClient::new(config.base_url.clone(), config.auth.clone(), ctx.verbose)?;
    let url = client.v2_url("/spaces?limit=1");
    let _ = client.get_json(url).await.context("Failed to validate auth")?;
    let path = Config::path()?;
    print_line(
        ctx,
        &format!(
            "Logged in to {} using {} auth (config: {})",
            config.base_url,
            config.auth.description(),
            path.display()
        ),
    );
    Ok(())
}

async fn handle_space(ctx: &AppContext, cmd: SpaceCommand) -> Result<()> {
    let client = load_client(ctx)?;
    match cmd {
        SpaceCommand::List(args) => space_list(&client, ctx, args).await,
        SpaceCommand::Get(args) => space_get(&client, ctx, args).await,
        SpaceCommand::Pages(args) => space_pages(&client, ctx, args).await,
    }
}

async fn space_list(client: &ApiClient, ctx: &AppContext, args: SpaceListArgs) -> Result<()> {
    let mut params = vec![format!("limit={}", args.limit)];
    if let Some(keys) = args.keys {
        params.push(format!("keys={keys}"));
    }
    if let Some(space_type) = args.r#type {
        params.push(format!("type={space_type}"));
    }
    if let Some(status) = args.status {
        params.push(format!("status={status}"));
    }
    if let Some(labels) = args.labels {
        params.push(format!("labels={labels}"));
    }
    let url = client.v2_url(&format!("/spaces?{}", params.join("&")));
    let items = client.get_paginated_results(url, args.all).await?;
    let output = OutputFormat::parse(&args.output)?;
    match output {
        OutputFormat::Json => maybe_print_json(ctx, &items),
        OutputFormat::Table => {
            let rows = items
                .iter()
                .map(|item| {
                    vec![
                        item.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        item.get("key").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        item.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        item.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    ]
                })
                .collect();
            maybe_print_table(ctx, &["ID", "Key", "Name", "Type", "Status"], rows);
            Ok(())
        }
        OutputFormat::Markdown => markdown_not_supported(),
    }
}

async fn space_get(client: &ApiClient, ctx: &AppContext, args: SpaceGetArgs) -> Result<()> {
    let space_id = resolve_space_id(client, &args.space).await?;
    let url = client.v2_url(&format!("/spaces/{space_id}"));
    let (json, _) = client.get_json(url).await?;
    let output = OutputFormat::parse(&args.output)?;
    match output {
        OutputFormat::Json => maybe_print_json(ctx, &json),
        OutputFormat::Table => {
            let rows = vec![
                vec!["ID".to_string(), json.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string()],
                vec!["Key".to_string(), json.get("key").and_then(|v| v.as_str()).unwrap_or("").to_string()],
                vec!["Name".to_string(), json.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string()],
                vec!["Type".to_string(), json.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string()],
                vec!["Status".to_string(), json.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string()],
            ];
            maybe_print_table(ctx, &["Field", "Value"], rows);
            Ok(())
        }
        OutputFormat::Markdown => markdown_not_supported(),
    }
}

async fn space_pages(client: &ApiClient, ctx: &AppContext, args: SpacePagesArgs) -> Result<()> {
    let space_id = resolve_space_id(client, &args.space).await?;
    let mut params = vec![format!("limit={}", args.limit), format!("depth={}", args.depth)];
    if let Some(status) = args.status {
        params.push(format!("status={status}"));
    }
    if let Some(title) = args.title {
        params.push(format!("title={}", urlencoding::encode(&title)));
    }
    let url = client.v2_url(&format!("/spaces/{space_id}/pages?{}", params.join("&")));
    let items = client.get_paginated_results(url, args.all).await?;

    if args.tree {
        let output = OutputFormat::parse(&args.output)?;
        match output {
            OutputFormat::Json => maybe_print_json(ctx, &items),
            OutputFormat::Table => {
                let tree = build_page_tree(&items);
                for line in tree {
                    println!("{line}");
                }
                Ok(())
            }
            OutputFormat::Markdown => markdown_not_supported(),
        }
    } else {
        let output = OutputFormat::parse(&args.output)?;
        match output {
            OutputFormat::Json => maybe_print_json(ctx, &items),
            OutputFormat::Table => {
                let rows = items
                    .iter()
                    .map(|item| {
                        vec![
                            item.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            item.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            item.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            item.get("parentId").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        ]
                    })
                    .collect();
                maybe_print_table(ctx, &["ID", "Title", "Status", "Parent"], rows);
                Ok(())
            }
            OutputFormat::Markdown => markdown_not_supported(),
        }
    }
}

async fn handle_page(ctx: &AppContext, cmd: PageCommand) -> Result<()> {
    let client = load_client(ctx)?;
    match cmd {
        PageCommand::List(args) => page_list(&client, ctx, args).await,
        PageCommand::Get(args) => page_get(&client, ctx, args).await,
        PageCommand::Body(args) => page_body(&client, ctx, args).await,
        PageCommand::Create(args) => page_create(&client, ctx, args).await,
        PageCommand::Update(args) => page_update(&client, ctx, args).await,
        PageCommand::Delete(args) => page_delete(&client, ctx, args).await,
        PageCommand::Children(args) => page_children(&client, ctx, args).await,
    }
}

async fn page_list(client: &ApiClient, ctx: &AppContext, args: PageListArgs) -> Result<()> {
    let mut params = vec![format!("limit={}", args.limit)];
    if let Some(space) = args.space {
        let space_id = resolve_space_id(client, &space).await?;
        params.push(format!("space-id={space_id}"));
    }
    if let Some(status) = args.status {
        params.push(format!("status={status}"));
    }
    if let Some(title) = args.title {
        params.push(format!("title={}", urlencoding::encode(&title)));
    }
    let url = client.v2_url(&format!("/pages?{}", params.join("&")));
    let items = client.get_paginated_results(url, args.all).await?;
    let output = OutputFormat::parse(&args.output)?;
    match output {
        OutputFormat::Json => maybe_print_json(ctx, &items),
        OutputFormat::Table => {
            let space_ids: Vec<String> = items
                .iter()
                .filter_map(|item| item.get("spaceId").and_then(|v| v.as_str()).map(|s| s.to_string()))
                .collect();
            let space_keys = resolve_space_keys(client, &space_ids).await?;
            let rows = items
                .iter()
                .map(|item| {
                    let space_id = item.get("spaceId").and_then(|v| v.as_str()).unwrap_or("");
                    let space_key = space_keys.get(space_id).cloned().unwrap_or_else(|| space_id.to_string());
                    vec![
                        item.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        item.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        space_key,
                        item.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    ]
                })
                .collect();
            maybe_print_table(ctx, &["ID", "Title", "Space", "Status"], rows);
            Ok(())
        }
        OutputFormat::Markdown => markdown_not_supported(),
    }
}

async fn page_get(client: &ApiClient, ctx: &AppContext, args: PageGetArgs) -> Result<()> {
    let page_id = resolve_page_id(client, &args.page).await?;
    let mut url = client.v2_url(&format!("/pages/{page_id}?body-format={}", args.body_format));
    if let Some(version) = args.version {
        url.push_str(&format!("&version={version}"));
    }
    let (json, _) = client.get_json(url).await?;
    let output = OutputFormat::parse(&args.output)?;
    match output {
        OutputFormat::Json => maybe_print_json(ctx, &json),
        OutputFormat::Table => {
            let space_id = json.get("spaceId").and_then(|v| v.as_str()).unwrap_or("");
            let space_key = resolve_space_key(client, space_id).await.unwrap_or_else(|_| space_id.to_string());
            let rows = vec![
                vec!["ID".to_string(), json.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string()],
                vec!["Title".to_string(), json.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string()],
                vec!["Space".to_string(), space_key],
                vec!["Status".to_string(), json.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string()],
                vec!["Parent".to_string(), json.get("parentId").and_then(|v| v.as_str()).unwrap_or("").to_string()],
            ];
            maybe_print_table(ctx, &["Field", "Value"], rows);
            Ok(())
        }
        OutputFormat::Markdown => {
            let view_url = client.v2_url(&format!("/pages/{page_id}?body-format=view"));
            let (view_json, _) = client.get_json(view_url).await?;
            let html = view_json
                .get("body")
                .and_then(|body| body.get("view"))
                .and_then(|view| view.get("value"))
                .and_then(|value| value.as_str())
                .context("Missing view body content")?;
            let markdown = html_to_markdown_with_options(
                html,
                client.base_url(),
                MarkdownOptions {
                    keep_empty_list_items: args.keep_empty_list_items,
                },
            )?;
            let with_header = add_markdown_header(client.base_url(), &view_json, &markdown);
            println!("{with_header}");
            Ok(())
        }
    }
}

async fn page_body(client: &ApiClient, _ctx: &AppContext, args: PageBodyArgs) -> Result<()> {
    let page_id = resolve_page_id(client, &args.page).await?;
    let format = args.format.to_lowercase();
    match format.as_str() {
        "markdown" | "md" => {
            let url = client.v2_url(&format!("/pages/{page_id}?body-format=view"));
            let (json, _) = client.get_json(url).await?;
            let html = json
                .get("body")
                .and_then(|body| body.get("view"))
                .and_then(|view| view.get("value"))
                .and_then(|value| value.as_str())
                .context("Missing view body content")?;
            let markdown = html_to_markdown_with_options(
                html,
                client.base_url(),
                MarkdownOptions {
                    keep_empty_list_items: args.keep_empty_list_items,
                },
            )?;
            let with_header = add_markdown_header(client.base_url(), &json, &markdown);
            println!("{with_header}");
            Ok(())
        }
        "view" => {
            let url = client.v2_url(&format!("/pages/{page_id}?body-format=view"));
            let (json, _) = client.get_json(url).await?;
            let html = json
                .get("body")
                .and_then(|body| body.get("view"))
                .and_then(|view| view.get("value"))
                .and_then(|value| value.as_str())
                .context("Missing view body content")?;
            let decoded = decode_unicode_escapes_str(html);
            println!("{decoded}");
            Ok(())
        }
        "storage" => {
            let url = client.v2_url(&format!("/pages/{page_id}?body-format=storage"));
            let (json, _) = client.get_json(url).await?;
            let body = json
                .get("body")
                .and_then(|body| body.get("storage"))
                .and_then(|storage| storage.get("value"))
                .and_then(|value| value.as_str())
                .context("Missing storage body content")?;
            println!("{body}");
            Ok(())
        }
        "atlas_doc_format" | "adf" => {
            let url = client.v2_url(&format!("/pages/{page_id}?body-format=atlas_doc_format"));
            let (json, _) = client.get_json(url).await?;
            let body = json
                .get("body")
                .and_then(|body| body.get("atlas_doc_format"))
                .and_then(|adf| adf.get("value"))
                .and_then(|value| value.as_str())
                .context("Missing ADF body content")?;
            match serde_json::from_str::<serde_json::Value>(body) {
                Ok(value) => print_json(&value)?,
                Err(_) => println!("{body}"),
            }
            Ok(())
        }
        _ => Err(anyhow::anyhow!(
            "Invalid body format: {}. Use markdown, view, storage, atlas_doc_format, or adf.",
            args.format
        )),
    }
}

async fn page_create(client: &ApiClient, ctx: &AppContext, args: PageCreateArgs) -> Result<()> {
    let space_id = resolve_space_id(client, &args.space).await?;
    let body = read_body(args.body, args.body_file.as_ref()).await?;
    let title = match args.title {
        Some(title) => title,
        None => derive_title_from_file(args.body_file.as_ref())
            .context("Title is required when reading from stdin")?,
    };
    let mut payload = json!({
        "spaceId": space_id,
        "title": title,
        "body": { "representation": args.body_format, "value": body },
        "status": args.status.unwrap_or_else(|| "current".to_string()),
    });
    if let Some(parent) = args.parent {
        let parent_id = resolve_page_id(client, &parent).await?;
        payload["parentId"] = Value::String(parent_id);
    }
    let url = client.v2_url("/pages");
    let result = client.post_json(url, payload).await?;
    let output = OutputFormat::parse(&args.output)?;
    match output {
        OutputFormat::Json => maybe_print_json(ctx, &result),
        OutputFormat::Table => {
            let space_key = resolve_space_key(client, result.get("spaceId").and_then(|v| v.as_str()).unwrap_or("")).await.unwrap_or_else(|_| "".to_string());
            let webui = result
                .get("_links")
                .and_then(|v| v.get("webui"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let rows = vec![
                vec!["ID".to_string(), result.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string()],
                vec!["Title".to_string(), result.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string()],
                vec!["Space".to_string(), space_key],
                vec!["Web".to_string(), webui.to_string()],
            ];
            maybe_print_table(ctx, &["Field", "Value"], rows);
            Ok(())
        }
        OutputFormat::Markdown => markdown_not_supported(),
    }
}

async fn page_update(client: &ApiClient, ctx: &AppContext, args: PageUpdateArgs) -> Result<()> {
    let page_id = resolve_page_id(client, &args.page).await?;
    let url = client.v2_url(&format!("/pages/{page_id}"));
    let (current, _) = client.get_json(url.clone()).await?;
    let current_version = current
        .get("version")
        .and_then(|v| v.get("number"))
        .and_then(|v| v.as_i64())
        .context("Missing current version number")?;
    let title = args
        .title
        .or_else(|| current.get("title").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .context("Title is required")?;
    let status = args
        .status
        .or_else(|| current.get("status").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .unwrap_or_else(|| "current".to_string());

    let body = if args.body.is_none() && args.body_file.is_none() {
        let body_url = client.v2_url(&format!("/pages/{page_id}?body-format={}", args.body_format));
        let (current_body, _) = client.get_json(body_url).await?;
        current_body
            .get("body")
            .and_then(|body| body.get(&args.body_format))
            .and_then(|body| body.get("value"))
            .and_then(|value| value.as_str())
            .context("Missing body content for update")?
            .to_string()
    } else {
        read_body(args.body, args.body_file.as_ref()).await?
    };

    let mut payload = json!({
        "id": page_id,
        "title": title,
        "status": status,
        "body": { "representation": args.body_format, "value": body },
        "version": { "number": current_version + 1 }
    });
    if let Some(message) = args.message {
        payload["version"]["message"] = Value::String(message);
    }
    if let Some(parent) = args.parent {
        let parent_id = resolve_page_id(client, &parent).await?;
        payload["parentId"] = Value::String(parent_id);
    }
    let result = client.put_json(url, payload).await?;
    let output = OutputFormat::parse(&args.output)?;
    match output {
        OutputFormat::Json => maybe_print_json(ctx, &result),
        OutputFormat::Table => {
            let webui = result
                .get("_links")
                .and_then(|v| v.get("webui"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let rows = vec![
                vec!["ID".to_string(), result.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string()],
                vec!["Title".to_string(), result.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string()],
                vec!["Status".to_string(), result.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string()],
                vec!["Web".to_string(), webui.to_string()],
            ];
            maybe_print_table(ctx, &["Field", "Value"], rows);
            Ok(())
        }
        OutputFormat::Markdown => markdown_not_supported(),
    }
}

async fn page_delete(client: &ApiClient, ctx: &AppContext, args: PageDeleteArgs) -> Result<()> {
    let page_id = resolve_page_id(client, &args.page).await?;
    if !args.yes {
        let confirm = Confirm::new()
            .with_prompt(format!("Delete page {page_id}?"))
            .default(false)
            .interact()
            .map_err(|err| {
                anyhow::anyhow!("{err}. Use --yes to skip confirmation in non-interactive shells.")
            })?;
        if !confirm {
            print_line(ctx, "Cancelled.");
            return Ok(());
        }
    }
    if args.purge {
        let status = page_status(client, &page_id).await?;
        if status != "trashed" {
            if !args.force {
                return Err(anyhow::anyhow!(
                    "Page {page_id} is not trashed. Delete first or use --force to trash then purge."
                ));
            }
            let url = client.v2_url(&format!("/pages/{page_id}"));
            client.delete(url).await?;
        }
        let mut url = client.v2_url(&format!("/pages/{page_id}"));
        url.push_str("?purge=true");
        client.delete(url).await?;
        print_line(ctx, &format!("Purged page {page_id}"));
        Ok(())
    } else {
        let url = client.v2_url(&format!("/pages/{page_id}"));
        client.delete(url).await?;
        print_line(ctx, &format!("Deleted page {page_id}"));
        Ok(())
    }
}

async fn page_children(client: &ApiClient, ctx: &AppContext, args: PageChildrenArgs) -> Result<()> {
    let page_id = resolve_page_id(client, &args.page).await?;
    let endpoint = if args.recursive { "descendants" } else { "direct-children" };
    let url = client.v2_url(&format!("/pages/{page_id}/{endpoint}?limit={}", args.limit));
    let items = client.get_paginated_results(url, args.all).await?;
    let output = OutputFormat::parse(&args.output)?;
    match output {
        OutputFormat::Json => maybe_print_json(ctx, &items),
        OutputFormat::Table => {
            let rows = items
                .iter()
                .map(|item| {
                    vec![
                        item.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        item.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        item.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        item.get("parentId").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    ]
                })
                .collect();
            maybe_print_table(ctx, &["ID", "Title", "Type", "Parent"], rows);
            Ok(())
        }
        OutputFormat::Markdown => markdown_not_supported(),
    }
}

async fn handle_search(ctx: &AppContext, cmd: SearchCommand) -> Result<()> {
    let client = load_client(ctx)?;
    let mut cql = to_cql_query(&cmd.query);
    if let Some(space) = cmd.space {
        cql = format!("space = {} AND ({cql})", space);
    }
    let output = OutputFormat::parse(&cmd.output)?;
    if cmd.all {
        let results = search_all(&client, &cql, cmd.limit).await?;
        match output {
            OutputFormat::Json => maybe_print_json(ctx, &results),
            OutputFormat::Table => {
                let rows = results
                    .iter()
                    .map(|item| {
                        let content = item.get("content").cloned().unwrap_or(Value::Null);
                        vec![
                            content.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            content.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            content.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        ]
                    })
                    .collect();
                maybe_print_table(ctx, &["ID", "Type", "Title"], rows);
                Ok(())
            }
            OutputFormat::Markdown => markdown_not_supported(),
        }
    } else {
        let params = [format!("cql={}", urlencoding::encode(&cql)), format!("limit={}", cmd.limit)];
        let url = format!("{}?{}", client.v1_url("/search"), params.join("&"));
        let (json, _) = client.get_json(url).await?;
        match output {
            OutputFormat::Json => maybe_print_json(ctx, &json),
            OutputFormat::Table => {
                let results = json.get("results").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                let rows = results
                    .iter()
                    .map(|item| {
                        let content = item.get("content").cloned().unwrap_or(Value::Null);
                        vec![
                            content.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            content.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            content.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        ]
                    })
                    .collect();
                maybe_print_table(ctx, &["ID", "Type", "Title"], rows);
                Ok(())
            }
            OutputFormat::Markdown => markdown_not_supported(),
        }
    }
}

async fn handle_attachment(ctx: &AppContext, cmd: AttachmentCommand) -> Result<()> {
    let client = load_client(ctx)?;
    match cmd {
        AttachmentCommand::List(args) => attachment_list(&client, ctx, args).await,
        AttachmentCommand::Get(args) => attachment_get(&client, ctx, args).await,
        AttachmentCommand::Download(args) => attachment_download(&client, ctx, args).await,
        AttachmentCommand::Upload(args) => attachment_upload(&client, ctx, args).await,
        AttachmentCommand::Delete(args) => attachment_delete(&client, ctx, args).await,
    }
}

async fn attachment_list(client: &ApiClient, ctx: &AppContext, args: AttachmentListArgs) -> Result<()> {
    let url = if let Some(page) = args.page {
        let page_id = resolve_page_id(client, &page).await?;
        client.v2_url(&format!("/pages/{page_id}/attachments?limit={}", args.limit))
    } else {
        client.v2_url(&format!("/attachments?limit={}", args.limit))
    };
    let items = client.get_paginated_results(url, args.all).await?;
    let output = OutputFormat::parse(&args.output)?;
    match output {
        OutputFormat::Json => maybe_print_json(ctx, &items),
        OutputFormat::Table => {
            let rows = items
                .iter()
                .map(|item| {
                    vec![
                        item.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        item.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        item.get("mediaType").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        human_size(item.get("fileSize").and_then(|v| v.as_i64()).unwrap_or(0)),
                    ]
                })
                .collect();
            maybe_print_table(ctx, &["ID", "Title", "Type", "Size"], rows);
            Ok(())
        }
        OutputFormat::Markdown => markdown_not_supported(),
    }
}

async fn attachment_get(client: &ApiClient, ctx: &AppContext, args: AttachmentGetArgs) -> Result<()> {
    let url = client.v2_url(&format!("/attachments/{}", args.attachment));
    let (json, _) = client.get_json(url).await?;
    let output = OutputFormat::parse(&args.output)?;
    match output {
        OutputFormat::Json => maybe_print_json(ctx, &json),
        OutputFormat::Table => {
            let rows = vec![
                vec!["ID".to_string(), json.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string()],
                vec!["Title".to_string(), json.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string()],
                vec!["Type".to_string(), json.get("mediaType").and_then(|v| v.as_str()).unwrap_or("").to_string()],
                vec!["Size".to_string(), human_size(json.get("fileSize").and_then(|v| v.as_i64()).unwrap_or(0))],
            ];
            maybe_print_table(ctx, &["Field", "Value"], rows);
            Ok(())
        }
        OutputFormat::Markdown => markdown_not_supported(),
    }
}

async fn attachment_download(client: &ApiClient, ctx: &AppContext, args: AttachmentDownloadArgs) -> Result<()> {
    let url = client.v2_url(&format!("/attachments/{}", args.attachment));
    let (json, _) = client.get_json(url).await?;
    let download = json
        .get("downloadLink")
        .and_then(|v| v.as_str())
        .or_else(|| json.get("_links").and_then(|v| v.get("download")).and_then(|v| v.as_str()))
        .context("Missing download link")?;
    let base = Url::parse(client.base_url())?;
    let full_url = if download.starts_with("http") {
        Url::parse(download)?
    } else {
        base.join(download)?
    };
    let response = client
        .apply_auth(client.http().get(full_url))?
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("Download failed: {}", response.status()));
    }
    let total = response.content_length();
    let file_name = resolve_download_path(&args.output, &json)?;
    let mut file = tokio::fs::File::create(&file_name).await?;
    let mut stream = response.bytes_stream();

    let progress = if ctx.quiet {
        None
    } else if let Some(total) = total {
        let bar = ProgressBar::new(total);
        bar.set_style(
            ProgressStyle::with_template("{spinner:.green} {bytes}/{total_bytes} {bar:40.cyan/blue} {eta}")
                .unwrap(),
        );
        Some(bar)
    } else {
        let bar = ProgressBar::new_spinner();
        bar.set_style(
            ProgressStyle::with_template("{spinner:.green} {bytes} {elapsed}")
                .unwrap(),
        );
        bar.enable_steady_tick(std::time::Duration::from_millis(120));
        Some(bar)
    };

    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
        if let Some(bar) = &progress {
            bar.inc(chunk.len() as u64);
        }
    }
    if let Some(bar) = progress {
        bar.finish_and_clear();
    }
    print_line(ctx, &format!("Downloaded to {}", file_name.display()));
    Ok(())
}

async fn attachment_upload(client: &ApiClient, ctx: &AppContext, args: AttachmentUploadArgs) -> Result<()> {
    let page_id = resolve_page_id(client, &args.page).await?;
    let metadata = tokio::fs::metadata(&args.file).await?;
    let size = metadata.len();
    if size > 5 * 1024 * 1024 {
        let confirm = Confirm::new()
            .with_prompt(format!("Upload {:.2} MB attachment?", size as f64 / 1_048_576.0))
            .default(false)
            .interact()?;
        if !confirm {
            print_line(ctx, "Cancelled.");
            return Ok(());
        }
    }

    let result = client
        .upload_attachment(&page_id, &args.file, args.comment)
        .await?;
    let attachment = result
        .get("results")
        .and_then(|v| v.as_array())
        .and_then(|items| items.first())
        .cloned()
        .unwrap_or(result);
    let output = OutputFormat::parse(&args.output)?;
    match output {
        OutputFormat::Json => maybe_print_json(ctx, &attachment),
        OutputFormat::Table => {
            let rows = vec![
                vec!["ID".to_string(), attachment.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string()],
                vec!["Title".to_string(), attachment.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string()],
            ];
            maybe_print_table(ctx, &["Field", "Value"], rows);
            Ok(())
        }
        OutputFormat::Markdown => markdown_not_supported(),
    }
}

async fn attachment_delete(client: &ApiClient, ctx: &AppContext, args: AttachmentDeleteArgs) -> Result<()> {
    if !args.yes {
        let confirm = Confirm::new()
            .with_prompt(format!("Delete attachment {}?", args.attachment))
            .default(false)
            .interact()
            .map_err(|err| {
                anyhow::anyhow!("{err}. Use --yes to skip confirmation in non-interactive shells.")
            })?;
        if !confirm {
            print_line(ctx, "Cancelled.");
            return Ok(());
        }
    }
    let mut url = client.v2_url(&format!("/attachments/{}", args.attachment));
    if args.purge {
        url.push_str("?purge=true");
    }
    client.delete(url).await?;
    print_line(ctx, &format!("Deleted attachment {}", args.attachment));
    Ok(())
}

async fn handle_label(ctx: &AppContext, cmd: LabelCommand) -> Result<()> {
    let client = load_client(ctx)?;
    match cmd {
        LabelCommand::List(args) => label_list(&client, ctx, args).await,
        LabelCommand::Add(args) => label_add(&client, ctx, args).await,
        LabelCommand::Remove(args) => label_remove(&client, ctx, args).await,
        LabelCommand::Pages(args) => label_pages(&client, ctx, args).await,
    }
}

async fn label_list(client: &ApiClient, ctx: &AppContext, args: LabelListArgs) -> Result<()> {
    let url = client.v2_url(&format!("/labels?limit={}", args.limit));
    let items = client.get_paginated_results(url, args.all).await?;
    let output = OutputFormat::parse(&args.output)?;
    match output {
        OutputFormat::Json => maybe_print_json(ctx, &items),
        OutputFormat::Table => {
            let rows = items
                .iter()
                .map(|item| {
                    vec![
                        item.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        item.get("prefix").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    ]
                })
                .collect();
            maybe_print_table(ctx, &["ID", "Name", "Prefix"], rows);
            Ok(())
        }
        OutputFormat::Markdown => markdown_not_supported(),
    }
}

async fn label_add(client: &ApiClient, ctx: &AppContext, args: LabelAddArgs) -> Result<()> {
    let page_id = resolve_page_id(client, &args.page).await?;
    let url = client.v1_url(&format!("/content/{page_id}/label"));
    let body = json!([
        { "prefix": "global", "name": args.label }
    ]);
    client.post_json(url, body).await?;
    print_line(ctx, "Added label.");
    Ok(())
}

async fn label_remove(client: &ApiClient, ctx: &AppContext, args: LabelRemoveArgs) -> Result<()> {
    let page_id = resolve_page_id(client, &args.page).await?;
    let url = client.v1_url(&format!("/content/{page_id}/label?name={}&prefix=global", urlencoding::encode(&args.label)));
    client.delete(url).await?;
    print_line(ctx, "Removed label.");
    Ok(())
}

async fn label_pages(client: &ApiClient, ctx: &AppContext, args: LabelPagesArgs) -> Result<()> {
    let cql = label_cql(&args.label);
    let url = format!("{}?cql={}&limit={}", client.v1_url("/search"), urlencoding::encode(&cql), args.limit);
    let (json, _) = client.get_json(url).await?;
    let output = OutputFormat::parse(&args.output)?;
    match output {
        OutputFormat::Json => maybe_print_json(ctx, &json),
        OutputFormat::Table => {
            let results = json.get("results").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let rows = results
                .iter()
                .map(|item| label_result_row(item))
                .collect();
            maybe_print_table(ctx, &["ID", "Type", "Title"], rows);
            Ok(())
        }
        OutputFormat::Markdown => markdown_not_supported(),
    }
}

fn load_client(ctx: &AppContext) -> Result<ApiClient> {
    if let Some(config) = Config::from_env()? {
        return ApiClient::new(config.base_url, config.auth, ctx.verbose);
    }
    if !Config::exists()? {
        return Err(anyhow::anyhow!("Not logged in. Run confcli auth login"));
    }
    let config = Config::load().context("Failed to load config")?;
    ApiClient::new(config.base_url, config.auth, ctx.verbose)
}

fn maybe_print_json<T: serde::Serialize>(_ctx: &AppContext, value: &T) -> Result<()> {
    print_json(value)
}

fn maybe_print_table(_ctx: &AppContext, headers: &[&str], rows: Vec<Vec<String>>) {
    print_table(headers, rows);
}

fn print_line(ctx: &AppContext, message: &str) {
    if ctx.quiet {
        return;
    }
    println!("{message}");
}

fn human_size(bytes: i64) -> String {
    if bytes < 0 {
        return bytes.to_string();
    }
    format_size(bytes as u64, BINARY)
}

fn resolve_download_path(output: &Option<PathBuf>, json: &Value) -> Result<PathBuf> {
    if let Some(path) = output {
        return Ok(path.clone());
    }
    let title = json.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let file_name = Path::new(title).file_name().and_then(|v| v.to_str()).unwrap_or("");
    if file_name.is_empty() {
        return Err(anyhow::anyhow!(
            "Unsafe or missing attachment title. Provide --output to choose a file path."
        ));
    }
    Ok(PathBuf::from(file_name))
}

fn add_markdown_header(base_url: &str, json: &Value, markdown: &str) -> String {
    let webui = json
        .get("_links")
        .and_then(|v| v.get("webui"))
        .and_then(|v| v.as_str());
    if let Some(webui) = webui {
        let source = format!("{base_url}{webui}");
        format!("<!-- Source: {source} -->\n\n{markdown}")
    } else {
        markdown.to_string()
    }
}

async fn page_status(client: &ApiClient, page_id: &str) -> Result<String> {
    let url = client.v2_url(&format!("/pages/{page_id}"));
    let (json, _) = client.get_json(url).await?;
    Ok(json
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("current")
        .to_string())
}

fn label_cql(label: &str) -> String {
    let label = escape_cql_text(label);
    if label.contains(':') {
        format!("label = \"{label}\"")
    } else {
        format!(
            "label in (\"{label}\", \"team:{label}\", \"my:{label}\")"
        )
    }
}

fn label_result_row(item: &Value) -> Vec<String> {
    if let Some(content) = item.get("content") {
        let id = content.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let typ = content.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let title = content.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
        return vec![id, typ, title];
    }

    let entity_type = item.get("entityType").and_then(|v| v.as_str()).unwrap_or("");
    if entity_type == "space" {
        let key = item
            .get("space")
            .and_then(|v| v.get("key"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("");
        return vec![key.to_string(), "space".to_string(), title.to_string()];
    }

    let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let typ = item.get("type").or_else(|| item.get("entityType")).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
    vec![id, typ, title]
}

fn markdown_not_supported() -> Result<()> {
    Err(anyhow::anyhow!(
        "Markdown output is only supported for page get/body"
    ))
}

fn escape_cql_text(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ")
        .replace('\r', " ")
        .replace('\t', " ")
}

fn to_cql_query(query: &str) -> String {
    let has_operator = ["=", "~", "AND", "OR", "NOT", "(", ")"]
        .iter()
        .any(|op| query.contains(op));
    if has_operator {
        query.to_string()
    } else {
        format!("text ~ \"{}\"", escape_cql_text(query))
    }
}

async fn search_all(client: &ApiClient, cql: &str, limit: usize) -> Result<Vec<Value>> {
    let mut start = 0usize;
    let mut results = Vec::new();
    loop {
        let params = [
            format!("cql={}", urlencoding::encode(cql)),
            format!("limit={}", limit),
            format!("start={}", start),
        ];
        let url = format!("{}?{}", client.v1_url("/search"), params.join("&"));
        let (json, _) = client.get_json(url).await?;
        let page = json.get("results").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        let page_len = page.len();
        if page_len == 0 {
            break;
        }
        results.extend(page);
        if page_len < limit {
            break;
        }
        start += limit;
    }
    Ok(results)
}

async fn read_body(body: Option<String>, body_file: Option<&PathBuf>) -> Result<String> {
    if body.is_some() && body_file.is_some() {
        return Err(anyhow::anyhow!("Use either --body or --body-file, not both"));
    }
    if let Some(body) = body {
        return Ok(body);
    }
    if let Some(path) = body_file {
        if path == &PathBuf::from("-") {
            let mut input = String::new();
            let mut stdin = tokio::io::stdin();
            use tokio::io::AsyncReadExt;
            stdin.read_to_string(&mut input).await?;
            return Ok(input);
        }
        return tokio::fs::read_to_string(path)
            .await
            .with_context(|| format!("Failed to read {}", path.display()));
    }
    Err(anyhow::anyhow!("Provide --body or --body-file (use '-' for stdin)"))
}

fn derive_title_from_file(body_file: Option<&PathBuf>) -> Option<String> {
    let path = body_file?;
    if path == &PathBuf::from("-") {
        return None;
    }
    path.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string())
}

async fn resolve_space_key(client: &ApiClient, space_id: &str) -> Result<String> {
    let url = client.v2_url(&format!("/spaces/{}", space_id));
    let (json, _) = client.get_json(url).await?;
    Ok(json.get("key").and_then(|v| v.as_str()).unwrap_or(space_id).to_string())
}

async fn resolve_space_keys(client: &ApiClient, space_ids: &[String]) -> Result<HashMap<String, String>> {
    let mut unique: Vec<String> = space_ids.to_vec();
    unique.sort();
    unique.dedup();
    if unique.is_empty() {
        return Ok(HashMap::new());
    }
    let mut map = HashMap::new();
    for chunk in unique.chunks(250) {
        let ids = chunk.join(",");
        let url = client.v2_url(&format!("/spaces?ids={ids}&limit={}", chunk.len()));
        let items = client.get_paginated_results(url, false).await?;
        for item in items {
            if let (Some(id), Some(key)) = (item.get("id").and_then(|v| v.as_str()), item.get("key").and_then(|v| v.as_str())) {
                map.insert(id.to_string(), key.to_string());
            }
        }
    }
    Ok(map)
}

async fn resolve_space_id(client: &ApiClient, space: &str) -> Result<String> {
    if space.chars().all(|c| c.is_ascii_digit()) {
        return Ok(space.to_string());
    }
    let url = client.v2_url(&format!("/spaces?keys={space}&limit=1"));
    let items = client.get_paginated_results(url, false).await?;
    let id = items
        .first()
        .and_then(|item| item.get("id"))
        .and_then(|v| v.as_str())
        .context("Space not found")?;
    Ok(id.to_string())
}

async fn resolve_page_id(client: &ApiClient, page: &str) -> Result<String> {
    if page.chars().all(|c| c.is_ascii_digit()) {
        return Ok(page.to_string());
    }
    if let Ok(url) = Url::parse(page) {
        if let Some(id) = extract_page_id_from_url(&url) {
            return Ok(id);
        }
    }
    if let Some((space, title)) = page.split_once(':') {
        let space_id = resolve_space_id(client, space).await?;
        let url = client.v2_url(&format!("/pages?space-id={space_id}&title={}&limit=1", urlencoding::encode(title)));
        let items = client.get_paginated_results(url, false).await?;
        let id = items
            .first()
            .and_then(|item| item.get("id"))
            .and_then(|v| v.as_str())
            .context("Page not found")?;
        return Ok(id.to_string());
    }
    Err(anyhow::anyhow!(
        "Unable to resolve page reference '{page}'. Use a page id, URL, or SPACE:Title."
    ))
}

fn extract_page_id_from_url(url: &Url) -> Option<String> {
    let segments: Vec<&str> = url.path_segments()?.collect();
    if let Some(pos) = segments.iter().position(|seg| *seg == "pages") {
        if let Some(id) = segments.get(pos + 1) {
            if id.chars().all(|c| c.is_ascii_digit()) {
                return Some(id.to_string());
            }
        }
    }
    url.query_pairs()
        .find(|(key, _)| key == "pageId")
        .map(|(_, value)| value.to_string())
}

fn build_page_tree(items: &[Value]) -> Vec<String> {
    let mut children: HashMap<String, Vec<Value>> = HashMap::new();
    let mut roots = Vec::new();
    for item in items {
        let parent = item
            .get("parentId")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if parent.is_empty() {
            roots.push(item.clone());
        } else {
            children.entry(parent.to_string()).or_default().push(item.clone());
        }
    }

    let mut lines = Vec::new();
    for root in roots {
        walk_tree(&root, &children, 0, &mut lines);
    }
    lines
}

fn walk_tree(node: &Value, children: &HashMap<String, Vec<Value>>, depth: usize, lines: &mut Vec<String>) {
    let title = node.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let id = node.get("id").and_then(|v| v.as_str()).unwrap_or("");
    lines.push(format!("{}- {} ({})", "  ".repeat(depth), title, id));
    if let Some(children_nodes) = children.get(id) {
        let mut sorted = children_nodes.clone();
        sorted.sort_by_key(|v| v.get("childPosition").and_then(|p| p.as_i64()).unwrap_or(0));
        for child in sorted {
            walk_tree(&child, children, depth + 1, lines);
        }
    }
}
