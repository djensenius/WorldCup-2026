//! Real national-flag artwork rendered via terminal graphics protocols.
//!
//! Flags are vendored as SVGs (see `assets/flags/ATTRIBUTION.md`), rasterized
//! with `resvg`, and drawn through [`ratatui_image`] using the Kitty, iTerm2, or
//! Sixel protocol when the terminal supports it. On terminals without graphics
//! support no flags are drawn by default (half-blocks are only used if the user
//! forces `WC26_GRAPHICS=halfblocks`). The active protocol is detected once at
//! startup (overridable with the `WC26_GRAPHICS` environment variable).
//!
//! For list screens (Matches, Standings) a tiny inline [`swatch`] is available:
//! the same SVG rasterized to a single half-block row, returned as styled spans
//! so it scrolls with the text and works on any terminal.

use std::cell::RefCell;
use std::collections::HashMap;

use image::{DynamicImage, RgbaImage};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui_image::Resize;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::Protocol;
use resvg::usvg;

/// Top/bottom pixel colours for each cell of a one-row half-block swatch.
type SwatchPixels = Vec<(Color, Color)>;

thread_local! {
    /// Per-render-thread cache of rasterized swatch pixel pairs, keyed by
    /// `(team code, width in cells)`. The TUI renders on one thread.
    static SWATCH_CACHE: RefCell<HashMap<(String, u16), Option<SwatchPixels>>> =
        RefCell::new(HashMap::new());
}

/// A tiny inline flag for list rows: the real flag rasterized to one half-block
/// row `cols` cells wide, returned as styled spans. `None` when no flag exists.
#[must_use]
pub fn swatch(code: &str, cols: u16) -> Option<Vec<Span<'static>>> {
    if cols == 0 {
        return None;
    }
    let key = (code.to_ascii_uppercase(), cols);
    let pixels = SWATCH_CACHE.with(|cache| {
        cache
            .borrow_mut()
            .entry(key.clone())
            .or_insert_with(|| rasterize_swatch(&key.0, cols))
            .clone()
    })?;
    Some(
        pixels
            .into_iter()
            .map(|(top, bottom)| Span::styled("\u{2580}", Style::new().fg(top).bg(bottom)))
            .collect(),
    )
}

fn rasterize_swatch(code: &str, cols: u16) -> Option<SwatchPixels> {
    let svg = svg(code)?;
    let image = rasterize(svg, u32::from(cols), 2)?.to_rgba8();
    let mut pairs = Vec::with_capacity(usize::from(cols));
    for x in 0..u32::from(cols) {
        let top = image.get_pixel(x, 0).0;
        let bottom = image.get_pixel(x, 1).0;
        pairs.push((
            Color::Rgb(top[0], top[1], top[2]),
            Color::Rgb(bottom[0], bottom[1], bottom[2]),
        ));
    }
    Some(pairs)
}

/// Detect (or force) a terminal graphics picker. Returns `None` when no real
/// graphics protocol is available or graphics are disabled, in which case flags
/// are simply not drawn.
///
/// Detection is environment-based only — we never issue an interactive terminal
/// query, which can desync stdin and break key handling inside multiplexers and
/// some PTYs. [`Picker::from_fontsize`] does the heavy lifting: it detects tmux
/// (enabling tmux passthrough so image escapes reach the outer terminal) and
/// guesses the outer terminal's protocol from environment variables (e.g.
/// WezTerm → iTerm2, Kitty → Kitty). We add a few terminals it misses (Ghostty,
/// Kitty-by-TERM). Set `WC26_GRAPHICS` to `kitty`/`iterm2`/`sixel`/`halfblocks`
/// to force a protocol, or `off` to disable flags entirely.
#[must_use]
pub fn make_picker() -> Option<Picker> {
    let forced = std::env::var("WC26_GRAPHICS").ok();
    if forced.as_deref() == Some("off") {
        return None;
    }
    // Builds a picker, detecting tmux + outer protocol from the environment and
    // marking `is_tmux` so escapes are wrapped in tmux passthrough. No stdin.
    let mut picker = Picker::from_fontsize(guess_font_size());
    let protocol = forced
        .as_deref()
        .and_then(parse_protocol)
        .or_else(|| non_halfblocks(picker.protocol_type()))
        .or_else(guess_extra_protocol)?;
    picker.set_protocol_type(protocol);
    Some(picker)
}

fn parse_protocol(name: &str) -> Option<ProtocolType> {
    match name.to_ascii_lowercase().as_str() {
        "kitty" => Some(ProtocolType::Kitty),
        "iterm2" => Some(ProtocolType::Iterm2),
        "sixel" => Some(ProtocolType::Sixel),
        "halfblocks" => Some(ProtocolType::Halfblocks),
        _ => None,
    }
}

/// Treat a detected half-blocks protocol as "no real graphics" so we don't draw
/// flags by default on terminals without image support.
fn non_halfblocks(protocol: ProtocolType) -> Option<ProtocolType> {
    (protocol != ProtocolType::Halfblocks).then_some(protocol)
}

/// Identify a few graphics terminals that ratatui-image's env heuristics miss
/// (notably Ghostty and Kitty-by-`TERM`). Only consulted outside tmux, where
/// `TERM`/`TERM_PROGRAM` are not masked by the multiplexer.
fn guess_extra_protocol() -> Option<ProtocolType> {
    let env = |key: &str| std::env::var(key).ok();
    let term = env("TERM").unwrap_or_default().to_ascii_lowercase();
    let program = env("TERM_PROGRAM").unwrap_or_default().to_ascii_lowercase();
    if env("KITTY_WINDOW_ID").is_some_and(|v| !v.is_empty())
        || env("KONSOLE_VERSION").is_some()
        || term.contains("kitty")
        || term.contains("ghostty")
        || program == "ghostty"
    {
        return Some(ProtocolType::Kitty);
    }
    None
}

/// A reasonable default cell size in pixels when we can't query the terminal.
/// Most monospace cells are about 1:2 (width:height).
fn guess_font_size() -> (u16, u16) {
    (8, 16)
}

/// A cache of rendered flag protocols, keyed by team code and cell size.
pub struct FlagStore {
    picker: Picker,
    cache: HashMap<(String, u16, u16), Protocol>,
}

impl FlagStore {
    /// Build a store from a detected [`Picker`].
    #[must_use]
    pub fn new(picker: Picker) -> Self {
        Self {
            picker,
            cache: HashMap::new(),
        }
    }

    /// Get (building and caching on first use) the flag protocol for `code`
    /// sized to a `cols`×`rows` cell area. Returns `None` if no flag exists for
    /// the code or it cannot be rendered.
    pub fn flag(&mut self, code: &str, cols: u16, rows: u16) -> Option<&Protocol> {
        if cols == 0 || rows == 0 {
            return None;
        }
        let key = (code.to_ascii_uppercase(), cols, rows);
        if !self.cache.contains_key(&key) {
            let svg = svg(&key.0)?;
            let (fw, fh) = self.picker.font_size();
            let width = u32::from(cols) * u32::from(fw);
            let height = u32::from(rows) * u32::from(fh);
            let image = rasterize(svg, width, height)?;
            let protocol = self
                .picker
                .new_protocol(image, Rect::new(0, 0, cols, rows), Resize::Fit(None))
                .ok()?;
            self.cache.insert(key.clone(), protocol);
        }
        self.cache.get(&key)
    }
}

fn rasterize(svg: &str, width: u32, height: u32) -> Option<DynamicImage> {
    if width == 0 || height == 0 {
        return None;
    }
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;
    let size = tree.size();
    let scale_x = (f64::from(width) / f64::from(size.width())) as f32;
    let scale_y = (f64::from(height) / f64::from(size.height())) as f32;
    let transform = resvg::tiny_skia::Transform::from_scale(scale_x, scale_y);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    let rgba = RgbaImage::from_raw(width, height, pixmap.data().to_vec())?;
    Some(DynamicImage::ImageRgba8(rgba))
}

/// Whether a flag is available for a team code.
#[cfg(test)]
fn has_flag(code: &str) -> bool {
    svg(&code.to_ascii_uppercase()).is_some()
}

/// The embedded SVG source for a FIFA team code, if vendored.
fn svg(code: &str) -> Option<&'static str> {
    let svg = match code {
        "ALG" => include_str!("../../assets/flags/ALG.svg"),
        "ARG" => include_str!("../../assets/flags/ARG.svg"),
        "AUS" => include_str!("../../assets/flags/AUS.svg"),
        "AUT" => include_str!("../../assets/flags/AUT.svg"),
        "BEL" => include_str!("../../assets/flags/BEL.svg"),
        "BIH" => include_str!("../../assets/flags/BIH.svg"),
        "BRA" => include_str!("../../assets/flags/BRA.svg"),
        "CAN" => include_str!("../../assets/flags/CAN.svg"),
        "CIV" => include_str!("../../assets/flags/CIV.svg"),
        "COD" => include_str!("../../assets/flags/COD.svg"),
        "COL" => include_str!("../../assets/flags/COL.svg"),
        "CPV" => include_str!("../../assets/flags/CPV.svg"),
        "CRO" => include_str!("../../assets/flags/CRO.svg"),
        "CUW" => include_str!("../../assets/flags/CUW.svg"),
        "CZE" => include_str!("../../assets/flags/CZE.svg"),
        "ECU" => include_str!("../../assets/flags/ECU.svg"),
        "EGY" => include_str!("../../assets/flags/EGY.svg"),
        "ENG" => include_str!("../../assets/flags/ENG.svg"),
        "ESP" => include_str!("../../assets/flags/ESP.svg"),
        "FRA" => include_str!("../../assets/flags/FRA.svg"),
        "GER" => include_str!("../../assets/flags/GER.svg"),
        "GHA" => include_str!("../../assets/flags/GHA.svg"),
        "HAI" => include_str!("../../assets/flags/HAI.svg"),
        "IRN" => include_str!("../../assets/flags/IRN.svg"),
        "IRQ" => include_str!("../../assets/flags/IRQ.svg"),
        "JOR" => include_str!("../../assets/flags/JOR.svg"),
        "JPN" => include_str!("../../assets/flags/JPN.svg"),
        "KOR" => include_str!("../../assets/flags/KOR.svg"),
        "KSA" => include_str!("../../assets/flags/KSA.svg"),
        "MAR" => include_str!("../../assets/flags/MAR.svg"),
        "MEX" => include_str!("../../assets/flags/MEX.svg"),
        "NED" => include_str!("../../assets/flags/NED.svg"),
        "NOR" => include_str!("../../assets/flags/NOR.svg"),
        "NZL" => include_str!("../../assets/flags/NZL.svg"),
        "PAN" => include_str!("../../assets/flags/PAN.svg"),
        "PAR" => include_str!("../../assets/flags/PAR.svg"),
        "POR" => include_str!("../../assets/flags/POR.svg"),
        "QAT" => include_str!("../../assets/flags/QAT.svg"),
        "RSA" => include_str!("../../assets/flags/RSA.svg"),
        "SCO" => include_str!("../../assets/flags/SCO.svg"),
        "SEN" => include_str!("../../assets/flags/SEN.svg"),
        "SUI" => include_str!("../../assets/flags/SUI.svg"),
        "SWE" => include_str!("../../assets/flags/SWE.svg"),
        "TUN" => include_str!("../../assets/flags/TUN.svg"),
        "TUR" => include_str!("../../assets/flags/TUR.svg"),
        "URU" => include_str!("../../assets/flags/URU.svg"),
        "USA" => include_str!("../../assets/flags/USA.svg"),
        "UZB" => include_str!("../../assets/flags/UZB.svg"),
        _ => return None,
    };
    Some(svg)
}

#[cfg(test)]
mod tests {
    use super::{has_flag, rasterize, svg};

    const TEAMS: [&str; 48] = [
        "ALG", "ARG", "AUS", "AUT", "BEL", "BIH", "BRA", "CAN", "CIV", "COD", "COL", "CPV", "CRO",
        "CUW", "CZE", "ECU", "EGY", "ENG", "ESP", "FRA", "GER", "GHA", "HAI", "IRN", "IRQ", "JOR",
        "JPN", "KOR", "KSA", "MAR", "MEX", "NED", "NOR", "NZL", "PAN", "PAR", "POR", "QAT", "RSA",
        "SCO", "SEN", "SUI", "SWE", "TUN", "TUR", "URU", "USA", "UZB",
    ];

    #[test]
    fn every_team_has_embedded_svg() {
        for code in TEAMS {
            assert!(svg(code).is_some(), "missing svg for {code}");
        }
    }

    #[test]
    fn has_flag_is_case_insensitive() {
        assert!(has_flag("can"));
        assert!(has_flag("CAN"));
        assert!(!has_flag("SFW1"));
    }

    #[test]
    fn embedded_svgs_rasterize() {
        for code in ["CAN", "USA", "MEX", "BRA", "KOR"] {
            let s = svg(code).unwrap_or("");
            let img = rasterize(s, 96, 64);
            assert!(img.is_some(), "failed to rasterize {code}");
        }
    }
}
