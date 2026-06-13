//! Persisted user configuration.
//!
//! Stored as TOML in the platform config directory. Holds the data-provider
//! selection and API keys, display preferences (theme, icons, timezone), and
//! the user's favourite teams. API keys may also be supplied via the
//! environment (`WC26_API_FOOTBALL_KEY`, `WC26_FOOTBALL_DATA_KEY`).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use wc_data::{ProviderConfig, ProviderKind};

/// How kickoff times are displayed.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TimezonePref {
    /// Convert to the system local timezone (default).
    #[default]
    Local,
    /// Show UTC.
    Utc,
    /// A fixed offset in whole hours from UTC (e.g. `-4`).
    FixedOffset(i8),
}

/// Top-level configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Provider selection and keys.
    pub provider: ProviderSettings,
    /// Display preferences.
    pub ui: UiSettings,
    /// Favourite teams, by display name or abbreviation (case-insensitive).
    pub favorites: Vec<String>,
}

/// Provider selection and credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderSettings {
    /// Which backend to use (`espn`, `api-football`, `football-data`).
    pub kind: String,
    /// API key for API-Football, if used.
    pub api_football_key: Option<String>,
    /// API token for football-data.org, if used.
    pub football_data_key: Option<String>,
}

/// Display preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSettings {
    /// Colour theme name.
    pub theme: String,
    /// Use Nerd Font glyphs for icons.
    pub nerd_fonts: bool,
    /// Show colored ASCII-art flags.
    pub show_flags: bool,
    /// How to display kickoff times.
    pub timezone: TimezonePref,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            provider: ProviderSettings::default(),
            ui: UiSettings::default(),
            favorites: Vec::new(),
        }
    }
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            kind: ProviderKind::default().as_str().to_owned(),
            api_football_key: None,
            football_data_key: None,
        }
    }
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            theme: "world-night".to_owned(),
            nerd_fonts: false,
            show_flags: true,
            timezone: TimezonePref::default(),
        }
    }
}

impl Config {
    /// The default config file path for this platform.
    ///
    /// # Errors
    /// Returns an error if no config directory can be determined.
    pub fn default_path() -> Result<PathBuf> {
        let dirs = directories::ProjectDirs::from("dev", "djensenius", "wc26")
            .context("could not determine a config directory")?;
        Ok(dirs.config_dir().join("config.toml"))
    }

    /// Load config from `path`, returning defaults if the file does not exist.
    ///
    /// # Errors
    /// Returns an error if the file exists but cannot be read or parsed.
    pub fn load_from(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading config at {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing config at {}", path.display()))
    }

    /// Write the config to `path`, creating parent directories as needed.
    ///
    /// # Errors
    /// Returns an error if the file or its parent directory cannot be written.
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating config directory {}", parent.display()))?;
        }
        let text = toml::to_string_pretty(self).context("serialising config")?;
        std::fs::write(path, text).with_context(|| format!("writing config to {}", path.display()))
    }

    /// Overlay API keys from the environment when set.
    pub fn merge_env(&mut self) {
        if let Ok(key) = std::env::var("WC26_API_FOOTBALL_KEY")
            && !key.trim().is_empty()
        {
            self.provider.api_football_key = Some(key);
        }
        if let Ok(key) = std::env::var("WC26_FOOTBALL_DATA_KEY")
            && !key.trim().is_empty()
        {
            self.provider.football_data_key = Some(key);
        }
    }

    /// Resolve the [`ProviderConfig`] consumed by `wc-data`. Unknown provider
    /// names fall back to ESPN.
    #[must_use]
    pub fn provider_config(&self) -> ProviderConfig {
        ProviderConfig {
            kind: ProviderKind::parse(&self.provider.kind).unwrap_or_default(),
            api_football_key: self.provider.api_football_key.clone(),
            football_data_key: self.provider.football_data_key.clone(),
        }
    }

    /// Whether `name` (team display name or abbreviation) is a favourite.
    #[must_use]
    pub fn is_favorite(&self, name: &str, abbreviation: &str) -> bool {
        self.favorites
            .iter()
            .any(|f| f.eq_ignore_ascii_case(name) || f.eq_ignore_ascii_case(abbreviation))
    }

    /// Toggle a team's favourite status. If the team (matched by display name or
    /// abbreviation, case-insensitively) is already a favourite it is removed;
    /// otherwise its display `name` is added. Returns the new favourite state.
    pub fn toggle_favorite(&mut self, name: &str, abbreviation: &str) -> bool {
        if self.is_favorite(name, abbreviation) {
            self.favorites
                .retain(|f| !f.eq_ignore_ascii_case(name) && !f.eq_ignore_ascii_case(abbreviation));
            false
        } else {
            self.favorites.push(name.to_owned());
            true
        }
    }
}
