//! Colour themes, selectable at runtime and persisted to the config file.

use ratatui::style::Color;

/// Theme names, in cycle order. The first entry is the default.
pub const NAMES: [&str; 4] = ["world-night", "world-day", "pitch", "high-contrast"];

/// A resolved set of UI colours.
#[derive(Debug, Clone)]
pub struct Theme {
    /// Primary accent (selected tab, headings).
    pub accent: Color,
    /// Default foreground text.
    pub fg: Color,
    /// De-emphasised text (hints, borders).
    pub dim: Color,
    /// Background for highlighted chrome (status pill, selected row).
    pub bg: Color,
    /// Success / win / qualified indicator.
    pub ok: Color,
    /// Warning / live indicator.
    pub warn: Color,
    /// Error / loss / eliminated indicator.
    pub error: Color,
}

impl Theme {
    /// Resolve a theme by name, falling back to the default for unknown names.
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        match name {
            "world-day" => Self::world_day(),
            "pitch" => Self::pitch(),
            "high-contrast" => Self::high_contrast(),
            _ => Self::world_night(),
        }
    }

    /// The default dark palette.
    #[must_use]
    pub fn world_night() -> Self {
        Self {
            accent: Color::Rgb(116, 199, 236),
            fg: Color::Rgb(217, 224, 238),
            dim: Color::Rgb(120, 130, 150),
            bg: Color::Rgb(40, 46, 62),
            ok: Color::Rgb(140, 217, 153),
            warn: Color::Rgb(245, 203, 110),
            error: Color::Rgb(240, 124, 138),
        }
    }

    /// A light palette.
    #[must_use]
    pub fn world_day() -> Self {
        Self {
            accent: Color::Rgb(0, 95, 168),
            fg: Color::Rgb(28, 32, 40),
            dim: Color::Rgb(110, 118, 130),
            bg: Color::Rgb(220, 228, 240),
            ok: Color::Rgb(28, 138, 60),
            warn: Color::Rgb(176, 116, 0),
            error: Color::Rgb(190, 40, 60),
        }
    }

    /// A green "pitch" palette.
    #[must_use]
    pub fn pitch() -> Self {
        Self {
            accent: Color::Rgb(255, 255, 255),
            fg: Color::Rgb(228, 240, 228),
            dim: Color::Rgb(150, 180, 150),
            bg: Color::Rgb(30, 92, 50),
            ok: Color::Rgb(190, 245, 160),
            warn: Color::Rgb(250, 220, 120),
            error: Color::Rgb(250, 150, 150),
        }
    }

    /// A maximum-contrast palette.
    #[must_use]
    pub fn high_contrast() -> Self {
        Self {
            accent: Color::Rgb(255, 255, 0),
            fg: Color::Rgb(255, 255, 255),
            dim: Color::Rgb(180, 180, 180),
            bg: Color::Rgb(0, 0, 0),
            ok: Color::Rgb(0, 255, 0),
            warn: Color::Rgb(255, 200, 0),
            error: Color::Rgb(255, 60, 60),
        }
    }
}
