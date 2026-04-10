use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub base_url: String,
    pub client_id: String,
    pub client_secret: String,
    #[serde(default)]
    pub ssl_verify: bool,
}

pub fn config_path() -> Option<PathBuf> {
    let mut path = dirs::home_dir()?;
    path.push(".omadacli");
    path.push("config.toml");
    Some(path)
}

impl Config {
    /// Load config from ~/.omadacli/config.toml, with env vars as overrides.
    /// Falls back to env vars only if no config file exists.
    pub fn load() -> Result<Self> {
        let from_file = config_path()
            .filter(|p| p.exists())
            .map(|path| {
                let content =
                    std::fs::read_to_string(&path).context("Failed to read config file")?;
                toml::from_str::<Config>(&content).context("Failed to parse config file")
            })
            .transpose()?;

        match from_file {
            Some(mut config) => {
                if let Ok(v) = std::env::var("OMADA_BASE_URL") {
                    config.base_url = v;
                }
                if let Ok(v) = std::env::var("OMADA_CLIENT_ID") {
                    config.client_id = v;
                }
                if let Ok(v) = std::env::var("OMADA_CLIENT_SECRET") {
                    config.client_secret = v;
                }
                if std::env::var("OMADA_SSL_VERIFY").as_deref() == Ok("true") {
                    config.ssl_verify = true;
                }
                Ok(config)
            }
            None => Ok(Self {
                base_url: std::env::var("OMADA_BASE_URL").context(
                    "No config file found and OMADA_BASE_URL not set.\n\
                     Run `omada config --base-url <URL> --client-id <ID> --client-secret <SECRET>` to configure.",
                )?,
                client_id: std::env::var("OMADA_CLIENT_ID")
                    .context("OMADA_CLIENT_ID not set")?,
                client_secret: std::env::var("OMADA_CLIENT_SECRET")
                    .context("OMADA_CLIENT_SECRET not set")?,
                ssl_verify: std::env::var("OMADA_SSL_VERIFY").as_deref() == Ok("true"),
            }),
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path().context("Could not resolve home directory")?;
        std::fs::create_dir_all(path.parent().unwrap())
            .context("Failed to create config directory")?;
        let content = toml::to_string_pretty(self).context("Failed to serialize config")?;
        std::fs::write(&path, content).context("Failed to write config file")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .context("Failed to set config file permissions")?;
        }
        Ok(())
    }
}
