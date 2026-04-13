mod auth;
mod cache;
mod config;
mod execute;
mod model;
mod sites;
mod spec;

use anyhow::{Context, Result};
use clap::{Arg, Command};
use model::{ApiSpec, ParamLocation};
use std::collections::HashMap;

/// Leak a String into a &'static str. Acceptable for a short-lived CLI process.
fn s(string: String) -> &'static str {
    Box::leak(string.into_boxed_str())
}

/// Returns true when the parameter description indicates millisecond timestamps.
///
/// Detects explicit labels ("unit: ms", "millisecond") and also infers from
/// example timestamps: a 13-digit decimal in the description is almost certainly
/// epoch-milliseconds (e.g. "support field 1679297710438").
fn param_uses_ms(description: &Option<String>) -> bool {
    let Some(d) = description.as_deref() else {
        return false;
    };
    if d.contains("unit: ms") || d.contains("unit:ms") || d.contains("millisecond") {
        return true;
    }
    // Detect a 13-digit example timestamp embedded in the description.
    let mut run = 0usize;
    for c in d.chars() {
        if c.is_ascii_digit() {
            run += 1;
            if run == 13 {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

/// Resolve a `--start` / `--end` value into the right unit (seconds or ms).
///
/// Accepted forms:
///   `now`          → current time
///   `<N>m`         → N minutes ago
///   `<N>h`         → N hours ago
///   `<N>d`         → N days ago
///   `<N>w`         → N weeks ago
///   raw integer    → passed through unchanged (caller's responsibility)
fn resolve_time(value: &str, use_ms: bool) -> anyhow::Result<String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let offset_secs: Option<u64> = match value {
        "now" => Some(0),
        v if v.ends_with('m') => v[..v.len() - 1].parse::<u64>().ok().map(|n| n * 60),
        v if v.ends_with('h') => v[..v.len() - 1].parse::<u64>().ok().map(|n| n * 3_600),
        v if v.ends_with('d') => v[..v.len() - 1].parse::<u64>().ok().map(|n| n * 86_400),
        v if v.ends_with('w') => v[..v.len() - 1].parse::<u64>().ok().map(|n| n * 7 * 86_400),
        _ => None,
    };

    match offset_secs {
        Some(offset) => {
            let ts = now.saturating_sub(offset);
            Ok(if use_ms {
                (ts * 1_000).to_string()
            } else {
                ts.to_string()
            })
        }
        None => Ok(value.to_string()), // raw integer — pass through
    }
}

fn build_command(spec: &ApiSpec) -> Command {
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut cmd = Command::new("omada")
        .about("CLI for Omada controller OpenAPI")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("auth")
                .about("Authenticate with the Omada controller and verify credentials"),
        )
        .subcommand(
            Command::new("list")
                .about("List all available API operations")
                .arg(
                    Arg::new("tag")
                        .long("tag")
                        .value_name("TAG")
                        .help("Filter by tag (e.g. \"Device\", \"Client\")"),
                ),
        )
        .subcommand(
            Command::new("spec")
                .about("Manage the cached API spec")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("refresh")
                        .about("Delete the cached spec and re-fetch from the controller"),
                ),
        )
        .subcommand(
            Command::new("sites")
                .about("Manage the cached site list")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("refresh")
                        .about("Delete the cached site list and re-fetch from the controller"),
                ),
        )
        .subcommand(
            Command::new("schema")
                .about("Show the parameters and request-body schema for an operation")
                .arg(
                    Arg::new("operation")
                        .required(true)
                        .help("Operation ID (e.g. createSomething)"),
                ),
        );

    for op in &spec.operations {
        let op_id = s(op.operation_id.clone());
        let about = s(op.summary.clone().unwrap_or_default());
        let mut sub = Command::new(op_id).about(about);
        let mut has_site_id = false;

        for param in &op.parameters {
            if param.name == "omadacId" {
                continue;
            }
            let flag = s(execute::camel_to_kebab(&param.name));
            let value_name = s(param.name.to_uppercase());
            let mut arg = Arg::new(flag).long(flag).value_name(value_name);

            if let Some(desc) = &param.description {
                arg = arg.help(s(desc.clone()));
            }

            match param.name.as_str() {
                "page" => arg = arg.default_value("1"),
                "pageSize" => arg = arg.default_value("20"),
                "start" => {
                    let use_ms = param_uses_ms(&param.description);
                    let v = now_secs.saturating_sub(86_400);
                    arg = arg.default_value(s(if use_ms {
                        (v * 1_000).to_string()
                    } else {
                        v.to_string()
                    }));
                }
                "end" => {
                    let use_ms = param_uses_ms(&param.description);
                    arg = arg.default_value(s(if use_ms {
                        (now_secs * 1_000).to_string()
                    } else {
                        now_secs.to_string()
                    }));
                }
                "siteId" => {
                    has_site_id = true;
                    // Never required: auto-resolved from site list if omitted
                }
                _ if param.location == ParamLocation::Path || param.required => {
                    arg = arg.required(true);
                }
                _ => {}
            }

            sub = sub.arg(arg);
        }

        if has_site_id {
            sub = sub.arg(
                Arg::new("site")
                    .long("site")
                    .value_name("SITE_NAME")
                    .help("Site name (alternative to --site-id; looked up from cache)"),
            );
        }

        if op.has_request_body {
            sub = sub.arg(
                Arg::new("json")
                    .long("json")
                    .value_name("JSON")
                    .help("Request body as JSON string"),
            );

            // Some endpoints put `page`/`pageSize` in the request body rather than
            // as query params. Synthesize flags so the usual defaults apply; they
            // get merged into the body at execute time. Skip if the same name is
            // already a query param (guard against double-definition).
            let has_query_page = op.parameters.iter().any(|p| p.name == "page");
            let has_query_page_size = op.parameters.iter().any(|p| p.name == "pageSize");
            if op.body_has_page && !has_query_page {
                sub = sub.arg(
                    Arg::new("page")
                        .long("page")
                        .value_name("PAGE")
                        .default_value("1")
                        .help("Start page number (merged into request body)"),
                );
            }
            if op.body_has_page_size && !has_query_page_size {
                sub = sub.arg(
                    Arg::new("page-size")
                        .long("page-size")
                        .value_name("PAGESIZE")
                        .default_value("20")
                        .help("Entries per page (merged into request body)"),
                );
            }
        }

        cmd = cmd.subcommand(sub);
    }

    cmd
}

async fn get_or_fetch_spec(
    client: &reqwest::Client,
    base_url: &str,
    omadac_id: &str,
) -> Result<ApiSpec> {
    if let Some(cached) = cache::load(omadac_id) {
        return Ok(cached);
    }
    let openapi = spec::fetch(client, base_url).await?;
    let api_spec = spec::convert(&openapi);
    cache::save(omadac_id, &api_spec)?;
    Ok(api_spec)
}

fn cmd_config() -> Result<()> {
    let matches = Command::new("omada config")
        .about("Save controller credentials to ~/.omadacli/config.toml")
        .arg(
            Arg::new("base-url")
                .long("base-url")
                .value_name("URL")
                .required(true)
                .help("Controller base URL (e.g. https://192.168.1.1:8043)"),
        )
        .arg(
            Arg::new("client-id")
                .long("client-id")
                .value_name("ID")
                .required(true)
                .help("OpenAPI client ID"),
        )
        .arg(
            Arg::new("client-secret")
                .long("client-secret")
                .value_name("SECRET")
                .required(true)
                .help("OpenAPI client secret"),
        )
        .arg(
            Arg::new("ssl-verify")
                .long("ssl-verify")
                .action(clap::ArgAction::SetTrue)
                .help("Enable TLS certificate verification (default: off)"),
        )
        .get_matches_from(std::env::args().skip(1));

    let cfg = config::Config {
        base_url: matches.get_one::<String>("base-url").unwrap().clone(),
        client_id: matches.get_one::<String>("client-id").unwrap().clone(),
        client_secret: matches.get_one::<String>("client-secret").unwrap().clone(),
        ssl_verify: matches.get_flag("ssl-verify"),
    };
    cfg.save()?;

    let path = config::config_path().unwrap();
    println!("Config saved to {}", path.display());
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // `omada config` needs no credentials — handle before loading config
    if std::env::args().nth(1).as_deref() == Some("config") {
        return cmd_config();
    }

    let config = config::Config::load()?;

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(!config.ssl_verify)
        .build()?;

    let omadac_id = match cache::find_omadac_id() {
        Some(id) => id,
        None => auth::get_omadac_id(&client, &config.base_url).await?,
    };
    let api_spec = get_or_fetch_spec(&client, &config.base_url, &omadac_id).await?;

    let matches = build_command(&api_spec).get_matches();

    match matches.subcommand() {
        Some(("auth", _)) => {
            let session = auth::authenticate(&client, &config).await?;
            println!("Authenticated successfully.");
            println!("Controller ID: {}", session.omadac_id);
            println!("Token type:    {}", session.token_type);
            println!("Expires in:    {}s", session.expires_in);
        }

        Some(("list", sub_m)) => {
            let ops = spec::list_operations(&api_spec);
            let tag = sub_m.get_one::<String>("tag");

            let filtered: Vec<_> = match tag {
                Some(filter) => ops
                    .iter()
                    .filter(|op| {
                        op.tag
                            .as_deref()
                            .is_some_and(|t| t.eq_ignore_ascii_case(filter))
                    })
                    .collect(),
                None => ops.iter().collect(),
            };

            println!("{:<40} {:<8} PATH", "OPERATION ID", "METHOD");
            println!("{}", "-".repeat(80));
            for op in &filtered {
                println!("{:<40} {:<8} {}", op.operation_id, op.method, op.path);
            }
            println!("\n{} operations", filtered.len());
        }

        Some(("spec", sub_m)) => {
            if let Some(("refresh", _)) = sub_m.subcommand() {
                cache::delete(&omadac_id)?;
                println!("Fetching spec from controller...");
                let openapi = spec::fetch(&client, &config.base_url).await?;
                let fresh = spec::convert(&openapi);
                cache::save(&omadac_id, &fresh)?;
                println!("Spec cached ({} operations).", fresh.operations.len());
            }
        }

        Some(("schema", sub_m)) => {
            let op_id = sub_m.get_one::<String>("operation").unwrap();
            let op = api_spec
                .operations
                .iter()
                .find(|op| &op.operation_id == op_id)
                .ok_or_else(|| anyhow::anyhow!("Unknown operation: {op_id}"))?;

            println!("{} {}", op.method, op.path);
            if let Some(summary) = &op.summary {
                println!("{summary}");
            }

            let visible_params: Vec<_> = op
                .parameters
                .iter()
                .filter(|p| p.name != "omadacId")
                .collect();
            if !visible_params.is_empty() {
                println!("\nParameters:");
                for p in &visible_params {
                    let loc = match p.location {
                        ParamLocation::Path => "path",
                        ParamLocation::Query => "query",
                    };
                    let req = if p.required { ", required" } else { "" };
                    let flag = execute::camel_to_kebab(&p.name);
                    print!("  --{flag} [{loc}{req}]");
                    if let Some(desc) = &p.description {
                        print!(" — {desc}");
                    }
                    println!();
                }
            }

            if let Some(schema) = &op.request_body_schema {
                println!("\nRequest body (JSON schema):");
                println!("{schema}");
            } else if op.has_request_body {
                println!("\nRequest body: (schema not available)");
            }
        }

        Some(("sites", sub_m)) => {
            if let Some(("refresh", _)) = sub_m.subcommand() {
                cache::delete_sites(&omadac_id)?;
                let session = auth::authenticate(&client, &config).await?;
                let site_list =
                    sites::get_or_fetch(&client, &session, &api_spec, &omadac_id, &config.base_url)
                        .await?;
                println!("Sites cached ({} site(s)).", site_list.len());
                for s in &site_list {
                    println!("  {} — {}", s.name, s.id);
                }
            }
        }

        Some((op_name, sub_m)) => {
            let op = api_spec
                .operations
                .iter()
                .find(|op| op.operation_id == op_name)
                .expect("operation exists — it was built from the spec");

            let session = auth::authenticate(&client, &config).await?;

            let mut params = HashMap::new();
            for param in &op.parameters {
                if param.name == "omadacId" {
                    continue;
                }
                let flag = execute::camel_to_kebab(&param.name);
                if let Some(val) = sub_m.get_one::<String>(&flag) {
                    params.insert(flag, val.clone());
                }
            }

            // Resolve relative time strings (e.g. "7d", "24h") for any time-range param.
            // Covers exact names "start"/"end" plus anything ending in "Start"/"End"
            // (e.g. filters.timeStart, filters.timeEnd).
            for param in &op.parameters {
                let n = param.name.as_str();
                if !matches!(n, "start" | "end") && !n.ends_with("Start") && !n.ends_with("End") {
                    continue;
                }
                let flag = execute::camel_to_kebab(&param.name);
                if let Some(val) = params.get(&flag).cloned() {
                    let use_ms = param_uses_ms(&param.description);
                    params.insert(flag, resolve_time(&val, use_ms)?);
                }
            }

            // Resolve site-id: explicit --site-id > --site name > auto-detect from site list
            let has_site_id_param = op.parameters.iter().any(|p| p.name == "siteId");
            if has_site_id_param && !params.contains_key("site-id") {
                let site_list =
                    sites::get_or_fetch(&client, &session, &api_spec, &omadac_id, &config.base_url)
                        .await?;
                let site = if let Some(name) = sub_m.get_one::<String>("site") {
                    site_list
                        .iter()
                        .find(|s| s.name.eq_ignore_ascii_case(name))
                        .ok_or_else(|| {
                            let names: Vec<_> = site_list.iter().map(|s| s.name.as_str()).collect();
                            anyhow::anyhow!(
                                "Site '{}' not found. Available: {}",
                                name,
                                names.join(", ")
                            )
                        })?
                } else if site_list.len() == 1 {
                    &site_list[0]
                } else if let Some(s) = site_list
                    .iter()
                    .find(|s| s.name.eq_ignore_ascii_case("Default"))
                {
                    s
                } else {
                    let names: Vec<_> = site_list.iter().map(|s| s.name.as_str()).collect();
                    anyhow::bail!(
                        "Multiple sites found; specify --site-id or --site. Available: {}",
                        names.join(", ")
                    );
                };
                params.insert("site-id".to_string(), site.id.clone());
            }

            let json_body = if op.has_request_body {
                let has_query_page = op.parameters.iter().any(|p| p.name == "page");
                let has_query_page_size = op.parameters.iter().any(|p| p.name == "pageSize");
                let inject_page = op.body_has_page && !has_query_page;
                let inject_page_size = op.body_has_page_size && !has_query_page_size;

                let user_json = sub_m.get_one::<String>("json").cloned();
                if inject_page || inject_page_size {
                    let mut body: serde_json::Value = match user_json.as_deref() {
                        Some(s) => {
                            serde_json::from_str(s).context("--json did not parse as JSON")?
                        }
                        None => serde_json::json!({}),
                    };
                    if let Some(obj) = body.as_object_mut() {
                        if inject_page
                            && !obj.contains_key("page")
                            && let Some(v) = sub_m.get_one::<String>("page")
                        {
                            obj.insert("page".into(), v.parse::<i64>()?.into());
                        }
                        if inject_page_size
                            && !obj.contains_key("pageSize")
                            && let Some(v) = sub_m.get_one::<String>("page-size")
                        {
                            obj.insert("pageSize".into(), v.parse::<i64>()?.into());
                        }
                    }
                    Some(serde_json::to_string(&body)?)
                } else {
                    user_json
                }
            } else {
                None
            };

            let result = execute::run(
                &client,
                &session,
                op,
                &params,
                json_body.as_deref(),
                &config.base_url,
            )
            .await?;

            println!("{}", serde_json::to_string_pretty(&result)?);
        }

        None => unreachable!(),
    }

    Ok(())
}
