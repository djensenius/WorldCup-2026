//! Real national-flag artwork rendered via terminal graphics protocols.
//!
//! Flags are vendored as SVGs (see `assets/flags/ATTRIBUTION.md`), rasterized
//! with `resvg`, and drawn through [`ratatui_image`] using the Kitty, iTerm2, or
//! Sixel protocol when the terminal supports it. By default, the big Live-card
//! flags require one of those real graphics protocols and are omitted on
//! terminals without image support. Users can opt into a text-cell fallback with
//! `WORLDCUP26_GRAPHICS=halfblocks`. Because real graphics-protocol images
//! aren't erased by ratatui's cell diff, the event loop clears the terminal when
//! the Live card changes or is left (see `App::run`); forced halfblocks are just
//! regular terminal cells and do not need those clears. The active protocol is
//! detected once at startup (overridable with the `WORLDCUP26_GRAPHICS`
//! environment variable).

use std::collections::HashMap;

use image::{DynamicImage, RgbaImage};
use ratatui::layout::Rect;
use ratatui_image::Resize;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::Protocol;
use resvg::usvg;

/// Detect (or force) a terminal graphics picker. Returns `None` when no real
/// graphics protocol is available or graphics are disabled; in that case the
/// Live-card flags are omitted by default. `WORLDCUP26_GRAPHICS=halfblocks`
/// explicitly forces a text-cell fallback instead.
///
/// Detection is environment-based only — we never issue an interactive terminal
/// query, which can desync stdin and break key handling inside multiplexers and
/// some PTYs. [`Picker::halfblocks`] detects tmux so escapes can be wrapped in
/// tmux passthrough, then we select a graphics protocol from environment
/// variables. Set `WORLDCUP26_GRAPHICS` to `kitty`/`iterm2`/`sixel`/`halfblocks`
/// to force a protocol, or `off` to disable flags entirely.
#[must_use]
pub fn make_picker() -> Option<Picker> {
    let forced = std::env::var("WORLDCUP26_GRAPHICS")
        .ok()
        .map(|value| value.to_ascii_lowercase());
    if forced.as_deref() == Some("off") {
        return None;
    }
    // Detects tmux and marks `is_tmux` so escapes are wrapped in tmux
    // passthrough. No stdin.
    let mut picker = Picker::halfblocks();
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

    /// Whether the picker uses a real terminal graphics protocol whose output is
    /// not erased by ratatui's cell diff.
    pub fn uses_graphics_protocol(&self) -> bool {
        self.picker.protocol_type() != ProtocolType::Halfblocks
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

/// Whether a flag is available for a team code (cheap: no rasterization).
#[must_use]
pub fn has_flag(code: &str) -> bool {
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
