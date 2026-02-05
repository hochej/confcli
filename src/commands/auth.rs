use anyhow::{Context, Result};
use confcli::auth::AuthMethod;
use confcli::client::ApiClient;
use confcli::config::Config;
use dialoguer::{Input, Password};
use url::Url;

use crate::cli::{AuthCommand, AuthLoginArgs};
use crate::context::AppContext;
use crate::helpers::print_line;

pub async fn handle(ctx: &AppContext, cmd: AuthCommand) -> Result<()> {
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
    let site_input = if let Some(domain) = args.domain {
        domain
    } else {
        Input::new()
            .with_prompt("Confluence site URL (e.g. https://yourcompany.atlassian.net/wiki)")
            .interact_text()?
    };

    let (site_url, origin) = normalize_site_url_and_origin(&site_input)?;

    let api_path_v1 = args
        .api_path
        .as_deref()
        .map(ensure_leading_slash)
        .unwrap_or_else(|| default_api_path_v1(&site_url));
    let api_base_v1 = format!("{}{}", origin, api_path_v1.trim_end_matches('/'));

    let api_path_v2 = args
        .api_v2_path
        .as_deref()
        .map(ensure_leading_slash)
        .unwrap_or_else(|| derive_api_path_v2(&api_path_v1));
    let api_base_v2 = format!("{}{}", origin, api_path_v2.trim_end_matches('/'));

    let auth = if let Some(token) = args.bearer {
        AuthMethod::Bearer { token }
    } else {
        let email = if let Some(email) = args.email {
            email
        } else {
            Input::new().with_prompt("Email").interact_text()?
        };
        let token = if let Some(token) = args
            .token
            .or_else(|| std::env::var("CONFLUENCE_API_TOKEN").ok())
        {
            token
        } else {
            Password::new()
                .with_prompt("API token")
                .with_confirmation("Confirm token", "Tokens do not match")
                .interact()?
        };
        AuthMethod::Basic { email, token }
    };

    let config = Config {
        site_url,
        api_base_v1,
        api_base_v2,
        auth,
    };
    let client = ApiClient::new(
        config.site_url.clone(),
        config.api_base_v1.clone(),
        config.api_base_v2.clone(),
        config.auth.clone(),
        ctx.verbose,
    )?;

    // Validate credentials. Prefer v2; fall back to v1 for Server/DC.
    let v2 = client.v2_url("/spaces?limit=1");
    let v1 = client.v1_url("/space?limit=1");
    if let Err(v2_err) = client.get_json(v2).await {
        client
            .get_json(v1)
            .await
            .with_context(|| format!("Failed to validate credentials (v2 error: {v2_err})"))?;
    }
    config.save()?;
    print_line(ctx, "Saved credentials.");
    Ok(())
}

async fn auth_status(ctx: &AppContext) -> Result<()> {
    if let Some(config) = Config::from_env()? {
        let client = ApiClient::new(
            config.site_url.clone(),
            config.api_base_v1.clone(),
            config.api_base_v2.clone(),
            config.auth.clone(),
            ctx.verbose,
        )?;
        let v2 = client.v2_url("/spaces?limit=1");
        let v1 = client.v1_url("/space?limit=1");
        if let Err(v2_err) = client.get_json(v2).await {
            client
                .get_json(v1)
                .await
                .with_context(|| format!("Failed to validate auth (v2 error: {v2_err})"))?;
        }
        print_line(
            ctx,
            &format!(
                "Logged in to {} using {} auth (from env)",
                config.site_url,
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
    let client = ApiClient::new(
        config.site_url.clone(),
        config.api_base_v1.clone(),
        config.api_base_v2.clone(),
        config.auth.clone(),
        ctx.verbose,
    )?;
    let v2 = client.v2_url("/spaces?limit=1");
    let v1 = client.v1_url("/space?limit=1");
    if let Err(v2_err) = client.get_json(v2).await {
        client
            .get_json(v1)
            .await
            .with_context(|| format!("Failed to validate auth (v2 error: {v2_err})"))?;
    }
    let path = Config::path()?;
    print_line(
        ctx,
        &format!(
            "Logged in to {} using {} auth (config: {})",
            config.site_url,
            config.auth.description(),
            path.display()
        ),
    );
    Ok(())
}

fn normalize_site_url_and_origin(input: &str) -> Result<(String, String)> {
    let trimmed = input.trim();
    let with_scheme = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    let mut url = Url::parse(&with_scheme).context("Invalid Confluence URL")?;
    match url.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(anyhow::anyhow!(
                "Invalid URL scheme '{scheme}'. Use http or https."
            ));
        }
    }
    let host = url
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid Confluence URL: missing host"))?;
    let port = url.port().map(|p| format!(":{p}")).unwrap_or_default();
    let origin = format!("{}://{}{}", url.scheme(), host, port);

    // Default web UI base for Cloud if the user only gave the domain.
    let is_cloud = host.ends_with(".atlassian.net");
    let path = url.path().trim_end_matches('/');
    if is_cloud && (path.is_empty() || path == "/") {
        url.set_path("/wiki");
    }

    let site_url = url.as_str().trim_end_matches('/').to_string();
    Ok((site_url, origin))
}

fn ensure_leading_slash(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

fn default_api_path_v1(site_url: &str) -> String {
    if site_url.trim_end_matches('/').ends_with("/wiki") {
        "/wiki/rest/api".to_string()
    } else {
        "/rest/api".to_string()
    }
}

fn derive_api_path_v2(api_path_v1: &str) -> String {
    if let Some(prefix) = api_path_v1.trim_end_matches('/').strip_suffix("/rest/api") {
        format!("{prefix}/api/v2")
    } else {
        "/api/v2".to_string()
    }
}
