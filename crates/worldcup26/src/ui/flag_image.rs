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
use std::process::Command;

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
/// variables. `override_mode`, or `WORLDCUP26_GRAPHICS` when no override is
/// provided, can be set to `auto`, `kitty`, `iterm2`, `sixel`, `halfblocks`, or
/// `off`.
#[must_use]
pub fn make_picker(override_mode: Option<&str>) -> Option<Picker> {
    let forced = override_mode
        .map(str::to_owned)
        .or_else(|| std::env::var("WORLDCUP26_GRAPHICS").ok())
        .filter(|value| !value.eq_ignore_ascii_case("auto"))
        .map(|value| value.to_ascii_lowercase());
    if forced.as_deref() == Some("off") {
        return None;
    }
    // Detects tmux and marks `is_tmux` so escapes are wrapped in tmux
    // passthrough. No stdin.
    //
    // ratatui-image's no-stdin pickers fall back to a guessed cell size of
    // 10x20px. The iTerm2/Sixel encoders emit images sized in absolute pixels,
    // so a wrong cell size makes a card occupy the wrong number of cells (it
    // anchors top-left and drifts away from the centred text labels). Feed the
    // picker the real terminal cell size when we can learn it without a stdin
    // query (from tmux, or the TIOCGWINSZ ioctl), so cards fill exactly the
    // cells we reserve for them and line up with the names above. `from_fontsize`
    // performs the same tmux/outer-protocol env detection as `halfblocks`.
    let mut picker = base_picker(detect_cell_size());
    let protocol = forced
        .as_deref()
        .and_then(parse_protocol)
        .or_else(|| non_halfblocks(picker.protocol_type()))
        .or_else(guess_extra_protocol)?;
    picker.set_protocol_type(protocol);
    Some(picker)
}

/// Build the base picker, seeding it with a known real cell size when we have
/// one (so pixel-sized graphics fill the cells we reserve) and otherwise using
/// the default guess. Both constructors run the same no-stdin tmux/outer-
/// protocol env detection.
#[allow(deprecated)]
fn base_picker(font_size: Option<(u16, u16)>) -> Picker {
    font_size.map_or_else(Picker::halfblocks, Picker::from_fontsize)
}

/// The terminal's cell size in pixels `(width, height)`, learned without a
/// stdin query: from tmux's reported client cell size when inside tmux, else
/// from the `TIOCGWINSZ` ioctl. Returns `None` if it can't be determined (e.g.
/// the multiplexer or terminal doesn't report pixel dimensions), in which case
/// the picker keeps its default guess.
fn detect_cell_size() -> Option<(u16, u16)> {
    tmux_cell_size().or_else(winsize_cell_size)
}

/// Cell size in pixels as reported by tmux (`client_cell_width`/`_height`).
/// tmux zeroes the pty's `TIOCGWINSZ` pixel fields, so this is the only way to
/// recover the real cell size inside tmux.
fn tmux_cell_size() -> Option<(u16, u16)> {
    std::env::var("TMUX").ok()?;
    let output = Command::new("tmux")
        .args([
            "display-message",
            "-p",
            "#{client_cell_width} #{client_cell_height}",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut parts = text.split_whitespace();
    let w: u16 = parts.next()?.parse().ok()?;
    let h: u16 = parts.next()?.parse().ok()?;
    (w > 0 && h > 0).then_some((w, h))
}

/// Cell size in pixels from the `TIOCGWINSZ` ioctl on stdout. This reads the
/// terminal's window size (no stdin interaction), dividing the pixel extent by
/// the cell grid. Returns `None` when the terminal doesn't report pixels, or on
/// platforms without the ioctl (Windows).
#[cfg(unix)]
fn winsize_cell_size() -> Option<(u16, u16)> {
    let ws = rustix::termios::tcgetwinsize(std::io::stdout()).ok()?;
    if ws.ws_xpixel == 0 || ws.ws_ypixel == 0 || ws.ws_col == 0 || ws.ws_row == 0 {
        return None;
    }
    Some((ws.ws_xpixel / ws.ws_col, ws.ws_ypixel / ws.ws_row))
}

#[cfg(not(unix))]
fn winsize_cell_size() -> Option<(u16, u16)> {
    None
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

/// Identify a few graphics terminals that ratatui-image's env heuristics miss,
/// including the outer terminal environment when running inside tmux.
fn guess_extra_protocol() -> Option<ProtocolType> {
    guess_from_env(|key| std::env::var(key)).or_else(guess_from_tmux)
}

fn guess_from_env(
    mut env: impl FnMut(&str) -> Result<String, std::env::VarError>,
) -> Option<ProtocolType> {
    let term = env("TERM").unwrap_or_default().to_ascii_lowercase();
    let program = env("TERM_PROGRAM").unwrap_or_default().to_ascii_lowercase();
    if env_non_empty(&mut env, "WEZTERM_EXECUTABLE")
        || env_non_empty(&mut env, "WEZTERM_PANE")
        || program == "wezterm"
    {
        return Some(ProtocolType::Iterm2);
    }
    if env_non_empty(&mut env, "KITTY_WINDOW_ID")
        || env_non_empty(&mut env, "KONSOLE_VERSION")
        || term.contains("kitty")
        || term.contains("ghostty")
        || program == "ghostty"
    {
        return Some(ProtocolType::Kitty);
    }
    None
}

fn env_non_empty(
    env: &mut impl FnMut(&str) -> Result<String, std::env::VarError>,
    key: &str,
) -> bool {
    env(key).is_ok_and(|value| !value.is_empty())
}

fn guess_from_tmux() -> Option<ProtocolType> {
    std::env::var("TMUX").ok()?;
    let tmux_env = tmux_show_environment();
    guess_from_env(|key| {
        tmux_env
            .get(key)
            .cloned()
            .ok_or(std::env::VarError::NotPresent)
    })
    .or_else(|| tmux_has_sixel().then_some(ProtocolType::Sixel))
}

fn tmux_show_environment() -> HashMap<String, String> {
    let Ok(output) = Command::new("tmux")
        .args(["show-environment", "-g"])
        .output()
    else {
        return HashMap::new();
    };
    if !output.status.success() {
        return HashMap::new();
    }
    parse_tmux_environment(&String::from_utf8_lossy(&output.stdout))
}

fn parse_tmux_environment(output: &str) -> HashMap<String, String> {
    output
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once('=')?;
            (!key.starts_with('-')).then(|| (key.to_owned(), value.to_owned()))
        })
        .collect()
}

fn tmux_has_sixel() -> bool {
    let Ok(output) = Command::new("tmux")
        .args(["display-message", "-p", "#{client_termfeatures}"])
        .output()
    else {
        return false;
    };
    output.status.success()
        && String::from_utf8_lossy(&output.stdout)
            .to_ascii_lowercase()
            .split(',')
            .any(|feature| feature.trim() == "sixel")
}

/// A cache of rendered flag protocols, keyed by team code and cell size.
pub struct FlagStore {
    picker: Picker,
    /// Composited Live-card images (home flag + score + away flag), keyed by
    /// their full layout/content fingerprint.
    cards: HashMap<String, Protocol>,
}

/// The blocky score drawn between the two flags in a composited Live card.
pub struct ScoreBlocks {
    /// Row-major filled-cell mask, `cols` wide and 5 rows tall.
    pub mask: Vec<bool>,
    /// Width of the score in cells.
    pub cols: u16,
    /// Fill colour as straight RGBA.
    pub rgba: [u8; 4],
}

/// A fully specified Live-card body to composite into a single image: two flags
/// flanking a blocky score, laid out in terminal cells.
pub struct FlagCard<'a> {
    /// Home team code (e.g. `NOR`).
    pub home: &'a str,
    /// Away team code (e.g. `FRA`).
    pub away: &'a str,
    /// Width/height of each flag, in cells.
    pub flag_cols: u16,
    /// Height of each flag, in cells.
    pub flag_rows: u16,
    /// Horizontal gap between a flag and the score, in cells.
    pub gap_cols: u16,
    /// Total card width, in cells.
    pub width_cols: u16,
    /// Total card height, in cells.
    pub height_rows: u16,
    /// The score drawn between the flags.
    pub score: ScoreBlocks,
}

impl FlagCard<'_> {
    fn cache_key(&self) -> String {
        let mask: String = self
            .score
            .mask
            .iter()
            .map(|&on| if on { '1' } else { '0' })
            .collect();
        format!(
            "{}|{}|{}x{}|g{}|{}x{}|s{}|{:?}|{mask}",
            self.home.to_ascii_uppercase(),
            self.away.to_ascii_uppercase(),
            self.flag_cols,
            self.flag_rows,
            self.gap_cols,
            self.width_cols,
            self.height_rows,
            self.score.cols,
            self.score.rgba,
        )
    }
}

impl FlagStore {
    /// Build a store from a detected [`Picker`].
    #[must_use]
    pub fn new(picker: Picker) -> Self {
        Self {
            picker,
            cards: HashMap::new(),
        }
    }

    /// Whether the picker uses a real terminal graphics protocol whose output is
    /// not erased by ratatui's cell diff.
    pub fn uses_graphics_protocol(&self) -> bool {
        self.picker.protocol_type() != ProtocolType::Halfblocks
    }

    /// Get (building and caching on first use) a composited Live-card image:
    /// the home flag, the blocky score, and the away flag drawn into a single
    /// image. Rendering the whole card as one image avoids multi-image / image+
    /// text rendering desync under terminal multiplexers (e.g. WezTerm + tmux),
    /// where only the first image on a row would otherwise survive. Returns
    /// `None` if the card has no area.
    pub fn card(&mut self, card: &FlagCard) -> Option<&Protocol> {
        if card.width_cols == 0 || card.height_rows == 0 {
            return None;
        }
        let key = card.cache_key();
        if !self.cards.contains_key(&key) {
            let (fw, fh) = self.picker.font_size();
            let image = composite_card(card, fw, fh)?;
            let protocol = self
                .picker
                .new_protocol(
                    image,
                    Rect::new(0, 0, card.width_cols, card.height_rows),
                    Resize::Fit(None),
                )
                .ok()?;
            self.cards.insert(key.clone(), protocol);
        }
        self.cards.get(&key)
    }
}

/// Compose a [`FlagCard`] into a single RGBA image at `fw`×`fh` pixels per cell.
fn composite_card(card: &FlagCard, fw: u16, fh: u16) -> Option<DynamicImage> {
    let (fw, fh) = (u32::from(fw), u32::from(fh));
    let canvas_w = u32::from(card.width_cols) * fw;
    let canvas_h = u32::from(card.height_rows) * fh;
    if canvas_w == 0 || canvas_h == 0 {
        return None;
    }
    let mut canvas = RgbaImage::new(canvas_w, canvas_h);

    let flag_box_w = u32::from(card.flag_cols) * fw;
    let flag_box_h = u32::from(card.flag_rows) * fh;
    let flag_y = i64::from(card.height_rows.saturating_sub(card.flag_rows) / 2) * i64::from(fh);

    if let Some(home) =
        svg(&card.home.to_ascii_uppercase()).and_then(|s| rasterize_fit(s, flag_box_w, flag_box_h))
    {
        image::imageops::overlay(&mut canvas, &home, 0, flag_y);
    }
    let away_x_cells = card.flag_cols + card.gap_cols + card.score.cols + card.gap_cols;
    if let Some(away) =
        svg(&card.away.to_ascii_uppercase()).and_then(|s| rasterize_fit(s, flag_box_w, flag_box_h))
    {
        image::imageops::overlay(
            &mut canvas,
            &away,
            i64::from(u32::from(away_x_cells) * fw),
            flag_y,
        );
    }

    draw_score(&mut canvas, card, fw, fh);
    Some(DynamicImage::ImageRgba8(canvas))
}

/// Paint the blocky score mask onto the card canvas.
fn draw_score(canvas: &mut RgbaImage, card: &FlagCard, fw: u32, fh: u32) {
    let cols = u32::from(card.score.cols);
    if cols == 0 {
        return;
    }
    let mask_rows = u32::try_from(card.score.mask.len()).unwrap_or(0) / cols;
    if mask_rows == 0 {
        return;
    }
    let score_left_cells = u32::from(card.flag_cols + card.gap_cols);
    let mask_height = u16::try_from(mask_rows).unwrap_or(0);
    let score_top_cells = u32::from(card.height_rows.saturating_sub(mask_height) / 2);
    let pixel = image::Rgba(card.score.rgba);
    let (canvas_w, canvas_h) = (canvas.width(), canvas.height());
    for cy in 0..mask_rows {
        for cx in 0..cols {
            if !card.score.mask[(cy * cols + cx) as usize] {
                continue;
            }
            let x0 = (score_left_cells + cx) * fw;
            let y0 = (score_top_cells + cy) * fh;
            for dy in 0..fh {
                for dx in 0..fw {
                    let (px, py) = (x0 + dx, y0 + dy);
                    if px < canvas_w && py < canvas_h {
                        canvas.put_pixel(px, py, pixel);
                    }
                }
            }
        }
    }
}

/// Rasterize an SVG to fit (preserving aspect ratio, centred and letterboxed
/// with transparency) within a `box_w`×`box_h` pixel box.
fn rasterize_fit(svg: &str, box_w: u32, box_h: u32) -> Option<RgbaImage> {
    if box_w == 0 || box_h == 0 {
        return None;
    }
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let size = tree.size();
    let scale = (f64::from(box_w) / f64::from(size.width()))
        .min(f64::from(box_h) / f64::from(size.height())) as f32;
    let iw = ((f64::from(size.width()) * f64::from(scale)).round() as u32).max(1);
    let ih = ((f64::from(size.height()) * f64::from(scale)).round() as u32).max(1);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(iw, ih)?;
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    let flag = RgbaImage::from_raw(iw, ih, pixmap.data().to_vec())?;
    let mut canvas = RgbaImage::new(box_w, box_h);
    let ox = i64::from(box_w.saturating_sub(iw) / 2);
    let oy = i64::from(box_h.saturating_sub(ih) / 2);
    image::imageops::overlay(&mut canvas, &flag, ox, oy);
    Some(canvas)
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
    use ratatui_image::picker::ProtocolType;

    use super::{
        FlagCard, ScoreBlocks, composite_card, guess_from_env, has_flag, parse_tmux_environment,
        rasterize_fit, svg,
    };
    use image::RgbaImage;

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
            let img = rasterize_fit(s, 96, 64);
            assert!(img.is_some(), "failed to rasterize {code}");
            let img = img.unwrap_or_else(|| RgbaImage::new(1, 1));
            assert_eq!((img.width(), img.height()), (96, 64));
        }
    }

    #[test]
    fn composite_card_has_expected_pixel_dimensions() {
        let mask_cols = 14u16;
        let card = FlagCard {
            home: "NOR",
            away: "FRA",
            flag_cols: 12,
            flag_rows: 8,
            gap_cols: 3,
            width_cols: 12 + 3 + mask_cols + 3 + 12,
            height_rows: 8,
            score: ScoreBlocks {
                mask: vec![true; usize::from(mask_cols) * 5],
                cols: mask_cols,
                rgba: [245, 203, 110, 255],
            },
        };
        let Some(img) = composite_card(&card, 6, 12) else {
            panic!("composite produced no image");
        };
        assert_eq!(img.width(), u32::from(card.width_cols) * 6);
        assert_eq!(img.height(), u32::from(card.height_rows) * 12);
    }

    #[test]
    fn tmux_environment_parser_ignores_removed_values() {
        let env = parse_tmux_environment(
            "TERM_PROGRAM=WezTerm\n-WEZTERM_PANE\nWEZTERM_EXECUTABLE=/Applications/WezTerm.app/Contents/MacOS/wezterm-gui\n",
        );
        assert_eq!(env.get("TERM_PROGRAM").map(String::as_str), Some("WezTerm"));
        assert_eq!(env.get("WEZTERM_PANE"), None);
        assert_eq!(
            env.get("WEZTERM_EXECUTABLE").map(String::as_str),
            Some("/Applications/WezTerm.app/Contents/MacOS/wezterm-gui")
        );
    }

    #[test]
    fn wezterm_uses_iterm2_protocol() {
        let protocol = guess_from_env(|key| match key {
            "TERM_PROGRAM" => Ok("WezTerm".to_owned()),
            _ => Err(std::env::VarError::NotPresent),
        });
        assert_eq!(protocol, Some(ProtocolType::Iterm2));
    }

    #[test]
    fn kitty_hints_use_kitty_protocol() {
        let protocol = guess_from_env(|key| match key {
            "KITTY_WINDOW_ID" => Ok("1".to_owned()),
            _ => Err(std::env::VarError::NotPresent),
        });
        assert_eq!(protocol, Some(ProtocolType::Kitty));
    }

    #[test]
    fn empty_env_hints_are_ignored() {
        let protocol = guess_from_env(|key| match key {
            "WEZTERM_EXECUTABLE" | "WEZTERM_PANE" | "KITTY_WINDOW_ID" | "KONSOLE_VERSION" => {
                Ok(String::new())
            }
            _ => Err(std::env::VarError::NotPresent),
        });
        assert_eq!(protocol, None);
    }
}
