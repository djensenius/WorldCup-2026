//! Transient toast notifications shown over the body area.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Toast severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Informational.
    Info,
    /// Warning.
    Warn,
    /// Error.
    Error,
}

/// A single toast message with an expiry.
#[derive(Debug, Clone)]
pub struct Toast {
    /// Severity.
    pub level: Level,
    /// Message text.
    pub text: String,
    /// When the toast should disappear.
    pub expires_at: Instant,
}

/// How long toasts remain visible.
const TTL: Duration = Duration::from_secs(5);

/// A small queue of active toasts.
#[derive(Debug, Default)]
pub struct Toasts {
    items: VecDeque<Toast>,
}

impl Toasts {
    /// Push an informational toast.
    pub fn info(&mut self, text: impl Into<String>) {
        self.push(Level::Info, text);
    }

    /// Push a warning toast.
    pub fn warn(&mut self, text: impl Into<String>) {
        self.push(Level::Warn, text);
    }

    /// Push an error toast.
    pub fn error(&mut self, text: impl Into<String>) {
        self.push(Level::Error, text);
    }

    fn push(&mut self, level: Level, text: impl Into<String>) {
        self.items.push_back(Toast {
            level,
            text: text.into(),
            expires_at: Instant::now() + TTL,
        });
        while self.items.len() > 4 {
            self.items.pop_front();
        }
    }

    /// Drop expired toasts. Call each tick.
    pub fn expire(&mut self) {
        let now = Instant::now();
        self.items.retain(|t| t.expires_at > now);
    }

    /// The currently visible toasts, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = &Toast> {
        self.items.iter()
    }

    /// Whether there are any toasts to draw.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}
