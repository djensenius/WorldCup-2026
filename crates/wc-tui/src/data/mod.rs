//! Off-thread data loading for the UI.
//!
//! [`Remote`] is the load state of a resource; [`Poller`] drives a background
//! `tokio` task that fetches a value and hands it back to the UI thread via a
//! channel, so rendering never blocks on the network. A reload keeps the
//! previous `Ready` value visible while a refresh is in flight.

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
}

impl<T> Default for Poller<T> {
    fn default() -> Self {
        Self {
            state: Remote::Idle,
            rx: None,
            in_flight: false,
            last_refresh: None,
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
    pub fn drain(&mut self) {
        let Some(rx) = &mut self.rx else { return };
        match rx.try_recv() {
            Ok(result) => {
                self.in_flight = false;
                self.rx = None;
                self.last_refresh = Some(Instant::now());
                self.state = match result {
                    Ok(value) => Remote::Ready {
                        value,
                        fetched_at: Instant::now(),
                    },
                    Err(error) => Remote::Failed {
                        error,
                        at: Instant::now(),
                    },
                };
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                self.in_flight = false;
                self.rx = None;
            }
        }
    }
}
