//! Anti-Entropator: A Local Data Lakehouse for File Organization
//!
//! Transform a chaotic downloads folder into a queryable, organized data lakehouse.

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod cli;
mod config;
mod doctor;
mod domain;
mod ingest;
mod lakehouse;
mod profile;
mod scan;

use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env().add_directive("anti_entropator=info".parse()?))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Profile(args) => {
            profile::run(args).await?;
        }
        Commands::Doctor => {
            doctor::run().await?;
        }
        Commands::Up => {
            lakehouse::check_up().await?;
        }
        Commands::Init => {
            lakehouse::init().await?;
        }
        Commands::Scan(args) => {
            scan::run(args).await?;
        }
        Commands::Ingest(args) => {
            ingest::run(args).await?;
        }
        Commands::Sql => {
            println!("SQL REPL not yet implemented");
        }
        Commands::Query { sql } => {
            println!("Query command not yet implemented: {}", sql);
        }
        Commands::Duplicates(args) => {
            println!(
                "Duplicates command not yet implemented, dump: {:?}",
                args.dump
            );
        }
        Commands::Merge { branch } => {
            println!("Merge command not yet implemented for branch: {}", branch);
        }
    }

    Ok(())
}
