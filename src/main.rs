use std::thread::available_parallelism;

use clap::{Parser, Subcommand};
use glommio::GlommioError;

#[derive(Parser)]
#[command(author, version, about, long_about=None)]
struct Cli {
    #[clap(long)]
    concurrency: Option<usize>,

    #[command(subcommand)]
    command: Option<Commands>
}

#[derive(Subcommand)]
enum Commands {
    Echo,
}

fn main() -> Result<(), GlommioError<()>> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let concurrency = match cli.concurrency {
        Some(concurrency) => concurrency,
        None => match available_parallelism() {
            Ok(parallelism) => parallelism.into(),
            Err(e) => {
                error!(
                    "Failed to get available parallelism: {}. Starting on 1 core",
                    e
                );
                1
            }
        },
    };

    match &cli.command {
        Some(Commands::Echo) => rust_proxy::echo::run(concurrency),
        _ => rust_proxy::proxy::run(concurrency)
    }
}
