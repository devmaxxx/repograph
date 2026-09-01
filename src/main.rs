mod config;
mod ids;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "repograph", version)]
struct Cli {
    /// Repository root; defaults to the current directory.
    #[arg(long, global = true, default_value = ".")]
    repo: PathBuf,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Build,
    Update,
    Ask {
        words: Vec<String>,
        #[arg(long)] json: bool,
        #[arg(long, default_value_t = 5)] seeds: usize,
        #[arg(long)] bodies: bool,
        #[arg(long)] no_dense: bool,
    },
    Explain { node: String },
    Verify,
    Bench { #[arg(long)] cases: Option<PathBuf> },
    ImportLegacy { graph_json: PathBuf },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let repo = cli.repo.canonicalize()?;
    let cfg = config::Config::load(&repo)?;
    match cli.cmd {
        _ => {
            println!("repograph: {} families configured at {}", cfg.id_families.len(), repo.display());
            Ok(())
        }
    }
}
