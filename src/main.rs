mod auth;
mod cache;
mod execute;
mod model;
mod spec;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "omada", about = "CLI for Omada controller OpenAPI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Authenticate with the Omada controller and verify credentials
    Auth,

    /// List all available API operations
    List {
        /// Filter by tag (e.g. "Device", "Client")
        #[arg(long)]
        tag: Option<String>,
    },

    /// Execute an API operation by its operation ID
    Run {
        /// Operation ID (see `omada list`)
        operation_id: String,

        /// Parameters as --name value pairs. Path params (e.g. --site-id), query params,
        /// and --json '<body>' for request body.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = auth::Config::from_env()?;

    let ssl_verify = std::env::var("OMADA_SSL_VERIFY").as_deref() == Ok("true");
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(!ssl_verify)
        .build()?;

    match cli.command {
        Command::Auth => {
            let session = auth::authenticate(&client, &config).await?;
            println!("Authenticated successfully.");
            println!("Controller ID: {}", session.omadac_id);
            println!("Token type:    {}", session.token_type);
            println!("Expires in:    {}s", session.expires_in);
        }

        Command::List { tag } => {
            let openapi = spec::fetch(&client, &config.base_url).await?;
            let api_spec = spec::convert(&openapi);
            let operations = spec::list_operations(&api_spec);

            let ops: Vec<_> = match &tag {
                Some(filter) => operations
                    .iter()
                    .filter(|op| {
                        op.tag
                            .as_deref()
                            .is_some_and(|t| t.eq_ignore_ascii_case(filter))
                    })
                    .collect(),
                None => operations.iter().collect(),
            };

            println!("{:<40} {:<8} PATH", "OPERATION ID", "METHOD");
            println!("{}", "-".repeat(80));
            for op in &ops {
                println!("{:<40} {:<8} {}", op.operation_id, op.method, op.path);
            }
            println!("\n{} operations", ops.len());
        }

        Command::Run { operation_id, args } => {
            let session = auth::authenticate(&client, &config).await?;
            let api_spec = spec::fetch(&client, &config.base_url).await?;
            let (params, json_body) = execute::parse_args(&args);

            let result = execute::run(
                &client,
                &session,
                &api_spec,
                &operation_id,
                &params,
                json_body.as_deref(),
                &config.base_url,
            )
            .await?;

            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }

    Ok(())
}
