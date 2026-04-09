use anyhow::{Context, Result, anyhow};
use std::collections::HashMap;

use crate::auth::Session;
use crate::model::{ApiOperation, ParamLocation};

/// Convert camelCase param name to kebab-case CLI flag name.
/// e.g. "siteId" → "site-id", "apMac" → "ap-mac"
pub fn camel_to_kebab(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_uppercase() && !out.is_empty() {
            out.push('-');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c.to_ascii_lowercase());
        }
    }
    out
}

fn substitute_path(
    template: &str,
    session: &Session,
    params: &HashMap<String, String>,
) -> Result<String> {
    let mut out = String::new();
    let mut chars = template.chars();

    while let Some(c) = chars.next() {
        if c == '{' {
            let mut name = String::new();
            for c in chars.by_ref() {
                if c == '}' {
                    break;
                }
                name.push(c);
            }
            let value = if name == "omadacId" {
                session.omadac_id.clone()
            } else {
                let flag = camel_to_kebab(&name);
                params
                    .get(&flag)
                    .ok_or_else(|| anyhow!("Missing required path parameter: --{flag}"))?
                    .clone()
            };
            out.push_str(&value);
        } else {
            out.push(c);
        }
    }

    Ok(out)
}

fn collect_query_params(
    operation: &ApiOperation,
    params: &HashMap<String, String>,
) -> Vec<(String, String)> {
    let mut query = Vec::new();
    for param in &operation.parameters {
        if param.location != ParamLocation::Query {
            continue;
        }
        let flag = camel_to_kebab(&param.name);
        if let Some(v) = params.get(&flag).or_else(|| params.get(&param.name)) {
            query.push((param.name.clone(), v.clone()));
        }
    }
    query
}

pub async fn run(
    client: &reqwest::Client,
    session: &Session,
    operation: &ApiOperation,
    params: &HashMap<String, String>,
    json_body: Option<&str>,
    base_url: &str,
) -> Result<serde_json::Value> {
    let path = substitute_path(&operation.path, session, params)?;
    let url = format!("{base_url}{path}");
    let query = collect_query_params(operation, params);

    let mut req = client
        .request(operation.method.parse()?, &url)
        .header(
            "Authorization",
            format!("AccessToken={}", session.access_token),
        )
        .query(&query);

    if let Some(body) = json_body {
        let body_val: serde_json::Value =
            serde_json::from_str(body).context("--json value is not valid JSON")?;
        req = req.json(&body_val);
    }

    let resp = req.send().await.context("Request failed")?;
    let json: serde_json::Value = resp
        .json()
        .await
        .context("Failed to parse response as JSON")?;

    Ok(json)
}
