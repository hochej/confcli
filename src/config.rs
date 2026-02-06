use crate::auth::AuthMethod;
use anyhow::{Context, Result};
use dirs::config_dir;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use url::Url;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Base URL for human-facing Confluence pages, used for `open` and for
    /// constructing web links (often ends with `/wiki` on Confluence Cloud).
    #[serde(alias = "base_url")]
    pub site_url: String,
    /// Base URL for v1 REST API calls, e.g. `https://example.atlassian.net/wiki/rest/api`.
    ///
    /// Note: This is a full URL, not just the path.
    #[serde(default)]
    pub api_base_v1: String,
    /// Base URL for v2 REST API calls, e.g. `https://example.atlassian.net/wiki/api/v2`.
    ///
    /// Note: This is a full URL, not just the path.
    #[serde(default)]
    pub api_base_v2: String,
    pub auth: AuthMethod,
}

impl Config {
    pub fn path() -> Result<PathBuf> {
        let base = config_dir().context("Unable to resolve config directory")?;
        Ok(base.join("confcli").join("config.json"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        let data = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config: {}", path.display()))?;
        let mut config: Config = serde_json::from_str(&data)
            .with_context(|| format!("Invalid config format: {}", path.display()))?;
        config.normalize_and_backfill()?;
        Ok(config)
    }

    pub fn from_env() -> Result<Option<Self>> {
        let base_input = env::var("CONFLUENCE_BASE_URL")
            .ok()
            .or_else(|| env::var("CONFLUENCE_URL").ok())
            .or_else(|| env::var("CONFLUENCE_DOMAIN").ok());

        let base_input = match base_input {
            Some(url) => url,
            None => return Ok(None),
        };

        let site_url = normalize_site_url(&base_input)?;

        // Competitor migration: allow `CONFLUENCE_API_TOKEN` as a synonym for `CONFLUENCE_TOKEN`.
        let bearer = env::var("CONFLUENCE_BEARER_TOKEN").ok();
        if let Some(token) = bearer {
            let (api_base_v1, api_base_v2) = api_bases_from_env_or_defaults(&site_url)?;
            return Ok(Some(Config {
                site_url,
                api_base_v1,
                api_base_v2,
                auth: AuthMethod::Bearer { token },
            }));
        }

        let email = env::var("CONFLUENCE_EMAIL").ok();
        let token = env::var("CONFLUENCE_TOKEN")
            .ok()
            .or_else(|| env::var("CONFLUENCE_API_TOKEN").ok());
        if let (Some(email), Some(token)) = (email, token) {
            let (api_base_v1, api_base_v2) = api_bases_from_env_or_defaults(&site_url)?;
            return Ok(Some(Config {
                site_url,
                api_base_v1,
                api_base_v2,
                auth: AuthMethod::Basic { email, token },
            }));
        }

        Ok(None)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config dir: {}", parent.display()))?;
        }
        // Always write normalized config to disk.
        let mut normalized = self.clone();
        normalized.normalize_and_backfill()?;
        let data = serde_json::to_string_pretty(&normalized)?;
        fs::write(&path, data)
            .with_context(|| format!("Failed to write config: {}", path.display()))?;
        #[cfg(unix)]
        {
            let perms = fs::Permissions::from_mode(0o600);
            fs::set_permissions(&path, perms)
                .with_context(|| format!("Failed to set permissions: {}", path.display()))?;
        }
        Ok(())
    }

    pub fn exists() -> Result<bool> {
        let path = Self::path()?;
        Ok(Path::new(&path).exists())
    }

    pub fn clear() -> Result<()> {
        let path = Self::path()?;
        if Path::new(&path).exists() {
            fs::remove_file(&path)
                .with_context(|| format!("Failed to delete config: {}", path.display()))?;
        }
        Ok(())
    }

    fn normalize_and_backfill(&mut self) -> Result<()> {
        self.site_url = normalize_site_url(&self.site_url)?;

        // Back-compat: configs written before api_base fields existed.
        if self.api_base_v1.trim().is_empty() || self.api_base_v2.trim().is_empty() {
            let (api_base_v1, api_base_v2) = api_bases_from_env_or_defaults(&self.site_url)?;
            if self.api_base_v1.trim().is_empty() {
                self.api_base_v1 = api_base_v1;
            }
            if self.api_base_v2.trim().is_empty() {
                self.api_base_v2 = api_base_v2;
            }
        } else {
            self.api_base_v1 = normalize_full_url(&self.api_base_v1)?;
            self.api_base_v2 = normalize_full_url(&self.api_base_v2)?;
        }

        Ok(())
    }
}

fn normalize_site_url(input: &str) -> Result<String> {
    // Accept bare domains; default to https.
    let mut s = input.trim().to_string();
    if !s.starts_with("http://") && !s.starts_with("https://") {
        s = format!("https://{s}");
    }
    let mut url = Url::parse(&s).context("Invalid Confluence URL")?;
    match url.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(anyhow::anyhow!(
                "Invalid URL scheme '{scheme}'. Use http or https."
            ));
        }
    }
    if url.host_str().is_none() {
        return Err(anyhow::anyhow!("Invalid Confluence URL: missing host"));
    }
    // Cloud default: `.../wiki` is the common web base.
    if url.host_str().unwrap_or("").ends_with(".atlassian.net")
        && (url.path().is_empty() || url.path() == "/")
    {
        url.set_path("/wiki");
    }
    // Normalize: strip trailing `/`.
    let normalized = url.as_str().trim_end_matches('/').to_string();
    Ok(normalized)
}

fn normalize_full_url(input: &str) -> Result<String> {
    let s = input.trim();
    let url = Url::parse(s).context("Invalid API base URL")?;
    match url.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(anyhow::anyhow!(
                "Invalid URL scheme '{scheme}'. Use http or https."
            ));
        }
    }
    if url.host_str().is_none() {
        return Err(anyhow::anyhow!("Invalid API base URL: missing host"));
    }
    Ok(url.as_str().trim_end_matches('/').to_string())
}

fn api_bases_from_env_or_defaults(site_url: &str) -> Result<(String, String)> {
    let site = Url::parse(site_url).context("Invalid Confluence URL")?;
    let origin = format!(
        "{}://{}{}",
        site.scheme(),
        site.host_str().unwrap_or_default(),
        match site.port() {
            Some(port) => format!(":{port}"),
            None => "".to_string(),
        }
    );

    // Competitor migration: `CONFLUENCE_API_PATH` is a path like `/wiki/rest/api`.
    let api_path_v1 = env::var("CONFLUENCE_API_PATH")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(|s| ensure_leading_slash(&s))
        .unwrap_or_else(|| {
            if site.host_str().unwrap_or("").ends_with(".atlassian.net") {
                "/wiki/rest/api".to_string()
            } else {
                "/rest/api".to_string()
            }
        });

    let api_base_v1 = format!("{}{}", origin, api_path_v1.trim_end_matches('/'));

    // Derive v2 base from v1 path when possible: `/.../rest/api` -> `/.../api/v2`.
    let api_path_v2 = if let Some(prefix) = api_path_v1.strip_suffix("/rest/api") {
        format!("{prefix}/api/v2")
    } else {
        "/api/v2".to_string()
    };
    let api_base_v2 = format!("{}{}", origin, api_path_v2.trim_end_matches('/'));

    Ok((api_base_v1, api_base_v2))
}

fn ensure_leading_slash(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthMethod;

    // Mutating process env vars is global shared state. Rust 2024 makes these APIs `unsafe`
    // for a reason: tests are parallel by default. Serialize any env-var mutations.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn normalize_site_url_cloud_defaults_to_wiki() {
        let url = normalize_site_url("example.atlassian.net").unwrap();
        assert_eq!(url, "https://example.atlassian.net/wiki");

        let url = normalize_site_url("https://example.atlassian.net").unwrap();
        assert_eq!(url, "https://example.atlassian.net/wiki");

        let url = normalize_site_url("https://example.atlassian.net/wiki").unwrap();
        assert_eq!(url, "https://example.atlassian.net/wiki");
    }

    #[test]
    fn backfills_api_bases_from_legacy_base_url() {
        // Ensure the test is not influenced by external env.
        let _lock = ENV_LOCK.lock().unwrap();
        let prev = std::env::var("CONFLUENCE_API_PATH").ok();
        // SAFETY: guarded by ENV_LOCK.
        unsafe { std::env::remove_var("CONFLUENCE_API_PATH") };

        let mut cfg = Config {
            site_url: "https://example.atlassian.net/wiki".to_string(),
            api_base_v1: "".to_string(),
            api_base_v2: "".to_string(),
            auth: AuthMethod::Basic {
                email: "a@b.c".to_string(),
                token: "x".to_string(),
            },
        };
        cfg.normalize_and_backfill().unwrap();
        assert_eq!(
            cfg.api_base_v1,
            "https://example.atlassian.net/wiki/rest/api"
        );
        assert_eq!(cfg.api_base_v2, "https://example.atlassian.net/wiki/api/v2");

        // Restore previous value.
        match prev {
            Some(value) => {
                // SAFETY: guarded by ENV_LOCK.
                unsafe { std::env::set_var("CONFLUENCE_API_PATH", value) };
            }
            None => {
                // SAFETY: guarded by ENV_LOCK.
                unsafe { std::env::remove_var("CONFLUENCE_API_PATH") };
            }
        }
    }
}
