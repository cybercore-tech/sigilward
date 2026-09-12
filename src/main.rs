mod baseline;
mod config;
mod diff;
mod format;

use clap::{Parser, Subcommand};
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(name = "sigilward", version = "0.1.0", about = "File-integrity monitor — baseline a set of paths, detect drift")]
struct Args {
    #[command(subcommand)]
    command: Commands,

    /// Path to config.toml. Defaults to $XDG_CONFIG_HOME/sigilward/config.toml,
    /// then ~/.config/sigilward/config.toml, then ./config.toml.
    #[arg(short, long, global = true)]
    config: Option<std::path::PathBuf>,

    /// Disable cybercore color output.
    #[arg(long, global = true)]
    no_color: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Walk every watched path and write a fresh baseline, replacing any
    /// existing one. Run this once, when you're confident the current
    /// state is trustworthy.
    Init,
    /// Compare current state against the stored baseline and report drift.
    /// Exits non-zero if anything changed — for cron/scripting use.
    Check,
    /// Same as `init`, but only after you've reviewed `check`'s output and
    /// decided the current state should become the new trusted baseline.
    Update,
}

fn load_config(args: &Args) -> Result<config::AppConfig, String> {
    let path = args.config.clone().or_else(config::default_config_path).ok_or_else(|| {
        "No config found (checked --config, $XDG_CONFIG_HOME/sigilward, ~/.config/sigilward, ./config.toml)".to_string()
    })?;
    config::load(&path)
}

fn main() -> ExitCode {
    let args = Args::parse();

    let cfg = match load_config(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("sigilward: {e}");
            return ExitCode::FAILURE;
        }
    };
    let baseline_path = config::expand_home(&cfg.baseline_path);

    match args.command {
        Commands::Init | Commands::Update => {
            println!("sigilward: walking {} watch entries...", cfg.watch.len());
            let fresh = baseline::build(&cfg.watch);
            match baseline::save(&fresh, &baseline_path) {
                Ok(()) => {
                    println!("sigilward: baseline written to {} ({} files)", baseline_path.display(), fresh.files.len());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("sigilward: failed to write baseline: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Commands::Check => {
            let stored = match baseline::load(&baseline_path) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("sigilward: failed to load baseline from {} ({e}) — run `sigilward init` first", baseline_path.display());
                    return ExitCode::FAILURE;
                }
            };
            let current = baseline::build(&cfg.watch);
            let changes = diff::diff(&stored, &current);
            print!("{}", format::render_report(&changes, !args.no_color));
            if changes.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
    }
}
