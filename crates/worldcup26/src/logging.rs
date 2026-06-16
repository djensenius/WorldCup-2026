//! File-based tracing setup.
//!
//! Logs go to a rolling file in the platform cache/state directory rather than
//! stderr, so diagnostics never corrupt the alternate-screen UI. Controlled by
//! `RUST_LOG` (default `info`).

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

/// Initialise tracing to a rolling log file. The returned guard must be kept
/// alive for the lifetime of the program so buffered logs are flushed.
///
/// Returns `None` if no log directory could be determined or created; the app
/// still runs, just without file logging.
#[must_use]
pub fn init() -> Option<WorkerGuard> {
    let dir = directories::ProjectDirs::from("dev", "djensenius", "worldcup26")
        .map(|d| d.cache_dir().join("logs"))?;
    std::fs::create_dir_all(&dir).ok()?;

    let file_appender = tracing_appender::rolling::daily(&dir, "worldcup26.log");
    let (writer, guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_ansi(false)
        .init();

    Some(guard)
}
