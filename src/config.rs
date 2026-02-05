use crate::auth::AuthMethod;
use anyhow::{Context, Result};
use dirs::config_dir;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub base_url: String,
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
        let config: Config = serde_json::from_str(&data)
            .with_context(|| format!("Invalid config format: {}", path.display()))?;
        Ok(config)
    }

    pub fn from_env() -> Result<Option<Self>> {
        let base_url = env::var("CONFLUENCE_BASE_URL")
            .ok()
            .or_else(|| env::var("CONFLUENCE_URL").ok())
            .or_else(|| {
                env::var("CONFLUENCE_DOMAIN")
                    .ok()
                    .map(|domain| format!("https://{domain}"))
            });

        let base_url = match base_url {
            Some(mut url) => {
                if !url.starts_with("http") {
                    url = format!("https://{url}");
                }
                if !url.ends_with("/wiki") {
                    url.push_str("/wiki");
                }
                url
            }
            None => return Ok(None),
        };

        let bearer = env::var("CONFLUENCE_BEARER_TOKEN").ok();
        if let Some(token) = bearer {
            return Ok(Some(Config {
                base_url,
                auth: AuthMethod::Bearer { token },
            }));
        }

        let email = env::var("CONFLUENCE_EMAIL").ok();
        let token = env::var("CONFLUENCE_TOKEN").ok();
        if let (Some(email), Some(token)) = (email, token) {
            return Ok(Some(Config {
                base_url,
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
        let data = serde_json::to_string_pretty(self)?;
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
}
