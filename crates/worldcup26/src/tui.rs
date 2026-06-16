//! Terminal setup and teardown.
//!
//! [`init`] switches the terminal into raw mode, the alternate screen, and
//! enables mouse capture, then installs a panic hook that restores the terminal
//! before the default hook runs — otherwise a panic would leave the shell in a
//! broken state.

use std::io::{self, Stdout};

use anyhow::Result;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

/// The concrete terminal type used throughout the app.
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Enter raw mode and the alternate screen, enable mouse capture, install the
/// panic hook, and build the terminal.
///
/// # Errors
/// Returns an error if the terminal cannot be switched into raw mode or the
/// alternate screen.
pub fn init() -> Result<Tui> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    set_panic_hook();
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

/// Leave the alternate screen, disable mouse capture and raw mode. Safe to call
/// more than once.
///
/// # Errors
/// Returns an error if the terminal cannot be restored.
pub fn restore() -> Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}

/// Install a panic hook that restores the terminal before delegating to the
/// previously-installed hook.
fn set_panic_hook() {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore();
        hook(info);
    }));
}
