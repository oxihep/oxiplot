//! Text layout via the configured [`FontSet`] (STIX Two by default).
//!
//! Text is converted to filled glyph-outline [`Path`]s in pixel coordinates, so
//! the raster (tiny-skia) and SVG renderers draw identical glyphs — the output
//! is self-contained and font-independent on the viewer's side.

use skrifa::outline::OutlinePen;

use crate::color::Color;
use crate::draw::{DrawCommand, Path, Pt, Seg};
use crate::fonts::FontSet;

/// Font weight/slant, mirroring matplotlib's `normal`/`bold`/`italic`. The face
/// for each is taken from the active [`FontSet`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontStyle {
    /// Upright regular.
    #[default]
    Regular,
    /// Bold. Supported by the layout engine; no current artist requests it.
    #[allow(dead_code)]
    Bold,
    /// Oblique/italic. Supported by the layout engine; no current artist
    /// requests it.
    #[allow(dead_code)]
    Italic,
}

/// Horizontal anchor of a text block relative to its draw position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HAlign {
    /// Position is the left edge.
    #[default]
    Left,
    /// Position is the horizontal center.
    Center,
    /// Position is the right edge.
    Right,
}

/// Vertical anchor of a text block relative to its draw position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VAlign {
    /// Position is the text baseline.
    #[default]
    Baseline,
    /// Position is the top of the cap region.
    Top,
    /// Position is the vertical middle.
    Middle,
    /// Position is the bottom (descent).
    Bottom,
}

/// Measured extents of a string at a given pixel size.
#[derive(Debug, Clone, Copy)]
pub struct TextExtents {
    /// Total advance width in pixels.
    pub width: f32,
    /// Ascent above the baseline in pixels (positive).
    pub ascent: f32,
    /// Descent below the baseline in pixels (positive).
    pub descent: f32,
}

impl TextExtents {
    /// Total line height (ascent + descent).
    #[must_use]
    pub fn height(&self) -> f32 {
        self.ascent + self.descent
    }
}

/// Measure a single-line string rendered at `size_px` in the given style.
#[must_use]
pub fn measure(fonts: &FontSet, text: &str, size_px: f32, style: FontStyle) -> TextExtents {
    let font = fonts.face(style);
    let f = size_px / font.units_per_em;
    let mut width = 0.0;
    let mut prev = None;
    for c in text.chars() {
        let id = font.glyph_id(c);
        if let Some(p) = prev {
            width += font.kern(p, id) * f;
        }
        width += font.h_advance(id) * f;
        prev = Some(id);
    }
    TextExtents {
        width,
        ascent: font.ascent * f,
        descent: -font.descent * f,
    }
}

/// Lay out a single-line string into filled glyph-outline draw commands.
///
/// `(x, y)` is the anchor point; `halign`/`valign` say what part of the text
/// block that point refers to. `rotation_deg` rotates the block about the anchor
/// (positive = clockwise in screen space; use `-90.0` for an upward y-axis label).
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn layout(
    fonts: &FontSet,
    text: &str,
    x: f32,
    y: f32,
    size_px: f32,
    style: FontStyle,
    color: Color,
    halign: HAlign,
    valign: VAlign,
    rotation_deg: f32,
) -> Vec<DrawCommand> {
    let font = fonts.face(style);
    let f = size_px / font.units_per_em;
    let ext = measure(fonts, text, size_px, style);

    // Pen start (local x of the first glyph origin) from the horizontal anchor.
    let mut pen = match halign {
        HAlign::Left => 0.0,
        HAlign::Center => -ext.width / 2.0,
        HAlign::Right => -ext.width,
    };
    // Baseline local y from the vertical anchor (local frame is y-down).
    let baseline = match valign {
        VAlign::Baseline => 0.0,
        VAlign::Top => ext.ascent,
        VAlign::Middle => (ext.ascent - ext.descent) / 2.0,
        VAlign::Bottom => -ext.descent,
    };

    let (sin, cos) = rotation_deg.to_radians().sin_cos();
    let to_screen = |lx: f32, ly: f32| -> Pt { (x + lx * cos - ly * sin, y + lx * sin + ly * cos) };

    let mut cmds = Vec::new();
    let mut prev = None;
    for c in text.chars() {
        let id = font.glyph_id(c);
        if let Some(p) = prev {
            pen += font.kern(p, id) * f;
        }
        let pen_now = pen;
        // Map a font-unit point (y-up) to local frame (y-down) then to screen.
        let mut glyph = PathPen::new(|gx, gy| to_screen(pen_now + gx * f, baseline - gy * f));
        if font.draw(id, &mut glyph) && !glyph.path.is_empty() {
            cmds.push(DrawCommand::Path {
                path: glyph.path,
                fill: Some(color),
                stroke: None,
            });
        }
        pen += font.h_advance(id) * f;
        prev = Some(id);
    }
    cmds
}

/// Glyph outline paths for a string in a **local** frame: baseline at `y = 0`,
/// pen starting at `x = 0`. Used by the mixed text+math label layout.
pub(crate) fn glyph_paths_local(
    fonts: &FontSet,
    text: &str,
    size_px: f32,
    style: FontStyle,
) -> Vec<Path> {
    let font = fonts.face(style);
    let f = size_px / font.units_per_em;
    let mut pen = 0.0;
    let mut prev = None;
    let mut out = Vec::new();
    for c in text.chars() {
        let id = font.glyph_id(c);
        if let Some(p) = prev {
            pen += font.kern(p, id) * f;
        }
        let pen_now = pen;
        let mut glyph = PathPen::new(|gx, gy| (pen_now + gx * f, -gy * f));
        if font.draw(id, &mut glyph) && !glyph.path.is_empty() {
            out.push(glyph.path);
        }
        pen += font.h_advance(id) * f;
        prev = Some(id);
    }
    out
}

/// An [`OutlinePen`] building a [`Path`], mapping each font-unit point
/// (y up) through `map`.
pub(crate) struct PathPen<F: Fn(f32, f32) -> Pt> {
    pub(crate) path: Path,
    map: F,
}

impl<F: Fn(f32, f32) -> Pt> PathPen<F> {
    pub(crate) fn new(map: F) -> Self {
        PathPen {
            path: Path::new(),
            map,
        }
    }
}

impl<F: Fn(f32, f32) -> Pt> OutlinePen for PathPen<F> {
    fn move_to(&mut self, x: f32, y: f32) {
        let (a, b) = (self.map)(x, y);
        self.path.segs.push(Seg::Move(a, b));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (a, b) = (self.map)(x, y);
        self.path.segs.push(Seg::Line(a, b));
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let (a, b) = (self.map)(cx0, cy0);
        let (c, d) = (self.map)(x, y);
        self.path.segs.push(Seg::Quad(a, b, c, d));
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let (a, b) = (self.map)(cx0, cy0);
        let (c, d) = (self.map)(cx1, cy1);
        let (e, f) = (self.map)(x, y);
        self.path.segs.push(Seg::Cubic(a, b, c, d, e, f));
    }
    fn close(&mut self) {
        self.path.segs.push(Seg::Close);
    }
}
