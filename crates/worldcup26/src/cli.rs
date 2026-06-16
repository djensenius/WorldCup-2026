//! Command-line arguments.

use std::path::PathBuf;

use clap::{ArgAction, Parser};

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

    /// Theme to use for this run, overriding the config.
    #[arg(long, value_name = "NAME")]
    pub theme: Option<String>,

    /// Kickoff timezone for this run: `local`, `utc`, or a fixed hour offset
    /// such as `-4`.
    #[arg(long, value_name = "ZONE", allow_hyphen_values = true)]
    pub timezone: Option<String>,

    /// Terminal graphics protocol for Live flags (`auto`, `kitty`, `iterm2`,
    /// `sixel`, `halfblocks`, or `off`), overriding detection.
    #[arg(
        long,
        value_name = "MODE",
        value_parser = ["auto", "kitty", "iterm2", "sixel", "halfblocks", "off"]
    )]
    pub graphics: Option<String>,

    /// Enable Nerd Font glyphs for this run, overriding the config.
    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "no_nerd_fonts")]
    pub nerd_fonts: bool,

    /// Disable Nerd Font glyphs for this run, overriding the config.
    #[arg(long, action = ArgAction::SetTrue)]
    pub no_nerd_fonts: bool,

    /// Enable Live-card flags for this run, overriding the config.
    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "no_flags")]
    pub flags: bool,

    /// Disable Live-card flags for this run, overriding the config.
    #[arg(long, action = ArgAction::SetTrue)]
    pub no_flags: bool,

    /// Disable colours (use the terminal's default foreground only).
    #[arg(long)]
    pub no_color: bool,
}
