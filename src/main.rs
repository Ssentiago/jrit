pub mod commands;
pub mod config;
pub mod pipeline;

use crate::pipeline::pipeline_runner::run_pipeline;
use clap::{Parser, Subcommand};
use owo_colors::OwoColorize;

#[derive(Parser)]
#[command(name = "jrit", about = "Automated release tool")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Initialize jrit.toml interactively and create release workflow
    Init,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Some(Command::Init) => commands::init::run_init().await,
        None => run_pipeline().await,
    };

    if let Err(e) = result {
        eprintln!("{} {e:#}", "Error:".red());
        std::process::exit(1);
    }
}
