//! Icon glyphs, with an ASCII fallback when Nerd Fonts are disabled.

use crate::ui::screens::Screen;

/// Resolves icon glyphs based on whether Nerd Fonts are enabled.
#[derive(Debug, Clone, Copy)]
pub struct Icons {
    nerd: bool,
}

impl Icons {
    /// Create an icon set.
    #[must_use]
    pub fn new(nerd_fonts: bool) -> Self {
        Self { nerd: nerd_fonts }
    }

    /// Brand glyph shown in the tab bar title.
    #[must_use]
    pub fn brand(self) -> &'static str {
        if self.nerd { "\u{f1e3} " } else { "" }
    }

    /// Glyph for a tab.
    #[must_use]
    pub fn tab(self, screen: Screen) -> &'static str {
        if !self.nerd {
            return "";
        }
        match screen {
            Screen::Matches => "\u{f073} ",
            Screen::Live => "\u{f111} ",
            Screen::Standings => "\u{f0cb} ",
            Screen::Bracket => "\u{f0e8} ",
        }
    }

    /// Live indicator glyph.
    #[must_use]
    pub fn live(self) -> &'static str {
        if self.nerd { "\u{f111}" } else { "*" }
    }

    /// A small bullet/separator.
    #[must_use]
    pub fn bullet(self) -> &'static str {
        if self.nerd { "\u{f444}" } else { "-" }
    }

    /// Favourite/star glyph.
    #[must_use]
    pub fn star(self) -> &'static str {
        if self.nerd { "\u{f005}" } else { "★" }
    }
}
