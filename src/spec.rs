use anyhow::{Context, Result};
use openapiv3::{OpenAPI, Operation, Parameter, PathItem, ReferenceOr, RequestBody};
use std::collections::HashSet;

use crate::model::{ApiOperation, ApiParam, ApiSpec, ParamLocation};

/// Minimal view used by `omada list`.
pub struct ListOperation {
    pub method: String,
    pub path: String,
    pub operation_id: String,
    pub tag: Option<String>,
}

pub async fn fetch(client: &reqwest::Client, base_url: &str) -> Result<OpenAPI> {
    let url = format!("{base_url}/v3/api-docs/00%20All");
    let spec: OpenAPI = client
        .get(&url)
        .send()
        .await
        .context("Failed to fetch OpenAPI spec")?
        .json()
        .await
        .context("Failed to parse OpenAPI spec")?;
    Ok(spec)
}

pub fn convert(openapi: &OpenAPI) -> ApiSpec {
    let mut operations = Vec::new();
    let root_value = serde_json::to_value(openapi).unwrap_or(serde_json::Value::Null);

    for (path, path_item) in openapi.paths.iter() {
        let ReferenceOr::Item(path_item) = path_item else {
            continue;
        };
        for (method, operation) in iter_operations(path_item) {
            let Some(operation_id) = &operation.operation_id else {
                continue;
            };
            let parameters = collect_params(operation);
            let has_request_body = operation.request_body.is_some();
            let request_body_schema = operation
                .request_body
                .as_ref()
                .and_then(|rb| extract_request_schema(rb, &root_value));
            operations.push(ApiOperation {
                operation_id: operation_id.clone(),
                method: method.to_uppercase(),
                path: path.clone(),
                summary: operation.summary.clone(),
                tag: operation.tags.first().cloned(),
                parameters,
                has_request_body,
                request_body_schema,
            });
        }
    }

    operations.sort_by(|a, b| a.operation_id.cmp(&b.operation_id));
    ApiSpec { operations }
}

/// Recursively inline `$ref` pointers in a JSON value using `root` as the lookup base.
/// Keeps a visited set for cycle safety; cyclic refs are left as `{"$ref": ...}`.
fn resolve_refs(
    value: &mut serde_json::Value,
    root: &serde_json::Value,
    visited: &mut HashSet<String>,
) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(ref_str) = map.get("$ref").and_then(|v| v.as_str()).map(String::from)
                && let Some(target) = ref_str.strip_prefix('#').and_then(|p| root.pointer(p))
            {
                if visited.contains(&ref_str) {
                    return;
                }
                let mut resolved = target.clone();
                visited.insert(ref_str.clone());
                resolve_refs(&mut resolved, root, visited);
                visited.remove(&ref_str);
                *value = resolved;
                return;
            }
            for v in map.values_mut() {
                resolve_refs(v, root, visited);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr.iter_mut() {
                resolve_refs(v, root, visited);
            }
        }
        _ => {}
    }
}

/// Pull out the JSON schema for an operation's request body, with `$ref`s inlined.
/// Prefers `application/json`, falls back to the first content type.
fn extract_request_schema(
    body_ref: &ReferenceOr<RequestBody>,
    root: &serde_json::Value,
) -> Option<String> {
    let mut body_val = serde_json::to_value(body_ref).ok()?;
    resolve_refs(&mut body_val, root, &mut HashSet::new());
    let content = body_val.get("content")?;
    let media = content
        .get("application/json")
        .or_else(|| content.as_object().and_then(|o| o.values().next()))?;
    let schema = media.get("schema")?.clone();
    serde_json::to_string_pretty(&schema).ok()
}

pub fn list_operations(spec: &ApiSpec) -> Vec<ListOperation> {
    spec.operations
        .iter()
        .map(|op| ListOperation {
            method: op.method.clone(),
            path: op.path.clone(),
            operation_id: op.operation_id.clone(),
            tag: op.tag.clone(),
        })
        .collect()
}

fn collect_params(operation: &Operation) -> Vec<ApiParam> {
    let mut params = Vec::new();
    for param_ref in &operation.parameters {
        let ReferenceOr::Item(param) = param_ref else {
            continue;
        };
        let (data, location) = match param {
            Parameter::Path { parameter_data, .. } => (parameter_data, ParamLocation::Path),
            Parameter::Query { parameter_data, .. } => (parameter_data, ParamLocation::Query),
            _ => continue,
        };
        params.push(ApiParam {
            name: data.name.clone(),
            location,
            required: data.required,
            description: data.description.clone(),
        });
    }
    params
}

fn iter_operations(item: &PathItem) -> Vec<(&'static str, &Operation)> {
    let mut ops = Vec::new();
    if let Some(op) = &item.get {
        ops.push(("get", op));
    }
    if let Some(op) = &item.post {
        ops.push(("post", op));
    }
    if let Some(op) = &item.put {
        ops.push(("put", op));
    }
    if let Some(op) = &item.patch {
        ops.push(("patch", op));
    }
    if let Some(op) = &item.delete {
        ops.push(("delete", op));
    }
    ops
}
