//! Colored, scalable ASCII-art national flags.
//!
//! Each flag is described once as a list of resolution-independent [`Prim`]itives
//! (bands, crosses, discs, stars, crescents, small bitmaps) in per-mille
//! coordinates, then rasterized to any pixel size on demand. Grids are drawn
//! with the upper half-block glyph `▀` (foreground = top pixel, background =
//! bottom pixel) so each text line shows two pixel rows.
//!
//! Sizes:
//! * [`Flag::render`] — a multi-line flag sized to a target cell width.
//! * [`Flag::swatch`] — a single-line mini flag for inline use in lists.
//!
//! Flags are keyed by FIFA three-letter code. Simple flags are exact; emblem
//! flags (maple leaf, eagle, taeguk, …) are recognizable approximations.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

/// Width in cells for an inline swatch.
const SWATCH_WIDTH: usize = 5;

const WHITE: Color = Color::Rgb(245, 245, 245);
const BLACK: Color = Color::Rgb(28, 28, 32);
const RED: Color = Color::Rgb(206, 32, 46);
const GREEN: Color = Color::Rgb(0, 122, 61);
const BLUE: Color = Color::Rgb(0, 49, 122);
const GOLD: Color = Color::Rgb(255, 205, 0);
const NAVY: Color = Color::Rgb(10, 40, 95);

type Grid = Vec<Vec<Color>>;

/// A scalable drawing primitive. Coordinates and sizes are in per-mille
/// (0–1000) of the region the primitive is drawn into.
enum Prim {
    Fill(Color),
    /// Vertical bands by weight, left to right.
    VBands(&'static [(u16, Color)]),
    /// Horizontal bands by weight, top to bottom.
    HBands(&'static [(u16, Color)]),
    /// `count` equal horizontal stripes alternating `a`/`b` (a first).
    Stripes {
        count: u16,
        a: Color,
        b: Color,
    },
    /// Axis-aligned rectangle.
    Rect {
        x0: u16,
        y0: u16,
        x1: u16,
        y1: u16,
        color: Color,
    },
    /// A cross: a vertical bar centred at `x` and a horizontal bar at `y`,
    /// both reaching the edges, of the given thickness.
    Cross {
        x: u16,
        y: u16,
        thick: u16,
        color: Color,
    },
    /// A centred plus that does not reach the edges (Swiss-style).
    Plus {
        thick: u16,
        len: u16,
        color: Color,
    },
    /// A diagonal saltire (X) across the region.
    Saltire {
        thick: u16,
        color: Color,
    },
    /// A single diagonal bar (`anti` = bottom-left to top-right).
    Diagonal {
        thick: u16,
        anti: bool,
        color: Color,
    },
    /// Filled circle; `r` is per-mille of the region height.
    Disc {
        cx: u16,
        cy: u16,
        r: u16,
        color: Color,
    },
    /// Half of a filled circle (`lower` keeps the bottom half).
    HalfDisc {
        cx: u16,
        cy: u16,
        r: u16,
        lower: bool,
        color: Color,
    },
    /// Filled rhombus with the given half-extents.
    Diamond {
        cx: u16,
        cy: u16,
        rx: u16,
        ry: u16,
        color: Color,
    },
    /// A crescent: a disc with a smaller `bg`-coloured disc carved out.
    Crescent {
        cx: u16,
        cy: u16,
        r: u16,
        color: Color,
        bg: Color,
    },
    /// A right triangle with its base on the hoist (left) edge.
    TriHoist {
        w: u16,
        color: Color,
    },
    /// A right triangle filling from the top edge down to the hypotenuse that
    /// runs from the top-left to the bottom-right corner.
    Triangle {
        color: Color,
    },
    /// A small five-point star centred at `(cx, cy)`, radius `r` per-mille.
    Star {
        cx: u16,
        cy: u16,
        r: u16,
        color: Color,
    },
    /// A nested sub-flag occupying the top-left `w`×`h` of the region.
    Canton {
        w: u16,
        h: u16,
        prims: &'static [Prim],
    },
    /// Single-colour pixel art scaled to fill the region; spaces transparent.
    Bitmap {
        art: &'static [&'static str],
        color: Color,
    },
}

/// A flag, renderable at any size.
pub struct Flag {
    prims: &'static [Prim],
}

impl Flag {
    /// Render as half-block [`Line`]s sized to roughly `width` cells.
    #[must_use]
    pub fn render(&self, width: usize) -> Vec<Line<'static>> {
        let rows = rows_for(width);
        to_lines(&rasterize(self.prims, width.max(2), rows))
    }

    /// Render a one-line inline swatch.
    #[must_use]
    pub fn swatch(&self) -> Line<'static> {
        to_lines(&rasterize(self.prims, SWATCH_WIDTH, 2))
            .into_iter()
            .next()
            .unwrap_or_default()
    }
}

/// The flag for a FIFA team code, if one is defined.
#[must_use]
pub fn flag(code: &str) -> Option<Flag> {
    table(&code.to_uppercase()).map(|prims| Flag { prims })
}

// --- rasterization ---------------------------------------------------------

fn rows_for(width: usize) -> usize {
    // Flags are ~3:2; pixels are square, so rows ≈ width * 2/3, made even.
    let raw = (width * 2).div_euclid(3).max(2);
    raw + (raw & 1)
}

fn to_lines(grid: &Grid) -> Vec<Line<'static>> {
    grid.chunks_exact(2)
        .map(|pair| {
            let spans = pair[0]
                .iter()
                .zip(pair[1].iter())
                .map(|(top, bottom)| Span::styled("\u{2580}", Style::new().fg(*top).bg(*bottom)))
                .collect::<Vec<_>>();
            Line::from(spans)
        })
        .collect()
}

#[derive(Clone, Copy)]
struct Reg {
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
}

impl Reg {
    fn w(self) -> usize {
        self.x1 - self.x0
    }
    fn h(self) -> usize {
        self.y1 - self.y0
    }
}

fn rasterize(prims: &[Prim], w: usize, h: usize) -> Grid {
    let mut grid = vec![vec![WHITE; w]; h];
    let full = Reg {
        x0: 0,
        y0: 0,
        x1: w,
        y1: h,
    };
    draw_all(&mut grid, full, prims);
    grid
}

fn draw_all(grid: &mut Grid, reg: Reg, prims: &[Prim]) {
    for prim in prims {
        draw(grid, reg, prim);
    }
}

/// Visit every pixel in `reg`, calling `f(local_x, local_y, cell)`.
fn each(grid: &mut Grid, reg: Reg, mut f: impl FnMut(usize, usize, &mut Color)) {
    for (y, row) in grid.iter_mut().enumerate().take(reg.y1).skip(reg.y0) {
        for (x, cell) in row.iter_mut().enumerate().take(reg.x1).skip(reg.x0) {
            f(x - reg.x0, y - reg.y0, cell);
        }
    }
}

fn pm(value: u16, span: usize) -> usize {
    usize::from(value) * span / 1000
}

#[allow(clippy::too_many_lines)]
fn draw(grid: &mut Grid, reg: Reg, prim: &Prim) {
    let (rw, rh) = (reg.w(), reg.h());
    match *prim {
        Prim::Fill(color) => each(grid, reg, |_, _, cell| *cell = color),
        Prim::VBands(specs) => {
            let total = specs.iter().map(|(w, _)| *w).sum::<u16>().max(1);
            each(grid, reg, |x, _, cell| {
                let mut acc = 0u16;
                for (weight, color) in specs {
                    let start = pm(acc.saturating_mul(1000) / total, rw);
                    acc += *weight;
                    let end = pm(acc.saturating_mul(1000) / total, rw);
                    if (start..end).contains(&x) {
                        *cell = *color;
                    }
                }
            });
        }
        Prim::HBands(specs) => {
            let total = specs.iter().map(|(w, _)| *w).sum::<u16>().max(1);
            each(grid, reg, |_, y, cell| {
                let mut acc = 0u16;
                for (weight, color) in specs {
                    let start = pm(acc.saturating_mul(1000) / total, rh);
                    acc += *weight;
                    let end = pm(acc.saturating_mul(1000) / total, rh);
                    if (start..end).contains(&y) {
                        *cell = *color;
                    }
                }
            });
        }
        Prim::Stripes { count, a, b } => {
            let n = usize::from(count.max(1));
            each(grid, reg, |_, y, cell| {
                *cell = if (y * n / rh.max(1)) % 2 == 0 { a } else { b };
            });
        }
        Prim::Rect {
            x0,
            y0,
            x1,
            y1,
            color,
        } => {
            let (px0, px1) = (pm(x0, rw), pm(x1, rw));
            let (py0, py1) = (pm(y0, rh), pm(y1, rh));
            each(grid, reg, |x, y, cell| {
                if (px0..px1).contains(&x) && (py0..py1).contains(&y) {
                    *cell = color;
                }
            });
        }
        Prim::Cross { x, y, thick, color } => {
            let cx = pm(x, rw);
            let cy = pm(y, rh);
            let tw = pm(thick, rw).max(1) / 2;
            let th = pm(thick, rh).max(1) / 2;
            each(grid, reg, |px, py, cell| {
                if px.abs_diff(cx) <= tw || py.abs_diff(cy) <= th {
                    *cell = color;
                }
            });
        }
        Prim::Plus { thick, len, color } => {
            let cx = rw / 2;
            let cy = rh / 2;
            let tw = pm(thick, rw).max(1) / 2;
            let th = pm(thick, rh).max(1) / 2;
            let lw = pm(len, rw) / 2;
            let lh = pm(len, rh) / 2;
            each(grid, reg, |px, py, cell| {
                let vbar = px.abs_diff(cx) <= tw && py.abs_diff(cy) <= lh;
                let hbar = py.abs_diff(cy) <= th && px.abs_diff(cx) <= lw;
                if vbar || hbar {
                    *cell = color;
                }
            });
        }
        Prim::Saltire { thick, color } => {
            draw_diagonal(grid, reg, thick, false, color);
            draw_diagonal(grid, reg, thick, true, color);
        }
        Prim::Diagonal { thick, anti, color } => draw_diagonal(grid, reg, thick, anti, color),
        Prim::Disc { cx, cy, r, color } => {
            let (cxp, cyp, rp) = (pm(cx, rw), pm(cy, rh), pm(r, rh));
            each(grid, reg, |x, y, cell| {
                if x.abs_diff(cxp).pow(2) + y.abs_diff(cyp).pow(2) <= rp * rp {
                    *cell = color;
                }
            });
        }
        Prim::HalfDisc {
            cx,
            cy,
            r,
            lower,
            color,
        } => {
            let (cxp, cyp, rp) = (pm(cx, rw), pm(cy, rh), pm(r, rh));
            each(grid, reg, |x, y, cell| {
                let inside = x.abs_diff(cxp).pow(2) + y.abs_diff(cyp).pow(2) <= rp * rp;
                if inside && (y >= cyp) == lower {
                    *cell = color;
                }
            });
        }
        Prim::Diamond {
            cx,
            cy,
            rx,
            ry,
            color,
        } => {
            let (cxp, cyp) = (pm(cx, rw), pm(cy, rh));
            let (rxp, ryp) = (pm(rx, rw).max(1), pm(ry, rh).max(1));
            each(grid, reg, |x, y, cell| {
                if x.abs_diff(cxp) * ryp + y.abs_diff(cyp) * rxp <= rxp * ryp {
                    *cell = color;
                }
            });
        }
        Prim::Crescent {
            cx,
            cy,
            r,
            color,
            bg,
        } => {
            let (cxp, cyp, rp) = (pm(cx, rw), pm(cy, rh), pm(r, rh));
            let carve_r = rp.saturating_sub(rp / 4).max(1);
            let carve_cx = cxp + rp / 3;
            each(grid, reg, |x, y, cell| {
                if x.abs_diff(cxp).pow(2) + y.abs_diff(cyp).pow(2) <= rp * rp {
                    *cell = color;
                }
            });
            each(grid, reg, |x, y, cell| {
                if x.abs_diff(carve_cx).pow(2) + y.abs_diff(cyp).pow(2) <= carve_r * carve_r {
                    *cell = bg;
                }
            });
        }
        Prim::TriHoist { w, color } => {
            let wp = pm(w, rw).max(1);
            let half = rh / 2;
            each(grid, reg, |x, y, cell| {
                let from_mid = y.abs_diff(half);
                if x * half <= wp * half.saturating_sub(from_mid) {
                    *cell = color;
                }
            });
        }
        Prim::Triangle { color } => {
            each(grid, reg, |x, y, cell| {
                if y * rw <= x * rh {
                    *cell = color;
                }
            });
        }
        Prim::Star { cx, cy, r, color } => {
            let rp = pm(r, rh).max(2);
            let sub = Reg {
                x0: reg.x0 + pm(cx, rw).saturating_sub(rp),
                y0: reg.y0 + pm(cy, rh).saturating_sub(rp),
                x1: (reg.x0 + pm(cx, rw) + rp).min(reg.x1),
                y1: (reg.y0 + pm(cy, rh) + rp).min(reg.y1),
            };
            draw_bitmap(grid, sub, STAR, color);
        }
        Prim::Canton { w, h, prims } => {
            let sub = Reg {
                x0: reg.x0,
                y0: reg.y0,
                x1: reg.x0 + pm(w, rw).max(1),
                y1: reg.y0 + pm(h, rh).max(1),
            };
            draw_all(grid, sub, prims);
        }
        Prim::Bitmap { art, color } => draw_bitmap(grid, reg, art, color),
    }
}

fn draw_diagonal(grid: &mut Grid, reg: Reg, thick: u16, anti: bool, color: Color) {
    let (rw, rh) = (reg.w(), reg.h());
    if rw == 0 || rh == 0 {
        return;
    }
    let t = pm(thick, rw + rh).max(1);
    let denom = rw * rw + rh * rh;
    each(grid, reg, |x, y, cell| {
        let yy = if anti {
            rh.saturating_sub(1).saturating_sub(y)
        } else {
            y
        };
        let num = (rh * x).abs_diff(rw * yy);
        if num * num <= t * t * denom {
            *cell = color;
        }
    });
}

fn draw_bitmap(grid: &mut Grid, reg: Reg, art: &[&str], color: Color) {
    let (rw, rh) = (reg.w(), reg.h());
    let bh = art.len();
    let bw = art.iter().map(|r| r.len()).max().unwrap_or(0);
    if bw == 0 || bh == 0 || rw == 0 || rh == 0 {
        return;
    }
    each(grid, reg, |x, y, cell| {
        let sx = x * bw / rw;
        let sy = y * bh / rh;
        if art.get(sy).and_then(|row| row.as_bytes().get(sx)).copied() == Some(b'#') {
            *cell = color;
        }
    });
}

const STAR: &[&str] = &["  #  ", " ### ", "#####", " ### ", "## ##"];

const LEAF: &[&str] = &[
    "           ##           ",
    "        #  ##  #        ",
    "        #  ##  #        ",
    "        ## ## ##        ",
    "      # ######## #      ",
    "       ##########       ",
    "        ########        ",
    "         ######         ",
    "        ## ## ##        ",
    "           ##           ",
    "           ##           ",
    "           ##           ",
    "           ##           ",
    "                        ",
];

const EAGLE: &[&str] = &[
    "                        ",
    "                        ",
    "          ##            ",
    "         ####  #        ",
    "          ####          ",
    "          #  #          ",
    "                        ",
];

const STARFIELD: &[&str] = &[
    "# # # # # ",
    " # # # # #",
    "# # # # # ",
    " # # # # #",
    "# # # # # ",
];

// --- flag table ------------------------------------------------------------

#[allow(clippy::too_many_lines)]
fn table(code: &str) -> Option<&'static [Prim]> {
    let prims: &'static [Prim] = match code {
        "CAN" => &[
            Prim::VBands(&[
                (1, Color::Rgb(216, 32, 39)),
                (2, WHITE),
                (1, Color::Rgb(216, 32, 39)),
            ]),
            Prim::Bitmap {
                art: LEAF,
                color: Color::Rgb(216, 32, 39),
            },
        ],
        "USA" => &[
            Prim::Stripes {
                count: 13,
                a: Color::Rgb(179, 25, 66),
                b: WHITE,
            },
            Prim::Canton {
                w: 420,
                h: 540,
                prims: &[
                    Prim::Fill(NAVY),
                    Prim::Bitmap {
                        art: STARFIELD,
                        color: WHITE,
                    },
                ],
            },
        ],
        "MEX" => &[
            Prim::VBands(&[
                (1, Color::Rgb(0, 104, 71)),
                (1, WHITE),
                (1, Color::Rgb(206, 17, 38)),
            ]),
            Prim::Bitmap {
                art: EAGLE,
                color: Color::Rgb(105, 76, 42),
            },
        ],
        "BRA" => &[
            Prim::Fill(Color::Rgb(0, 156, 59)),
            Prim::Diamond {
                cx: 500,
                cy: 500,
                rx: 460,
                ry: 440,
                color: Color::Rgb(255, 223, 0),
            },
            Prim::Disc {
                cx: 500,
                cy: 500,
                r: 270,
                color: Color::Rgb(0, 39, 118),
            },
        ],
        "ARG" => &[
            Prim::HBands(&[
                (1, Color::Rgb(108, 172, 228)),
                (1, WHITE),
                (1, Color::Rgb(108, 172, 228)),
            ]),
            Prim::Disc {
                cx: 500,
                cy: 500,
                r: 200,
                color: Color::Rgb(247, 191, 73),
            },
            Prim::Star {
                cx: 500,
                cy: 500,
                r: 130,
                color: Color::Rgb(178, 120, 20),
            },
        ],
        "FRA" => &[Prim::VBands(&[
            (1, Color::Rgb(0, 53, 128)),
            (1, WHITE),
            (1, Color::Rgb(206, 17, 38)),
        ])],
        "BEL" => &[Prim::VBands(&[
            (1, BLACK),
            (1, GOLD),
            (1, Color::Rgb(206, 17, 38)),
        ])],
        "CIV" => &[Prim::VBands(&[
            (1, Color::Rgb(247, 109, 32)),
            (1, WHITE),
            (1, GREEN),
        ])],
        "AUT" => &[Prim::HBands(&[(1, RED), (1, WHITE), (1, RED)])],
        "GER" => &[Prim::HBands(&[
            (1, BLACK),
            (1, Color::Rgb(206, 17, 38)),
            (1, GOLD),
        ])],
        "NED" => &[Prim::HBands(&[
            (1, Color::Rgb(174, 28, 40)),
            (1, WHITE),
            (1, Color::Rgb(33, 70, 139)),
        ])],
        "COL" => &[Prim::HBands(&[
            (2, GOLD),
            (1, BLUE),
            (1, Color::Rgb(206, 17, 38)),
        ])],
        "ECU" => &[
            Prim::HBands(&[(2, GOLD), (1, BLUE), (1, Color::Rgb(206, 17, 38))]),
            Prim::Disc {
                cx: 500,
                cy: 500,
                r: 110,
                color: Color::Rgb(120, 90, 40),
            },
        ],
        "PAR" => &[
            Prim::HBands(&[(1, Color::Rgb(206, 17, 38)), (1, WHITE), (1, BLUE)]),
            Prim::Disc {
                cx: 500,
                cy: 500,
                r: 110,
                color: GOLD,
            },
        ],
        "URU" => &[
            Prim::Fill(WHITE),
            Prim::Stripes {
                count: 9,
                a: WHITE,
                b: Color::Rgb(0, 56, 145),
            },
            Prim::Rect {
                x0: 0,
                y0: 0,
                x1: 400,
                y1: 560,
                color: WHITE,
            },
            Prim::Disc {
                cx: 200,
                cy: 280,
                r: 150,
                color: Color::Rgb(247, 191, 73),
            },
        ],
        "JPN" => &[
            Prim::Fill(WHITE),
            Prim::Disc {
                cx: 500,
                cy: 500,
                r: 360,
                color: Color::Rgb(188, 0, 45),
            },
        ],
        "KOR" => &[
            Prim::Fill(WHITE),
            Prim::HalfDisc {
                cx: 500,
                cy: 500,
                r: 300,
                lower: false,
                color: Color::Rgb(205, 46, 58),
            },
            Prim::HalfDisc {
                cx: 500,
                cy: 500,
                r: 300,
                lower: true,
                color: Color::Rgb(0, 71, 160),
            },
        ],
        "ENG" => &[
            Prim::Fill(WHITE),
            Prim::Cross {
                x: 500,
                y: 500,
                thick: 170,
                color: Color::Rgb(206, 17, 38),
            },
        ],
        "SCO" => &[
            Prim::Fill(Color::Rgb(0, 90, 160)),
            Prim::Saltire {
                thick: 80,
                color: WHITE,
            },
        ],
        "SUI" => &[
            Prim::Fill(Color::Rgb(213, 43, 30)),
            Prim::Plus {
                thick: 200,
                len: 600,
                color: WHITE,
            },
        ],
        "NOR" => &[
            Prim::Fill(Color::Rgb(186, 12, 47)),
            Prim::Cross {
                x: 380,
                y: 500,
                thick: 340,
                color: WHITE,
            },
            Prim::Cross {
                x: 380,
                y: 500,
                thick: 170,
                color: Color::Rgb(0, 32, 91),
            },
        ],
        "SWE" => &[
            Prim::Fill(Color::Rgb(0, 106, 167)),
            Prim::Cross {
                x: 380,
                y: 500,
                thick: 200,
                color: GOLD,
            },
        ],
        "CZE" => &[
            Prim::HBands(&[(1, WHITE), (1, Color::Rgb(215, 20, 26))]),
            Prim::TriHoist {
                w: 520,
                color: Color::Rgb(17, 69, 126),
            },
        ],
        "CRO" => &[
            Prim::HBands(&[(1, Color::Rgb(206, 17, 38)), (1, WHITE), (1, BLUE)]),
            Prim::Rect {
                x0: 380,
                y0: 300,
                x1: 620,
                y1: 700,
                color: Color::Rgb(206, 17, 38),
            },
            Prim::Star {
                cx: 500,
                cy: 500,
                r: 120,
                color: WHITE,
            },
        ],
        "ESP" => &[
            Prim::HBands(&[
                (1, Color::Rgb(198, 11, 30)),
                (2, GOLD),
                (1, Color::Rgb(198, 11, 30)),
            ]),
            Prim::Rect {
                x0: 230,
                y0: 380,
                x1: 340,
                y1: 620,
                color: Color::Rgb(173, 28, 49),
            },
        ],
        "POR" => &[
            Prim::VBands(&[(2, Color::Rgb(0, 102, 71)), (3, Color::Rgb(218, 41, 28))]),
            Prim::Disc {
                cx: 400,
                cy: 500,
                r: 160,
                color: GOLD,
            },
            Prim::Disc {
                cx: 400,
                cy: 500,
                r: 90,
                color: Color::Rgb(0, 60, 40),
            },
        ],
        "MAR" => &[
            Prim::Fill(Color::Rgb(193, 39, 45)),
            Prim::Star {
                cx: 500,
                cy: 500,
                r: 230,
                color: Color::Rgb(0, 98, 51),
            },
        ],
        "TUR" => &[
            Prim::Fill(Color::Rgb(227, 10, 23)),
            Prim::Crescent {
                cx: 420,
                cy: 500,
                r: 270,
                color: WHITE,
                bg: Color::Rgb(227, 10, 23),
            },
            Prim::Star {
                cx: 640,
                cy: 460,
                r: 120,
                color: WHITE,
            },
        ],
        "TUN" => &[
            Prim::Fill(Color::Rgb(206, 17, 38)),
            Prim::Disc {
                cx: 500,
                cy: 500,
                r: 300,
                color: WHITE,
            },
            Prim::Crescent {
                cx: 540,
                cy: 500,
                r: 200,
                color: Color::Rgb(206, 17, 38),
                bg: WHITE,
            },
            Prim::Star {
                cx: 560,
                cy: 500,
                r: 110,
                color: Color::Rgb(206, 17, 38),
            },
        ],
        "ALG" => &[
            Prim::VBands(&[(1, Color::Rgb(0, 98, 51)), (1, WHITE)]),
            Prim::Crescent {
                cx: 470,
                cy: 500,
                r: 250,
                color: Color::Rgb(210, 16, 52),
                bg: WHITE,
            },
            Prim::Star {
                cx: 640,
                cy: 500,
                r: 120,
                color: Color::Rgb(210, 16, 52),
            },
        ],
        "EGY" => &[
            Prim::HBands(&[(1, Color::Rgb(206, 17, 38)), (1, WHITE), (1, BLACK)]),
            Prim::Disc {
                cx: 500,
                cy: 500,
                r: 130,
                color: Color::Rgb(196, 160, 50),
            },
        ],
        "IRN" => &[
            Prim::HBands(&[
                (1, Color::Rgb(35, 158, 70)),
                (1, WHITE),
                (1, Color::Rgb(218, 0, 0)),
            ]),
            Prim::Disc {
                cx: 500,
                cy: 500,
                r: 120,
                color: Color::Rgb(218, 0, 0),
            },
        ],
        "IRQ" => &[
            Prim::HBands(&[(1, Color::Rgb(206, 17, 38)), (1, WHITE), (1, BLACK)]),
            Prim::Star {
                cx: 500,
                cy: 500,
                r: 130,
                color: Color::Rgb(0, 122, 61),
            },
        ],
        "KSA" => &[
            Prim::Fill(Color::Rgb(0, 106, 78)),
            Prim::Rect {
                x0: 150,
                y0: 600,
                x1: 850,
                y1: 680,
                color: WHITE,
            },
        ],
        "JOR" => &[
            Prim::HBands(&[(1, BLACK), (1, WHITE), (1, Color::Rgb(0, 122, 61))]),
            Prim::TriHoist {
                w: 430,
                color: Color::Rgb(206, 17, 38),
            },
            Prim::Star {
                cx: 150,
                cy: 500,
                r: 110,
                color: WHITE,
            },
        ],
        "QAT" => &[
            Prim::Fill(Color::Rgb(138, 21, 56)),
            Prim::Rect {
                x0: 0,
                y0: 0,
                x1: 360,
                y1: 1000,
                color: WHITE,
            },
        ],
        "UZB" => &[
            Prim::HBands(&[
                (3, Color::Rgb(30, 144, 235)),
                (1, WHITE),
                (3, Color::Rgb(0, 153, 70)),
            ]),
            Prim::Crescent {
                cx: 170,
                cy: 180,
                r: 110,
                color: WHITE,
                bg: Color::Rgb(30, 144, 235),
            },
            Prim::Star {
                cx: 330,
                cy: 180,
                r: 60,
                color: WHITE,
            },
        ],
        "SEN" => &[
            Prim::VBands(&[
                (1, Color::Rgb(0, 133, 63)),
                (1, GOLD),
                (1, Color::Rgb(225, 8, 0)),
            ]),
            Prim::Star {
                cx: 500,
                cy: 500,
                r: 180,
                color: Color::Rgb(0, 133, 63),
            },
        ],
        "GHA" => &[
            Prim::HBands(&[
                (1, Color::Rgb(206, 17, 38)),
                (1, GOLD),
                (1, Color::Rgb(0, 107, 63)),
            ]),
            Prim::Star {
                cx: 500,
                cy: 500,
                r: 150,
                color: BLACK,
            },
        ],
        "RSA" => &[
            Prim::HBands(&[
                (1, Color::Rgb(0, 122, 77)),
                (1, WHITE),
                (1, Color::Rgb(0, 122, 77)),
            ]),
            Prim::Rect {
                x0: 0,
                y0: 380,
                x1: 1000,
                y1: 620,
                color: GOLD,
            },
            Prim::TriHoist {
                w: 360,
                color: BLACK,
            },
        ],
        "COD" => &[
            Prim::Fill(Color::Rgb(0, 122, 201)),
            Prim::Diagonal {
                thick: 220,
                anti: true,
                color: Color::Rgb(247, 209, 23),
            },
            Prim::Diagonal {
                thick: 130,
                anti: true,
                color: Color::Rgb(206, 17, 38),
            },
            Prim::Star {
                cx: 150,
                cy: 230,
                r: 110,
                color: GOLD,
            },
        ],
        "CPV" => &[
            Prim::Fill(Color::Rgb(0, 56, 147)),
            Prim::Rect {
                x0: 0,
                y0: 540,
                x1: 1000,
                y1: 620,
                color: WHITE,
            },
            Prim::Rect {
                x0: 0,
                y0: 620,
                x1: 1000,
                y1: 700,
                color: Color::Rgb(206, 17, 38),
            },
            Prim::Rect {
                x0: 0,
                y0: 700,
                x1: 1000,
                y1: 780,
                color: WHITE,
            },
            Prim::Star {
                cx: 380,
                cy: 640,
                r: 120,
                color: GOLD,
            },
        ],
        "CUW" => &[
            Prim::Fill(Color::Rgb(0, 40, 104)),
            Prim::Rect {
                x0: 0,
                y0: 640,
                x1: 1000,
                y1: 800,
                color: GOLD,
            },
            Prim::Star {
                cx: 170,
                cy: 230,
                r: 80,
                color: WHITE,
            },
            Prim::Star {
                cx: 300,
                cy: 380,
                r: 110,
                color: WHITE,
            },
        ],
        "BIH" => &[
            Prim::Fill(Color::Rgb(0, 20, 137)),
            Prim::Triangle { color: GOLD },
            Prim::Star {
                cx: 140,
                cy: 150,
                r: 70,
                color: WHITE,
            },
            Prim::Star {
                cx: 350,
                cy: 410,
                r: 70,
                color: WHITE,
            },
            Prim::Star {
                cx: 560,
                cy: 670,
                r: 70,
                color: WHITE,
            },
            Prim::Star {
                cx: 770,
                cy: 900,
                r: 70,
                color: WHITE,
            },
        ],
        "PAN" => &[
            Prim::Fill(WHITE),
            Prim::Rect {
                x0: 500,
                y0: 0,
                x1: 1000,
                y1: 500,
                color: Color::Rgb(206, 17, 38),
            },
            Prim::Rect {
                x0: 0,
                y0: 500,
                x1: 500,
                y1: 1000,
                color: Color::Rgb(0, 40, 104),
            },
            Prim::Star {
                cx: 250,
                cy: 250,
                r: 130,
                color: Color::Rgb(0, 40, 104),
            },
            Prim::Star {
                cx: 750,
                cy: 750,
                r: 130,
                color: Color::Rgb(206, 17, 38),
            },
        ],
        "HAI" => &[
            Prim::HBands(&[(1, Color::Rgb(0, 32, 145)), (1, Color::Rgb(206, 17, 38))]),
            Prim::Rect {
                x0: 360,
                y0: 360,
                x1: 640,
                y1: 640,
                color: WHITE,
            },
        ],
        "AUS" => &[
            Prim::Fill(NAVY),
            Prim::Canton {
                w: 500,
                h: 500,
                prims: UNION_JACK,
            },
            Prim::Star {
                cx: 720,
                cy: 760,
                r: 120,
                color: WHITE,
            },
            Prim::Star {
                cx: 250,
                cy: 820,
                r: 90,
                color: WHITE,
            },
        ],
        "NZL" => &[
            Prim::Fill(NAVY),
            Prim::Canton {
                w: 500,
                h: 500,
                prims: UNION_JACK,
            },
            Prim::Star {
                cx: 800,
                cy: 300,
                r: 90,
                color: Color::Rgb(206, 17, 38),
            },
            Prim::Star {
                cx: 700,
                cy: 640,
                r: 90,
                color: Color::Rgb(206, 17, 38),
            },
        ],
        _ => return None,
    };
    Some(prims)
}

const UNION_JACK: &[Prim] = &[
    Prim::Fill(NAVY),
    Prim::Saltire {
        thick: 240,
        color: WHITE,
    },
    Prim::Saltire {
        thick: 110,
        color: Color::Rgb(200, 16, 46),
    },
    Prim::Cross {
        x: 500,
        y: 500,
        thick: 300,
        color: WHITE,
    },
    Prim::Cross {
        x: 500,
        y: 500,
        thick: 160,
        color: Color::Rgb(200, 16, 46),
    },
];

#[cfg(test)]
mod tests {
    use super::{flag, rows_for, table};

    const TEAMS: [&str; 48] = [
        "ALG", "ARG", "AUS", "AUT", "BEL", "BIH", "BRA", "CAN", "CIV", "COD", "COL", "CPV", "CRO",
        "CUW", "CZE", "ECU", "EGY", "ENG", "ESP", "FRA", "GER", "GHA", "HAI", "IRN", "IRQ", "JOR",
        "JPN", "KOR", "KSA", "MAR", "MEX", "NED", "NOR", "NZL", "PAN", "PAR", "POR", "QAT", "RSA",
        "SCO", "SEN", "SUI", "SWE", "TUN", "TUR", "URU", "USA", "UZB",
    ];

    #[test]
    fn every_team_has_a_flag() {
        for code in TEAMS {
            assert!(table(code).is_some(), "missing flag for {code}");
        }
    }

    #[test]
    fn renders_at_several_sizes_without_panic() {
        for code in ["CAN", "USA", "BRA", "JPN", "KOR", "FRA"] {
            let Some(f) = flag(code) else {
                panic!("no flag for {code}");
            };
            for width in [6usize, 12, 20, 24, 40] {
                let rendered = f.render(width);
                assert_eq!(rendered.len(), rows_for(width) / 2);
                assert!(!rendered.is_empty());
            }
            let _ = f.swatch();
        }
    }

    #[test]
    fn lookup_is_case_insensitive() {
        assert!(flag("can").is_some());
        assert!(flag("Can").is_some());
    }

    #[test]
    fn unknown_code_has_no_flag() {
        assert!(flag("SFW1").is_none());
        assert!(flag("ZZZ").is_none());
    }
}
