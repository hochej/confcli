use anyhow::{Context, Result};
use confcli::auth::AuthMethod;
use confcli::client::ApiClient;
use confcli::config::Config;
use dialoguer::{Input, Password};

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
    let domain = if let Some(domain) = args.domain {
        domain
    } else {
        Input::new()
            .with_prompt("Confluence domain (e.g. yourcompany.atlassian.net)")
            .interact_text()?
    };

    let mut base_url = domain;
    if !base_url.starts_with("http") {
        base_url = format!("https://{base_url}");
    }

    // Validate the URL
    let parsed = url::Url::parse(&base_url).context("Invalid domain URL")?;
    match parsed.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(anyhow::anyhow!(
                "Invalid URL scheme '{scheme}'. Use http or https."
            ))
        }
    }
    if parsed.host().is_none() {
        return Err(anyhow::anyhow!("Invalid domain: no host found in URL"));
    }

    if !base_url.ends_with("/wiki") {
        base_url.push_str("/wiki");
    }

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
    let _ = client
        .get_json(url)
        .await
        .context("Failed to validate auth")?;
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
