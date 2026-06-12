//! Asynchronous input/tick event source.
//!
//! Combines crossterm's async [`EventStream`] with a periodic tick so the UI
//! reacts to input immediately and refreshes data on a steady cadence.

use crossterm::event::{
    Event as CtEvent, EventStream, KeyEvent, KeyEventKind, MouseEvent, MouseEventKind,
};
use futures::StreamExt;
use tokio::time::{Duration, Interval, interval};

/// An event delivered to the application loop.
pub enum AppEvent {
    /// A periodic timer tick (drives polling and toast expiry).
    Tick,
    /// A key was pressed.
    Key(KeyEvent),
    /// A mouse event (click/scroll); movement is filtered out.
    Mouse(MouseEvent),
    /// The terminal was resized (the next draw re-flows automatically).
    Resize,
    /// The input stream produced an error or ended.
    Error(String),
}

/// Merges terminal input with a periodic tick.
pub struct EventLoop {
    stream: EventStream,
    ticker: Interval,
}

impl EventLoop {
    /// Create an event loop that ticks every `tick` duration.
    #[must_use]
    pub fn new(tick: Duration) -> Self {
        Self {
            stream: EventStream::new(),
            ticker: interval(tick),
        }
    }

    /// Await the next event, collapsing irrelevant terminal events (focus,
    /// paste, key-release, mouse movement) into a retry.
    pub async fn next(&mut self) -> AppEvent {
        loop {
            let Self { stream, ticker } = &mut *self;
            tokio::select! {
                _ = ticker.tick() => return AppEvent::Tick,
                maybe = stream.next() => match maybe {
                    Some(Ok(CtEvent::Key(key))) if key.kind == KeyEventKind::Press => {
                        return AppEvent::Key(key);
                    }
                    Some(Ok(CtEvent::Mouse(mouse)))
                        if !matches!(mouse.kind, MouseEventKind::Moved | MouseEventKind::Drag(_)) =>
                    {
                        return AppEvent::Mouse(mouse);
                    }
                    Some(Ok(CtEvent::Resize(_, _))) => return AppEvent::Resize,
                    Some(Ok(_)) => {}
                    Some(Err(err)) => return AppEvent::Error(err.to_string()),
                    None => return AppEvent::Error("input stream ended".to_owned()),
                },
            }
        }
    }
}
