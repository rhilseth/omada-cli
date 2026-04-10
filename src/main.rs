mod auth;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "omada", about = "CLI for Omada controller OpenAPI")]
struct Cli {
    /// Accept invalid/self-signed TLS certificates
    #[arg(long, global = true)]
    insecure: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Authenticate with the Omada controller and verify credentials
    Auth,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = auth::Config::from_env()?;

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(cli.insecure)
        .build()?;

    match cli.command {
        Command::Auth => {
            let token = auth::authenticate(&client, &config).await?;
            println!("Authenticated successfully.");
            println!("Access token: {}", token.access_token);
            println!("Token type:   {}", token.token_type);
            println!("Expires in:   {}s", token.expires_in);
        }
    }

    Ok(())
}
