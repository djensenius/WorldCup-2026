//! Command-line arguments.

use std::path::PathBuf;

use clap::Parser;

/// WorldCup26 — a terminal UI for the FIFA World Cup 2026.
#[derive(Debug, Parser)]
#[command(name = "worldcup26", version, about)]
pub struct Cli {
    /// Path to the config file (defaults to the platform config directory).
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Data provider to use for this run, overriding the config
    /// (`espn`, `api-football`, or `football-data`).
    #[arg(short, long, value_name = "NAME")]
    pub provider: Option<String>,

    /// Disable colours (use the terminal's default foreground only).
    #[arg(long)]
    pub no_color: bool,
}
