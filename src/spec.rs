use anyhow::{Context, Result};
use openapiv3::{OpenAPI, Operation, Parameter, PathItem, ReferenceOr};

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
            operations.push(ApiOperation {
                operation_id: operation_id.clone(),
                method: method.to_uppercase(),
                path: path.clone(),
                summary: operation.summary.clone(),
                tag: operation.tags.first().cloned(),
                parameters,
                has_request_body,
            });
        }
    }

    operations.sort_by(|a, b| a.operation_id.cmp(&b.operation_id));
    ApiSpec { operations }
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
