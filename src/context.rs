use anyhow::{Context, Result};
use confcli::client::ApiClient;
use confcli::config::Config;

#[derive(Debug, Clone, Copy)]
pub struct AppContext {
    pub quiet: bool,
    pub verbose: u8,
    pub dry_run: bool,
}

pub fn load_client(ctx: &AppContext) -> Result<ApiClient> {
    if let Some(config) = Config::from_env()? {
        return ApiClient::new(
            config.site_url,
            config.api_base_v1,
            config.api_base_v2,
            config.auth,
            ctx.verbose,
        );
    }
    if !Config::exists()? {
        return Err(anyhow::anyhow!("Not logged in. Run confcli auth login"));
    }
    let config = Config::load().context("Failed to load config")?;
    ApiClient::new(
        config.site_url,
        config.api_base_v1,
        config.api_base_v2,
        config.auth,
        ctx.verbose,
    )
}
