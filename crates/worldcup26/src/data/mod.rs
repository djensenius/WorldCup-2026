//! Off-thread data loading for the UI.
//!
//! [`Remote`] is the load state of a resource; [`Poller`] drives a background
//! `tokio` task that fetches a value and hands it back to the UI thread via a
//! channel, so rendering never blocks on the network. A reload keeps the
//! previous `Ready` value visible while a refresh is in flight.

mod cache;

pub use cache::Cache;

use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

use wc_data::Provider;

/// The shared, runtime-selected data provider.
pub type SharedProvider = Arc<Provider>;

/// The load state of a fetched resource.
#[derive(Debug, Clone)]
pub enum Remote<T> {
    /// Not yet requested.
    Idle,
    /// A first load is in flight, with no previous value to show.
    Loading,
    /// A value was loaded successfully.
    Ready {
        /// The most recently loaded value.
        value: T,
        /// When it was fetched (monotonic clock).
        fetched_at: Instant,
    },
    /// The most recent load failed.
    Failed {
        /// Human-readable error message.
        error: String,
        /// When the failure occurred (monotonic clock).
        at: Instant,
    },
}

impl<T> Remote<T> {
    /// The current value, if one has ever loaded successfully.
    pub fn value(&self) -> Option<&T> {
        match self {
            Remote::Ready { value, .. } => Some(value),
            _ => None,
        }
    }

    /// How long ago the current value was fetched, if one is loaded.
    pub fn age(&self) -> Option<Duration> {
        match self {
            Remote::Ready { fetched_at, .. } => Some(fetched_at.elapsed()),
            _ => None,
        }
    }
}

/// Drives a background fetch for a single resource of type `T`.
///
/// Call [`Poller::refresh`] with a future to start a fetch, [`Poller::drain`]
/// every tick to apply a completed result, and [`Poller::is_due`] to decide
/// when to re-poll on a cadence.
pub struct Poller<T> {
    state: Remote<T>,
    rx: Option<UnboundedReceiver<Result<T, String>>>,
    in_flight: bool,
    last_refresh: Option<Instant>,
    from_cache: bool,
}

impl<T> Default for Poller<T> {
    fn default() -> Self {
        Self {
            state: Remote::Idle,
            rx: None,
            in_flight: false,
            last_refresh: None,
            from_cache: false,
        }
    }
}

impl<T: Send + 'static> Poller<T> {
    /// Create an idle poller.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The current load state.
    pub fn state(&self) -> &Remote<T> {
        &self.state
    }

    /// Whether a fetch is currently in flight.
    pub fn is_refreshing(&self) -> bool {
        self.in_flight
    }

    /// Seed the state with a value restored from the on-disk cache. The value
    /// is shown immediately and flagged stale (see [`Poller::is_stale`]), and
    /// `last_refresh` is left unset so a fresh fetch is considered due at once.
    pub fn seed(&mut self, value: T) {
        self.state = Remote::Ready {
            value,
            fetched_at: Instant::now(),
        };
        self.from_cache = true;
    }

    /// Whether the currently displayed value came from the on-disk cache and
    /// has not yet been refreshed from the network this session.
    pub fn is_stale(&self) -> bool {
        self.from_cache && matches!(self.state, Remote::Ready { .. })
    }

    /// Whether a re-poll is due: not currently fetching and either never
    /// fetched or `interval` has elapsed since the last completed fetch.
    pub fn is_due(&self, interval: Duration) -> bool {
        if self.in_flight {
            return false;
        }
        self.last_refresh
            .is_none_or(|last| last.elapsed() >= interval)
    }

    /// Spawn `fut` to load a new value. No-op if a fetch is already in flight.
    /// The future must resolve to `Ok(value)` or `Err(message)`.
    pub fn refresh<F>(&mut self, fut: F)
    where
        F: Future<Output = Result<T, String>> + Send + 'static,
    {
        if self.in_flight {
            return;
        }
        let (tx, rx) = unbounded_channel();
        self.rx = Some(rx);
        self.in_flight = true;
        if matches!(self.state, Remote::Idle) {
            self.state = Remote::Loading;
        }
        tokio::spawn(async move {
            let _ = tx.send(fut.await);
        });
    }

    /// Apply a completed fetch result, if any. Call once per tick.
    ///
    /// Returns `None` when nothing was applied this tick, `Some(Ok(()))` when a
    /// fresh value was stored, and `Some(Err(message))` when the fetch failed.
    /// On failure a previously loaded value is kept visible (so a transient
    /// error or an offline refresh does not blank the screen); the `Failed`
    /// state is only entered when there is no prior value to show.
    pub fn drain(&mut self) -> Option<Result<(), String>> {
        let rx = self.rx.as_mut()?;
        match rx.try_recv() {
            Ok(result) => {
                self.in_flight = false;
                self.rx = None;
                self.last_refresh = Some(Instant::now());
                match result {
                    Ok(value) => {
                        self.from_cache = false;
                        self.state = Remote::Ready {
                            value,
                            fetched_at: Instant::now(),
                        };
                        Some(Ok(()))
                    }
                    Err(error) => {
                        if !matches!(self.state, Remote::Ready { .. }) {
                            self.state = Remote::Failed {
                                error: error.clone(),
                                at: Instant::now(),
                            };
                        }
                        Some(Err(error))
                    }
                }
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.in_flight = false;
                self.rx = None;
                None
            }
        }
    }
}
