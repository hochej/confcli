use clap::{Args, Subcommand};

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
