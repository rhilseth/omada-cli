mod auth;
mod cache;
mod execute;
mod model;
mod spec;

use anyhow::Result;
use clap::{Arg, Command};
use model::{ApiSpec, ParamLocation};
use std::collections::HashMap;

/// Leak a String into a &'static str. Acceptable for a short-lived CLI process.
fn s(string: String) -> &'static str {
    Box::leak(string.into_boxed_str())
}

fn build_command(spec: &ApiSpec) -> Command {
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
        );

    for op in &spec.operations {
        let op_id = s(op.operation_id.clone());
        let about = s(op.summary.clone().unwrap_or_default());
        let mut sub = Command::new(op_id).about(about);

        for param in &op.parameters {
            let flag = s(execute::camel_to_kebab(&param.name));
            let value_name = s(param.name.to_uppercase());
            let mut arg = Arg::new(flag).long(flag).value_name(value_name);

            if let Some(desc) = &param.description {
                arg = arg.help(s(desc.clone()));
            }

            if param.location == ParamLocation::Path || param.required {
                arg = arg.required(true);
            }

            sub = sub.arg(arg);
        }

        if op.has_request_body {
            sub = sub.arg(
                Arg::new("json")
                    .long("json")
                    .value_name("JSON")
                    .help("Request body as JSON string"),
            );
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

#[tokio::main]
async fn main() -> Result<()> {
    let config = auth::Config::from_env()?;

    let ssl_verify = std::env::var("OMADA_SSL_VERIFY").as_deref() == Ok("true");
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(!ssl_verify)
        .build()?;

    let omadac_id = auth::get_omadac_id(&client, &config.base_url).await?;
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

        Some((op_name, sub_m)) => {
            let op = api_spec
                .operations
                .iter()
                .find(|op| op.operation_id == op_name)
                .expect("operation exists — it was built from the spec");

            let session = auth::authenticate(&client, &config).await?;

            let mut params = HashMap::new();
            for param in &op.parameters {
                let flag = execute::camel_to_kebab(&param.name);
                if let Some(val) = sub_m.get_one::<String>(&flag) {
                    params.insert(flag, val.clone());
                }
            }
            let json_body = op
                .has_request_body
                .then(|| sub_m.get_one::<String>("json").cloned())
                .flatten();

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
