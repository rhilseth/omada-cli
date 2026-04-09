use anyhow::{Context, Result, anyhow};
use openapiv3::{OpenAPI, Operation, Parameter, ReferenceOr};
use std::collections::HashMap;

use crate::auth::Session;

/// Convert camelCase param name to kebab-case CLI flag name.
/// e.g. "siteId" → "site-id", "apMac" → "ap-mac"
fn camel_to_kebab(s: &str) -> String {
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

/// Parse `["--site-id", "abc", "--page", "1"]` into a map and optional JSON body.
pub fn parse_args(raw: &[String]) -> (HashMap<String, String>, Option<String>) {
    let mut params = HashMap::new();
    let mut json_body = None;
    let mut i = 0;
    while i < raw.len() {
        if let Some(key) = raw[i].strip_prefix("--") {
            if i + 1 < raw.len() && !raw[i + 1].starts_with("--") {
                let value = raw[i + 1].clone();
                if key == "json" {
                    json_body = Some(value);
                } else {
                    params.insert(key.to_string(), value);
                }
                i += 2;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    (params, json_body)
}

fn find_operation<'a>(
    spec: &'a OpenAPI,
    operation_id: &str,
) -> Result<(String, String, &'a Operation)> {
    for (path, path_ref) in spec.paths.iter() {
        let ReferenceOr::Item(item) = path_ref else {
            continue;
        };
        let candidates = [
            ("GET", &item.get),
            ("POST", &item.post),
            ("PUT", &item.put),
            ("PATCH", &item.patch),
            ("DELETE", &item.delete),
        ];
        for (method, op_opt) in candidates {
            if let Some(op) = op_opt
                && op.operation_id.as_deref() == Some(operation_id)
            {
                return Ok((method.to_string(), path.clone(), op));
            }
        }
    }
    Err(anyhow!(
        "Operation '{}' not found. Use `omada list` to see available operations.",
        operation_id
    ))
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
    operation: &Operation,
    params: &HashMap<String, String>,
) -> Vec<(String, String)> {
    let mut query = Vec::new();
    for param_ref in &operation.parameters {
        let ReferenceOr::Item(param) = param_ref else {
            continue;
        };
        if let Parameter::Query { parameter_data, .. } = param {
            let flag = camel_to_kebab(&parameter_data.name);
            if let Some(v) = params
                .get(&flag)
                .or_else(|| params.get(&parameter_data.name))
            {
                query.push((parameter_data.name.clone(), v.clone()));
            }
        }
    }
    query
}

pub async fn run(
    client: &reqwest::Client,
    session: &Session,
    spec: &OpenAPI,
    operation_id: &str,
    params: &HashMap<String, String>,
    json_body: Option<&str>,
    base_url: &str,
) -> Result<serde_json::Value> {
    let (method, path_template, operation) = find_operation(spec, operation_id)?;
    let path = substitute_path(&path_template, session, params)?;
    let url = format!("{base_url}{path}");
    let query = collect_query_params(operation, params);

    let mut req = client
        .request(method.parse()?, &url)
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
