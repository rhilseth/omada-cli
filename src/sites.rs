use anyhow::{Context, Result};
use std::collections::HashMap;

use crate::{auth, cache, execute, model};

pub async fn get_or_fetch(
    client: &reqwest::Client,
    session: &auth::Session,
    api_spec: &model::ApiSpec,
    omadac_id: &str,
    base_url: &str,
) -> Result<Vec<model::CachedSite>> {
    if let Some(cached) = cache::load_sites(omadac_id) {
        return Ok(cached.sites);
    }

    let op = api_spec
        .operations
        .iter()
        .find(|op| op.operation_id == "getSiteList")
        .context("getSiteList operation not found in spec")?;

    let mut params = HashMap::new();
    params.insert("page".to_string(), "1".to_string());
    params.insert("page-size".to_string(), "20".to_string());

    let result = execute::run(client, session, op, &params, None, base_url).await?;

    let sites: Vec<model::CachedSite> = result["result"]["data"]
        .as_array()
        .with_context(|| {
            format!("Expected array at result.data in getSiteList response. Got: {result}")
        })?
        .iter()
        .filter_map(|v| {
            let id = v
                .get("id")
                .and_then(|v| v.as_str())
                .or_else(|| v.get("siteId").and_then(|v| v.as_str()))?
                .to_string();
            let name = v.get("name").and_then(|v| v.as_str())?.to_string();
            Some(model::CachedSite { id, name })
        })
        .collect();

    let site_list = model::SiteList { sites };
    cache::save_sites(omadac_id, &site_list)?;
    Ok(site_list.sites)
}
