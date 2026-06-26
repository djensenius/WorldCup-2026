//! Persisted user configuration.
//!
//! Stored as TOML in the platform config directory. Holds the data-provider
//! selection and API keys, display preferences (theme, icons, timezone), and
//! the user's favourite teams. API keys may also be supplied via the
//! environment (`WORLDCUP26_API_FOOTBALL_KEY`, `WORLDCUP26_FOOTBALL_DATA_KEY`).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use wc_data::{ProviderConfig, ProviderKind};

/// How kickoff times are displayed.
///
/// In the config file this is written as `"local"`, `"utc"`, or a bare integer
/// hour offset from UTC (e.g. `-4`). The legacy `{ fixed-offset = N }` table
/// form is still accepted when reading.
#[derive(Debug, Clone, Default)]
pub enum TimezonePref {
    /// Convert to the system local timezone (default).
    #[default]
    Local,
    /// Show UTC.
    Utc,
    /// A fixed offset in whole hours from UTC (e.g. `-4`).
    FixedOffset(i8),
}

impl Serialize for TimezonePref {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            TimezonePref::Local => serializer.serialize_str("local"),
            TimezonePref::Utc => serializer.serialize_str("utc"),
            TimezonePref::FixedOffset(hours) => serializer.serialize_i8(*hours),
        }
    }
}

impl<'de> Deserialize<'de> for TimezonePref {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct PrefVisitor;

        fn offset_from<E: de::Error>(value: i64) -> Result<TimezonePref, E> {
            i8::try_from(value)
                .map(TimezonePref::FixedOffset)
                .map_err(|_| E::custom(format!("timezone offset {value} out of range")))
        }

        impl<'de> de::Visitor<'de> for PrefVisitor {
            type Value = TimezonePref;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("\"local\", \"utc\", or an integer hour offset from UTC")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<TimezonePref, E> {
                match value.trim().to_ascii_lowercase().as_str() {
                    "local" => Ok(TimezonePref::Local),
                    "utc" => Ok(TimezonePref::Utc),
                    other => other
                        .parse::<i8>()
                        .map(TimezonePref::FixedOffset)
                        .map_err(|_| E::custom(format!("invalid timezone {value:?}"))),
                }
            }

            fn visit_i64<E: de::Error>(self, value: i64) -> Result<TimezonePref, E> {
                offset_from(value)
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<TimezonePref, E> {
                offset_from(i64::try_from(value).unwrap_or(i64::MAX))
            }

            // Back-compat: accept the older `{ fixed-offset = N }` table form.
            fn visit_map<A: de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<TimezonePref, A::Error> {
                let mut offset: Option<i8> = None;
                while let Some(key) = map.next_key::<String>()? {
                    if key == "fixed-offset" || key == "fixed_offset" {
                        offset = Some(map.next_value()?);
                    } else {
                        map.next_value::<de::IgnoredAny>()?;
                    }
                }
                offset
                    .map(TimezonePref::FixedOffset)
                    .ok_or_else(|| de::Error::custom("missing fixed-offset"))
            }
        }

        deserializer.deserialize_any(PrefVisitor)
    }
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
    /// Show national flags on the Live card when the terminal supports graphics.
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
        let dirs = directories::ProjectDirs::from("dev", "djensenius", "worldcup26")
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
        if let Ok(key) = std::env::var("WORLDCUP26_API_FOOTBALL_KEY")
            && !key.trim().is_empty()
        {
            self.provider.api_football_key = Some(key);
        }
        if let Ok(key) = std::env::var("WORLDCUP26_FOOTBALL_DATA_KEY")
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

#[cfg(test)]
mod tests {
    use super::*;

    fn timezone_of(toml_src: &str) -> TimezonePref {
        match toml::from_str::<Config>(toml_src) {
            Ok(cfg) => cfg.ui.timezone,
            Err(err) => panic!("parse failed: {err}"),
        }
    }

    fn to_toml(cfg: &Config) -> String {
        match toml::to_string_pretty(cfg) {
            Ok(text) => text,
            Err(err) => panic!("serialise failed: {err}"),
        }
    }

    #[test]
    fn timezone_accepts_local_and_utc_strings() {
        assert!(matches!(
            timezone_of("[ui]\ntimezone = \"local\"\n"),
            TimezonePref::Local
        ));
        assert!(matches!(
            timezone_of("[ui]\ntimezone = \"utc\"\n"),
            TimezonePref::Utc
        ));
    }

    #[test]
    fn timezone_accepts_bare_integer_offset() {
        assert!(matches!(
            timezone_of("[ui]\ntimezone = -4\n"),
            TimezonePref::FixedOffset(-4)
        ));
    }

    #[test]
    fn timezone_accepts_legacy_table_form() {
        assert!(matches!(
            timezone_of("[ui.timezone]\nfixed-offset = 5\n"),
            TimezonePref::FixedOffset(5)
        ));
    }

    #[test]
    fn timezone_round_trips_as_compact_forms() {
        let mut cfg = Config::default();
        cfg.ui.timezone = TimezonePref::FixedOffset(-4);
        let text = to_toml(&cfg);
        assert!(text.contains("timezone = -4"), "got:\n{text}");
        cfg.ui.timezone = TimezonePref::Utc;
        let text = to_toml(&cfg);
        assert!(text.contains("timezone = \"utc\""), "got:\n{text}");
    }
}
