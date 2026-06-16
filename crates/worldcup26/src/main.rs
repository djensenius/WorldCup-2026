//! WorldCup26 — a terminal UI for the FIFA World Cup 2026.
//!
//! Module layout:
//! - [`cli`]: command-line argument parsing.
//! - [`config`]: persisted user configuration.
//! - [`logging`]: file-based tracing setup.
//! - [`timefmt`]: kickoff-time formatting and timezone conversion.
//! - [`tui`]: terminal init/restore and the panic hook.
//! - [`event`]: the async input/tick event source.
//! - [`data`]: off-thread data loading (`Remote`, `Poller`).
//! - [`app`]: application state and the main loop.
//! - [`ui`]: rendering (tabs, screens, status bar, toasts, theme).

mod app;
mod cli;
mod config;
mod data;
mod event;
mod logging;
mod timefmt;
mod tui;
mod ui;

use anyhow::{Result, bail};
use clap::Parser;
use time::UtcOffset;
use wc_data::{Http, Provider};

use crate::app::App;
use crate::cli::Cli;
use crate::config::{Config, TimezonePref};

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Capture the local UTC offset on the main thread before any worker
    // threads spawn — the `time` crate refuses to read it once a process is
    // multi-threaded (a tokio runtime spawns workers).
    let local_offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(run(cli, local_offset))
}

async fn run(cli: Cli, local_offset: UtcOffset) -> Result<()> {
    let config_path = match cli.config.as_deref() {
        Some(path) => path.to_path_buf(),
        None => Config::default_path()?,
    };

    let persisted_config = Config::load_from(&config_path)?;
    let mut config = persisted_config.clone();
    config.merge_env();
    if let Some(provider) = cli.provider.as_deref() {
        config.provider.kind = provider.to_owned();
    }
    if let Some(theme) = cli.theme.as_deref() {
        config.ui.theme = theme.to_owned();
    }
    if let Some(timezone) = cli.timezone.as_deref() {
        config.ui.timezone = parse_timezone(timezone)?;
    }
    if cli.nerd_fonts {
        config.ui.nerd_fonts = true;
    }
    if cli.no_nerd_fonts {
        config.ui.nerd_fonts = false;
    }
    if cli.flags {
        config.ui.show_flags = true;
    }
    if cli.no_flags {
        config.ui.show_flags = false;
    }
    if cli.no_color {
        "high-contrast".clone_into(&mut config.ui.theme);
    }

    let _log_guard = logging::init();

    let http = Http::new()?;
    let (provider, startup_warning) =
        match Provider::from_config(&config.provider_config(), http.clone()) {
            Ok(provider) => (provider, None),
            Err(err) => {
                // Fall back to ESPN (no key required) so the app is always usable.
                let warning = format!("{err}; falling back to the ESPN provider.");
                (
                    Provider::Espn(wc_data::backends::EspnProvider::new(http)),
                    Some(warning),
                )
            }
        };

    let mut app = App::new(
        config,
        persisted_config,
        config_path,
        provider,
        local_offset,
        crate::ui::flag_image::make_picker(cli.graphics.as_deref())
            .map(crate::ui::flag_image::FlagStore::new),
    );
    if let Some(warning) = startup_warning {
        app.warn(warning);
    }

    let mut terminal = tui::init()?;
    let result = app.run(&mut terminal).await;
    tui::restore()?;
    result
}

fn parse_timezone(value: &str) -> Result<TimezonePref> {
    let value = value.trim();
    match value.to_ascii_lowercase().as_str() {
        "local" => Ok(TimezonePref::Local),
        "utc" => Ok(TimezonePref::Utc),
        _ => {
            let hours = value.parse::<i8>().map_err(|_| {
                anyhow::anyhow!("timezone must be `local`, `utc`, or an hour offset like `-4`")
            })?;
            if (-23..=23).contains(&hours) {
                Ok(TimezonePref::FixedOffset(hours))
            } else {
                bail!("timezone hour offset must be between -23 and 23")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TimezonePref, parse_timezone};

    #[test]
    fn parse_timezone_trims_fixed_offsets() {
        assert!(matches!(
            parse_timezone(" -4 "),
            Ok(TimezonePref::FixedOffset(-4))
        ));
    }
}
