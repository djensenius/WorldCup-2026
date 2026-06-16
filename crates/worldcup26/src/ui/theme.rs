//! Colour themes, selectable at runtime and persisted to the config file.

use ratatui::style::Color;

/// Theme names, in cycle order. The first entry is the default.
pub const NAMES: [&str; 9] = [
    "world-night",
    "world-day",
    "pitch",
    "high-contrast",
    "catppuccin-latte",
    "catppuccin-frappe",
    "catppuccin-macchiato",
    "catppuccin-mocha",
    "canada",
];

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
            "catppuccin-latte" => Self::catppuccin_latte(),
            "catppuccin-frappe" => Self::catppuccin_frappe(),
            "catppuccin-macchiato" => Self::catppuccin_macchiato(),
            "catppuccin-mocha" => Self::catppuccin_mocha(),
            "canada" => Self::canada(),
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

    /// Catppuccin Latte (light). Palette from <https://catppuccin.com/palette>.
    #[must_use]
    pub fn catppuccin_latte() -> Self {
        Self {
            accent: Color::Rgb(136, 57, 239), // Mauve
            fg: Color::Rgb(76, 79, 105),      // Text
            dim: Color::Rgb(140, 143, 161),   // Overlay1
            bg: Color::Rgb(204, 208, 218),    // Surface0
            ok: Color::Rgb(64, 160, 43),      // Green
            warn: Color::Rgb(223, 142, 29),   // Yellow
            error: Color::Rgb(210, 15, 57),   // Red
        }
    }

    /// Catppuccin Frappé (dark). Palette from <https://catppuccin.com/palette>.
    #[must_use]
    pub fn catppuccin_frappe() -> Self {
        Self {
            accent: Color::Rgb(202, 158, 230), // Mauve
            fg: Color::Rgb(198, 208, 245),     // Text
            dim: Color::Rgb(131, 139, 167),    // Overlay1
            bg: Color::Rgb(65, 69, 89),        // Surface0
            ok: Color::Rgb(166, 209, 137),     // Green
            warn: Color::Rgb(229, 200, 144),   // Yellow
            error: Color::Rgb(231, 130, 132),  // Red
        }
    }

    /// Catppuccin Macchiato (dark). Palette from <https://catppuccin.com/palette>.
    #[must_use]
    pub fn catppuccin_macchiato() -> Self {
        Self {
            accent: Color::Rgb(198, 160, 246), // Mauve
            fg: Color::Rgb(202, 211, 245),     // Text
            dim: Color::Rgb(128, 135, 162),    // Overlay1
            bg: Color::Rgb(54, 58, 79),        // Surface0
            ok: Color::Rgb(166, 218, 149),     // Green
            warn: Color::Rgb(238, 212, 159),   // Yellow
            error: Color::Rgb(237, 135, 150),  // Red
        }
    }

    /// Catppuccin Mocha (dark). Palette from <https://catppuccin.com/palette>.
    #[must_use]
    pub fn catppuccin_mocha() -> Self {
        Self {
            accent: Color::Rgb(203, 166, 247), // Mauve
            fg: Color::Rgb(205, 214, 244),     // Text
            dim: Color::Rgb(127, 132, 156),    // Overlay1
            bg: Color::Rgb(49, 50, 68),        // Surface0
            ok: Color::Rgb(166, 227, 161),     // Green
            warn: Color::Rgb(249, 226, 175),   // Yellow
            error: Color::Rgb(243, 139, 168),  // Red
        }
    }

    /// A Government of Canada palette: red and white with the signature navy
    /// chrome. Colours drawn from the GC Design System tokens
    /// (<https://design-system.canada.ca/en/styles/colour/>).
    #[must_use]
    pub fn canada() -> Self {
        Self {
            accent: Color::Rgb(223, 32, 57), // red-500 (Canada red)
            fg: Color::Rgb(255, 255, 255),   // text-light
            dim: Color::Rgb(179, 179, 179),  // grayscale-300
            bg: Color::Rgb(38, 55, 74),      // bg-primary (navy)
            ok: Color::Rgb(51, 204, 107),    // green-500
            warn: Color::Rgb(247, 206, 110), // yellow-300
            error: Color::Rgb(179, 25, 46),  // red-600 (danger)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{NAMES, Theme};

    #[test]
    fn every_named_theme_resolves() {
        for name in NAMES {
            let _ = Theme::from_name(name);
        }
    }

    #[test]
    fn theme_names_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for name in NAMES {
            assert!(seen.insert(name), "duplicate theme name: {name}");
        }
    }

    #[test]
    fn from_name_maps_each_new_theme() {
        assert_eq!(Theme::from_name("canada").accent, Theme::canada().accent);
        assert_eq!(
            Theme::from_name("catppuccin-latte").fg,
            Theme::catppuccin_latte().fg
        );
        assert_eq!(
            Theme::from_name("catppuccin-frappe").bg,
            Theme::catppuccin_frappe().bg
        );
        assert_eq!(
            Theme::from_name("catppuccin-macchiato").accent,
            Theme::catppuccin_macchiato().accent
        );
        assert_eq!(
            Theme::from_name("catppuccin-mocha").bg,
            Theme::catppuccin_mocha().bg
        );
    }

    #[test]
    fn unknown_theme_falls_back_to_default() {
        assert_eq!(
            Theme::from_name("does-not-exist").accent,
            Theme::world_night().accent
        );
    }
}
