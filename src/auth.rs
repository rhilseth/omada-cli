use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub struct Config {
    pub base_url: String,
    pub client_id: String,
    pub client_secret: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            base_url: std::env::var("OMADA_BASE_URL").context("OMADA_BASE_URL env var not set")?,
            client_id: std::env::var("OMADA_CLIENT_ID")
                .context("OMADA_CLIENT_ID env var not set")?,
            client_secret: std::env::var("OMADA_CLIENT_SECRET")
                .context("OMADA_CLIENT_SECRET env var not set")?,
        })
    }
}

pub struct Session {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub omadac_id: String,
    #[allow(dead_code)]
    pub refresh_token: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ControllerInfo {
    omadac_id: String,
}

#[derive(Serialize)]
struct TokenRequest<'a> {
    #[serde(rename = "omadacId")]
    omadac_id: &'a str,
    client_id: &'a str,
    client_secret: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccessToken {
    access_token: String,
    token_type: String,
    expires_in: u64,
    refresh_token: Option<String>,
}

#[derive(Deserialize)]
struct OmadaResponse<T> {
    #[serde(rename = "errorCode")]
    error_code: i32,
    msg: String,
    result: Option<T>,
}

async fn get_controller_id(client: &reqwest::Client, base_url: &str) -> Result<String> {
    let url = format!("{base_url}/api/info");
    let resp: OmadaResponse<ControllerInfo> = client
        .get(&url)
        .send()
        .await
        .context("Failed to reach Omada controller")?
        .json()
        .await
        .context("Failed to parse controller info")?;

    if resp.error_code != 0 {
        anyhow::bail!(
            "Failed to get controller info: {} (errorCode {})",
            resp.msg,
            resp.error_code
        );
    }

    resp.result
        .map(|i| i.omadac_id)
        .context("Controller info missing result field")
}

pub async fn authenticate(client: &reqwest::Client, config: &Config) -> Result<Session> {
    let omadac_id = get_controller_id(client, &config.base_url).await?;

    let url = format!("{}/openapi/authorize/token", config.base_url);
    let body = TokenRequest {
        omadac_id: &omadac_id,
        client_id: &config.client_id,
        client_secret: &config.client_secret,
    };

    let resp: OmadaResponse<AccessToken> = client
        .post(&url)
        .query(&[("grant_type", "client_credentials")])
        .json(&body)
        .send()
        .await
        .context("Failed to reach Omada controller")?
        .json()
        .await
        .context("Failed to parse token response")?;

    if resp.error_code != 0 {
        anyhow::bail!(
            "Authentication failed: {} (errorCode {})",
            resp.msg,
            resp.error_code
        );
    }

    let token = resp.result.context("Token response missing result field")?;
    Ok(Session {
        access_token: token.access_token,
        token_type: token.token_type,
        expires_in: token.expires_in,
        omadac_id,
        refresh_token: token.refresh_token,
    })
}
