use anyhow::{Context, Result};
use openapiv3::{OpenAPI, Operation, PathItem, ReferenceOr};

pub struct ApiOperation {
    pub method: String,
    pub path: String,
    pub operation_id: String,
    #[allow(dead_code)]
    pub summary: Option<String>,
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

pub fn list_operations(spec: &OpenAPI) -> Vec<ApiOperation> {
    let mut ops = Vec::new();

    for (path, path_item) in spec.paths.iter() {
        let ReferenceOr::Item(path_item) = path_item else {
            continue;
        };
        for (method, operation) in iter_operations(path_item) {
            let Some(operation_id) = &operation.operation_id else {
                continue;
            };
            ops.push(ApiOperation {
                method: method.to_uppercase(),
                path: path.clone(),
                operation_id: operation_id.clone(),
                summary: operation.summary.clone(),
                tag: operation.tags.first().cloned(),
            });
        }
    }

    ops.sort_by(|a, b| a.operation_id.cmp(&b.operation_id));
    ops
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
