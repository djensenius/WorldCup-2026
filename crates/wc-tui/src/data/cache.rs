//! On-disk cache of the last-good provider payloads.
//!
//! Successful fetches are written to the platform cache directory as JSON and
//! reloaded on the next start, so the UI can show the most recent schedule,
//! standings, and bracket immediately while the first network refresh is still
//! in flight (and remains usable offline). All operations are best-effort: a
//! missing, unreadable, or stale file simply yields no cached value.

use std::path::PathBuf;

use serde::Serialize;
use serde::de::DeserializeOwned;

/// Reads and writes cached resources under the platform cache directory.
pub struct Cache {
    dir: Option<PathBuf>,
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}

impl Cache {
    /// Locate the cache directory (`…/wc26/data`). If it cannot be determined,
    /// the cache is silently disabled.
    #[must_use]
    pub fn new() -> Self {
        let dir = directories::ProjectDirs::from("dev", "djensenius", "wc26")
            .map(|dirs| dirs.cache_dir().join("data"));
        Self { dir }
    }

    fn path(&self, key: &str) -> Option<PathBuf> {
        self.dir.as_ref().map(|dir| dir.join(format!("{key}.json")))
    }

    /// Load and deserialize the cached value for `key`, if present and valid.
    #[must_use]
    pub fn load<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let path = self.path(key)?;
        let bytes = std::fs::read(path).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    /// Serialize and write `value` for `key`. Errors are ignored; caching is a
    /// best-effort optimisation and must never interrupt the UI.
    pub fn store<T: Serialize>(&self, key: &str, value: &T) {
        let Some(path) = self.path(key) else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(bytes) = serde_json::to_vec(value) {
            let _ = std::fs::write(path, bytes);
        }
    }
}
