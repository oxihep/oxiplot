//! Artists — the drawable elements added to an [`crate::axes::Axes`]. Each artist
//! owns its data in data-coordinates and knows how to compute its data bounds
//! (for autoscaling) and how to draw itself through a [`Transform`].

use std::fmt;
use std::str::FromStr;

use crate::color::Color;
use crate::draw::{DrawCommand, DrawGroup, Rect, Stroke};
use crate::style::Style;
use crate::transform::{Bounds, Transform};

/// Lines drawn across a filled region: an uncertainty band, a shaded span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Hatch {
    /// Rising diagonals (matplotlib `"///"`): the usual statistical
    /// uncertainty band.
    Diagonal,
    /// Falling diagonals (`"\\\\\\"`).
    BackDiagonal,
    /// Both (`"xxx"`).
    Cross,
}

/// Hatch `rect` (in pixels) with lines `spacing` pixels apart. The lines
/// belong to one lattice anchored at the figure's origin, so neighbouring
/// rectangles line up.
pub(crate) fn hatch_rect(
    g: &mut DrawGroup,
    rect: Rect,
    hatch: Hatch,
    spacing: f32,
    stroke: &Stroke,
) {
    if rect.w <= 0.0 || rect.h <= 0.0 || spacing <= 0.0 {
        return;
    }
    // A diagonal at 45 degrees crosses a horizontal line every
    // `spacing × √2` pixels.
    let pitch = spacing * std::f32::consts::SQRT_2;
    let (left, right, top, bottom) = (rect.x, rect.right(), rect.y, rect.bottom());
    let rising = |g: &mut DrawGroup| {
        // Points with x + y = c: up and to the right on screen.
        let mut c = ((left + top) / pitch).ceil() * pitch;
        while c < right + bottom {
            let (x0, x1) = ((c - bottom).max(left), (c - top).min(right));
            if x1 > x0 {
                g.push(DrawCommand::Line {
                    p0: (x0, c - x0),
                    p1: (x1, c - x1),
                    stroke: stroke.clone(),
                });
            }
            c += pitch;
        }
    };
    let falling = |g: &mut DrawGroup| {
        // Points with x − y = c: down and to the right on screen.
        let mut c = ((left - bottom) / pitch).ceil() * pitch;
        while c < right - top {
            let (x0, x1) = ((c + top).max(left), (c + bottom).min(right));
            if x1 > x0 {
                g.push(DrawCommand::Line {
                    p0: (x0, x0 - c),
                    p1: (x1, x1 - c),
                    stroke: stroke.clone(),
                });
            }
            c += pitch;
        }
    };
    match hatch {
        Hatch::Diagonal => rising(g),
        Hatch::BackDiagonal => falling(g),
        Hatch::Cross => {
            rising(g);
            falling(g);
        }
    }
}

/// Marker shape for data points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Marker {
    /// No marker.
    #[default]
    None,
    /// Filled circle (matplotlib `o`).
    Circle,
    /// Filled square (`s`).
    Square,
    /// Upward triangle (`^`).
    TriangleUp,
    /// Downward triangle (`v`).
    TriangleDown,
    /// Diamond (`D`).
    Diamond,
    /// Diagonal cross (`x`).
    Cross,
    /// Upright cross (`+`).
    Plus,
    /// Five-pointed star (`*`).
    Star,
    /// Circle outline: an open marker, as ROOT's `kOpenCircle`.
    OpenCircle,
    /// Square outline.
    OpenSquare,
    /// Upward triangle outline.
    OpenTriangleUp,
    /// Downward triangle outline.
    OpenTriangleDown,
    /// Diamond outline.
    OpenDiamond,
}

impl Marker {
    /// The marker spec: matplotlib's (`"none"`, `"o"`, `"s"`, `"^"`, `"v"`,
    /// `"D"`, `"x"`, `"+"`, `"*"`), and `"open "` before it for an outline.
    #[must_use]
    pub fn spec(self) -> &'static str {
        match self {
            Marker::None => "none",
            Marker::Circle => "o",
            Marker::Square => "s",
            Marker::TriangleUp => "^",
            Marker::TriangleDown => "v",
            Marker::Diamond => "D",
            Marker::Cross => "x",
            Marker::Plus => "+",
            Marker::Star => "*",
            Marker::OpenCircle => "open o",
            Marker::OpenSquare => "open s",
            Marker::OpenTriangleUp => "open ^",
            Marker::OpenTriangleDown => "open v",
            Marker::OpenDiamond => "open D",
        }
    }

    /// Whether the marker is an outline, and the solid shape it outlines.
    fn outline(self) -> (bool, Marker) {
        match self {
            Marker::OpenCircle => (true, Marker::Circle),
            Marker::OpenSquare => (true, Marker::Square),
            Marker::OpenTriangleUp => (true, Marker::TriangleUp),
            Marker::OpenTriangleDown => (true, Marker::TriangleDown),
            Marker::OpenDiamond => (true, Marker::Diamond),
            solid => (false, solid),
        }
    }
}

impl fmt::Display for Marker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.spec())
    }
}

/// Parse a matplotlib marker spec, e.g. `"o"`, `"s"`, `"^"`, or `"none"`.
impl FromStr for Marker {
    type Err = ParseMarkerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "" | "none" | "None" => Ok(Marker::None),
            "o" => Ok(Marker::Circle),
            "s" => Ok(Marker::Square),
            "^" => Ok(Marker::TriangleUp),
            "v" => Ok(Marker::TriangleDown),
            "D" => Ok(Marker::Diamond),
            "x" => Ok(Marker::Cross),
            "+" => Ok(Marker::Plus),
            "*" => Ok(Marker::Star),
            "open o" => Ok(Marker::OpenCircle),
            "open s" => Ok(Marker::OpenSquare),
            "open ^" => Ok(Marker::OpenTriangleUp),
            "open v" => Ok(Marker::OpenTriangleDown),
            "open D" => Ok(Marker::OpenDiamond),
            _ => Err(ParseMarkerError(s.to_string())),
        }
    }
}

/// The error returned when a string cannot be parsed as a [`Marker`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseMarkerError(String);

impl fmt::Display for ParseMarkerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown marker `{}`", self.0)
    }
}

impl std::error::Error for ParseMarkerError {}

/// A legend handle describing how to draw a sample swatch.
#[derive(Debug, Clone)]
pub(crate) enum LegendHandle {
    /// A line/marker sample.
    Line {
        color: Color,
        width_pt: f32,
        dash: Option<Vec<f32>>,
        marker: Marker,
        marker_size_pt: f32,
    },
    /// A filled/outlined patch sample (histograms).
    Patch {
        face: Option<Color>,
        edge: Option<Color>,
        width_pt: f32,
        /// Lines across the sample, and their colour.
        hatch: Option<(Hatch, Color)>,
    },
}

/// One legend entry.
#[derive(Debug, Clone)]
pub(crate) struct LegendItem {
    pub label: String,
    pub handle: LegendHandle,
}

/// Draw a marker centered at `c` with pixel diameter `size_px`. An open
/// marker is the outline of its solid shape in the fill colour; a cross or a
/// plus is two strokes in it.
pub(crate) fn draw_marker(
    g: &mut DrawGroup,
    shape: Marker,
    c: (f32, f32),
    size_px: f32,
    fill: Option<Color>,
    edge: Option<Stroke>,
) {
    let r = size_px / 2.0;
    let (open, shape) = shape.outline();
    // The outline, and the strokes of a cross, are a sixth of the marker wide.
    let pen = |color: Color| Stroke::new(color, (size_px / 6.0).max(0.6));
    let (fill, edge) = match (open, fill) {
        (true, Some(color)) => (None, Some(pen(color))),
        _ => (fill, edge),
    };
    let polygon = |pts: Vec<(f32, f32)>| DrawCommand::Polygon {
        pts,
        fill,
        stroke: edge.clone(),
    };
    match shape {
        Marker::Circle => g.push(DrawCommand::Circle {
            c,
            r,
            fill,
            stroke: edge,
        }),
        Marker::Square => g.push(DrawCommand::Rect {
            rect: Rect::new(c.0 - r, c.1 - r, 2.0 * r, 2.0 * r),
            fill,
            stroke: edge,
        }),
        Marker::TriangleUp => g.push(polygon(vec![
            (c.0, c.1 - r),
            (c.0 + r * 0.866, c.1 + r * 0.5),
            (c.0 - r * 0.866, c.1 + r * 0.5),
        ])),
        Marker::TriangleDown => g.push(polygon(vec![
            (c.0, c.1 + r),
            (c.0 + r * 0.866, c.1 - r * 0.5),
            (c.0 - r * 0.866, c.1 - r * 0.5),
        ])),
        Marker::Diamond => g.push(polygon(vec![
            (c.0, c.1 - r * 1.2),
            (c.0 + r * 0.9, c.1),
            (c.0, c.1 + r * 1.2),
            (c.0 - r * 0.9, c.1),
        ])),
        Marker::Star => {
            // Ten vertices, alternating the outer and the inner radius.
            let pts = (0..10_u8)
                .map(|i| {
                    let radius = if i % 2 == 0 { r * 1.25 } else { r * 0.5 };
                    let angle = std::f32::consts::PI * (f32::from(i) / 5.0 - 0.5);
                    (c.0 + radius * angle.cos(), c.1 + radius * angle.sin())
                })
                .collect();
            g.push(polygon(pts));
        }
        Marker::Cross | Marker::Plus => {
            let Some(color) = fill.or_else(|| edge.as_ref().map(|e| e.color)) else {
                return;
            };
            // A plus reaches the marker's radius; a cross the same length
            // along the diagonals.
            let d = if shape == Marker::Plus { r } else { r * 0.707 };
            let arms = if shape == Marker::Plus {
                [((-d, 0.0), (d, 0.0)), ((0.0, -d), (0.0, d))]
            } else {
                [((-d, -d), (d, d)), ((-d, d), (d, -d))]
            };
            for (a, b) in arms {
                g.push(DrawCommand::Line {
                    p0: (c.0 + a.0, c.1 + a.1),
                    p1: (c.0 + b.0, c.1 + b.1),
                    stroke: pen(color),
                });
            }
        }
        // `outline` maps every open marker to its solid shape, and no marker
        // draws nothing.
        Marker::None
        | Marker::OpenCircle
        | Marker::OpenSquare
        | Marker::OpenTriangleUp
        | Marker::OpenTriangleDown
        | Marker::OpenDiamond => {}
    }
}

fn data_bounds(xs: &[f64], ys: &[f64]) -> Option<Bounds> {
    let mut b: Option<Bounds> = None;
    for (&x, &y) in xs.iter().zip(ys) {
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        b = Some(match b {
            None => Bounds::new(x, x, y, y),
            Some(p) => p.union(Bounds::new(x, x, y, y)),
        });
    }
    b
}

/// A connected line and/or markers through `(x, y)` points (matplotlib `plot`).
#[derive(Debug, Clone)]
pub(crate) struct LineArtist {
    pub xs: Vec<f64>,
    pub ys: Vec<f64>,
    pub color: Color,
    pub width_pt: f32,
    pub dash: Option<Vec<f32>>,
    pub marker: Marker,
    pub marker_size_pt: f32,
    pub label: Option<String>,
}

impl LineArtist {
    fn bounds(&self) -> Option<Bounds> {
        data_bounds(&self.xs, &self.ys)
    }

    fn draw(&self, t: &Transform, style: &Style, g: &mut DrawGroup) {
        let pts: Vec<(f32, f32)> = self
            .xs
            .iter()
            .zip(&self.ys)
            .filter(|(x, y)| x.is_finite() && y.is_finite())
            .map(|(&x, &y)| t.pt((x, y)))
            .collect();
        if self.width_pt > 0.0 && pts.len() >= 2 {
            let mut stroke = Stroke::new(self.color, style.px(self.width_pt));
            stroke.dash = self
                .dash
                .as_ref()
                .map(|d| d.iter().map(|v| style.px(*v)).collect());
            g.push(DrawCommand::Polyline {
                pts: pts.clone(),
                stroke,
            });
        }
        if self.marker != Marker::None {
            let size = style.px(self.marker_size_pt);
            for &p in &pts {
                draw_marker(g, self.marker, p, size, Some(self.color), None);
            }
        }
    }

    fn legend(&self) -> Option<LegendItem> {
        self.label.clone().map(|label| LegendItem {
            label,
            handle: LegendHandle::Line {
                color: self.color,
                width_pt: self.width_pt,
                dash: self.dash.clone(),
                marker: self.marker,
                marker_size_pt: self.marker_size_pt,
            },
        })
    }
}

/// How an mplhep-style histogram is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum HistType {
    /// Staircase outline closed to the baseline (mplhep default).
    #[default]
    Step,
    /// Filled staircase down to the baseline.
    Fill,
    /// Markers at bin centers with vertical error bars (data-point look).
    Errorbar,
    /// A shaded band spanning `y ± yerr` (uncertainty band).
    Band,
}

impl HistType {
    /// The mplhep histogram-type name (`"step"`, `"fill"`, `"errorbar"`, `"band"`).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            HistType::Step => "step",
            HistType::Fill => "fill",
            HistType::Errorbar => "errorbar",
            HistType::Band => "band",
        }
    }
}

impl fmt::Display for HistType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Parse an mplhep histogram-type name, e.g. `"step"` or `"fill"`.
impl FromStr for HistType {
    type Err = ParseHistTypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "step" => Ok(HistType::Step),
            "fill" => Ok(HistType::Fill),
            "errorbar" => Ok(HistType::Errorbar),
            "band" => Ok(HistType::Band),
            _ => Err(ParseHistTypeError(s.to_string())),
        }
    }
}

/// The error returned when a string cannot be parsed as a [`HistType`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseHistTypeError(String);

impl fmt::Display for ParseHistTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown histtype `{}`", self.0)
    }
}

impl std::error::Error for ParseHistTypeError {}

/// A finite value, or 0 for NaN/±∞ (a bin that cannot be drawn shows empty
/// rather than poisoning the whole outline).
pub(crate) fn finite_or_zero(v: f64) -> f64 {
    if v.is_finite() { v } else { 0.0 }
}

/// Build the staircase vertices for `edges`/`values` (data coords). When
/// `baseline` is set, the first and last points drop to `y = 0`. Non-finite
/// values are drawn at 0.
fn staircase(edges: &[f64], values: &[f64], baseline: bool) -> Vec<(f64, f64)> {
    let values: Vec<f64> = values.iter().copied().map(finite_or_zero).collect();
    let n = values.len();
    let mut pts = Vec::with_capacity(2 * n + 2);
    if n == 0 || edges.len() < n + 1 {
        return pts;
    }
    if baseline {
        pts.push((edges[0], 0.0));
    }
    pts.push((edges[0], values[0]));
    for i in 1..n {
        pts.push((edges[i], values[i - 1]));
        pts.push((edges[i], values[i]));
    }
    pts.push((edges[n], values[n - 1]));
    if baseline {
        pts.push((edges[n], 0.0));
    }
    pts
}

/// An mplhep-style histogram (a `Hist1D` reduced to edges + values + optional yerr).
#[derive(Debug, Clone)]
pub(crate) struct StepArtist {
    pub edges: Vec<f64>,
    pub values: Vec<f64>,
    /// The `(low, high)` error of each bin: how far its bar reaches below
    /// and above the value.
    pub errs: Option<Vec<(f64, f64)>>,
    /// Whether an errorbar histogram draws a bin that holds nothing.
    pub empty_bins: bool,
    /// Lines drawn across a band or a fill, on top of its colour.
    pub hatch: Option<Hatch>,
    pub histtype: HistType,
    pub color: Color,
    pub fill_color: Option<Color>,
    pub width_pt: f32,
    pub marker: Marker,
    pub marker_size_pt: f32,
    pub label: Option<String>,
}

impl StepArtist {
    fn n(&self) -> usize {
        self.values.len()
    }

    fn bounds(&self) -> Option<Bounds> {
        let n = self.n();
        if n == 0 || self.edges.len() < n + 1 {
            return None;
        }
        // The bars stand on y = 0; each bin adds its value, error bar ends,
        // and (for a log axis) its edges and value as positive candidates.
        let mut b = Bounds::new(self.edges[0], self.edges[n], 0.0, 0.0);
        for i in 0..n {
            let v = self.values[i];
            if !v.is_finite() {
                continue;
            }
            let (el, eh) = self.err(i);
            let (lo, hi) = (self.edges[i], self.edges[i + 1]);
            b = b
                .union(Bounds::new(lo, hi, v - el, v + eh))
                .union(Bounds::new(lo, hi, v, v));
        }
        Some(b)
    }

    /// The `(low, high)` error of bin `i`: 0 where there is none or it is
    /// not finite.
    fn err(&self, i: usize) -> (f64, f64) {
        self.errs
            .as_ref()
            .and_then(|e| e.get(i))
            .map_or((0.0, 0.0), |&(lo, hi)| {
                (finite_or_zero(lo), finite_or_zero(hi))
            })
    }

    /// Whether an errorbar histogram draws bin `i`: an empty bin is left
    /// out unless asked for, as ROOT leaves it out of a data histogram.
    fn shown(&self, i: usize) -> bool {
        self.values[i].is_finite() && (self.empty_bins || self.values[i] != 0.0)
    }

    fn map(&self, t: &Transform, pts: &[(f64, f64)]) -> Vec<(f32, f32)> {
        pts.iter().map(|&p| t.pt(p)).collect()
    }

    fn draw_errorbars(&self, t: &Transform, style: &Style, g: &mut DrawGroup) {
        if self.errs.is_none() {
            return;
        }
        let stroke = Stroke::new(self.color, style.px(1.0));
        let points = self.histtype == HistType::Errorbar;
        for i in 0..self.n() {
            let (v, (el, eh)) = (self.values[i], self.err(i));
            if !v.is_finite() || (el == 0.0 && eh == 0.0) || (points && !self.shown(i)) {
                continue;
            }
            let c = 0.5 * (self.edges[i] + self.edges[i + 1]);
            let cx = t.x(c);
            let ytop = t.y(v + eh);
            let ybot = t.y(v - el);
            g.push(DrawCommand::Line {
                p0: (cx, ybot),
                p1: (cx, ytop),
                stroke: stroke.clone(),
            });
        }
    }

    /// Hatch each bin between `lower` and `upper`. The lines are laid on one
    /// lattice over the whole figure, so they run on from bin to bin.
    fn draw_hatch(
        &self,
        t: &Transform,
        style: &Style,
        g: &mut DrawGroup,
        lower: &[f64],
        upper: &[f64],
    ) {
        let Some(hatch) = self.hatch else {
            return;
        };
        let stroke = Stroke::new(self.color, style.px(0.6));
        for i in 0..self.n() {
            let (lo, hi) = (lower[i], upper[i]);
            if !lo.is_finite() || !hi.is_finite() || lo == hi {
                continue;
            }
            let (x0, x1) = (t.x(self.edges[i]), t.x(self.edges[i + 1]));
            let (y0, y1) = (t.y(hi), t.y(lo));
            let rect = Rect::new(x0.min(x1), y0.min(y1), (x1 - x0).abs(), (y1 - y0).abs());
            hatch_rect(g, rect, hatch, style.px(4.5), &stroke);
        }
    }

    fn draw(&self, t: &Transform, style: &Style, g: &mut DrawGroup) {
        let n = self.n();
        if n == 0 {
            return;
        }
        match self.histtype {
            HistType::Step => {
                let pts = self.map(t, &staircase(&self.edges, &self.values, true));
                g.push(DrawCommand::Polyline {
                    pts,
                    stroke: Stroke::new(self.color, style.px(self.width_pt)),
                });
                self.draw_errorbars(t, style, g);
            }
            HistType::Fill => {
                let pts = self.map(t, &staircase(&self.edges, &self.values, true));
                g.push(DrawCommand::Polygon {
                    pts,
                    fill: Some(self.fill_color.unwrap_or(self.color)),
                    stroke: Some(Stroke::line(self.color, style.px(self.width_pt))),
                });
                let zeros = vec![0.0; n];
                self.draw_hatch(t, style, g, &zeros, &self.values);
                self.draw_errorbars(t, style, g);
            }
            HistType::Errorbar => {
                let size = style.px(self.marker_size_pt);
                for i in 0..n {
                    if !self.shown(i) {
                        continue;
                    }
                    let c = 0.5 * (self.edges[i] + self.edges[i + 1]);
                    let p = t.pt((c, self.values[i]));
                    draw_marker(g, self.marker, p, size, Some(self.color), None);
                }
                self.draw_errorbars(t, style, g);
            }
            HistType::Band => {
                let upper: Vec<f64> = (0..n).map(|i| self.values[i] + self.err(i).1).collect();
                let lower: Vec<f64> = (0..n).map(|i| self.values[i] - self.err(i).0).collect();
                let mut up = staircase(&self.edges, &upper, false);
                let mut lo = staircase(&self.edges, &lower, false);
                lo.reverse();
                up.append(&mut lo);
                // A hatched band is lines on a clear ground unless a fill
                // colour is asked for; a plain one is a translucent fill.
                let fill = match (self.hatch, self.fill_color) {
                    (_, Some(color)) => Some(color),
                    (Some(_), None) => None,
                    (None, None) => Some(self.color.with_alpha(0.3)),
                };
                if fill.is_some() {
                    g.push(DrawCommand::Polygon {
                        pts: self.map(t, &up),
                        fill,
                        stroke: None,
                    });
                }
                self.draw_hatch(t, style, g, &lower, &upper);
            }
        }
    }

    fn legend(&self) -> Option<LegendItem> {
        self.label.clone().map(|label| {
            let handle = match self.histtype {
                HistType::Errorbar => LegendHandle::Line {
                    color: self.color,
                    width_pt: 0.0,
                    dash: None,
                    marker: self.marker,
                    marker_size_pt: self.marker_size_pt,
                },
                // mplhep represents step/fill histograms in the legend with a
                // short horizontal line in the histogram's color (not a matplotlib
                // bar patch). The shaded uncertainty band stays a filled swatch.
                HistType::Step | HistType::Fill => LegendHandle::Line {
                    color: self.color,
                    width_pt: self.width_pt,
                    dash: None,
                    marker: Marker::None,
                    marker_size_pt: 0.0,
                },
                HistType::Band => LegendHandle::Patch {
                    face: match (self.hatch, self.fill_color) {
                        (_, Some(color)) => Some(color),
                        (Some(_), None) => None,
                        (None, None) => Some(self.color.with_alpha(0.3)),
                    },
                    edge: None,
                    width_pt: self.width_pt,
                    hatch: self.hatch.map(|h| (h, self.color)),
                },
            };
            LegendItem { label, handle }
        })
    }
}

/// Data points with x/y error bars and markers (matplotlib `errorbar`). Errors
/// are stored as `(low, high)` magnitude pairs (symmetric → equal halves).
#[derive(Debug, Clone)]
pub(crate) struct ErrorbarArtist {
    pub xs: Vec<f64>,
    pub ys: Vec<f64>,
    pub xerr: Option<(Vec<f64>, Vec<f64>)>,
    pub yerr: Option<(Vec<f64>, Vec<f64>)>,
    pub color: Color,
    pub marker: Marker,
    pub marker_size_pt: f32,
    pub elinewidth_pt: f32,
    pub capsize_pt: f32,
    pub line_width_pt: Option<f32>,
    pub label: Option<String>,
}

impl ErrorbarArtist {
    fn n(&self) -> usize {
        self.xs.len().min(self.ys.len())
    }

    /// The `(low, high)` error of point `i`; a missing or non-finite one is 0.
    fn err_at(pair: &Option<(Vec<f64>, Vec<f64>)>, i: usize) -> (f64, f64) {
        match pair {
            Some((lo, hi)) => (
                lo.get(i).copied().map_or(0.0, finite_or_zero),
                hi.get(i).copied().map_or(0.0, finite_or_zero),
            ),
            None => (0.0, 0.0),
        }
    }

    /// Whether point `i` can be drawn (both coordinates finite).
    fn drawable(&self, i: usize) -> bool {
        self.xs[i].is_finite() && self.ys[i].is_finite()
    }

    fn bounds(&self) -> Option<Bounds> {
        let n = self.n();
        let mut b: Option<Bounds> = None;
        for i in 0..n {
            let (x, y) = (self.xs[i], self.ys[i]);
            if !x.is_finite() || !y.is_finite() {
                continue;
            }
            let (xl, xh) = Self::err_at(&self.xerr, i);
            let (yl, yh) = Self::err_at(&self.yerr, i);
            // The point itself too: on a log axis its lower error bar may
            // reach ≤ 0 while the point does not.
            let bb = Bounds::new(x - xl, x + xh, y - yl, y + yh).union(Bounds::new(x, x, y, y));
            b = Some(match b {
                None => bb,
                Some(p) => p.union(bb),
            });
        }
        b
    }

    fn draw(&self, t: &Transform, style: &Style, g: &mut DrawGroup) {
        let n = self.n();
        let estroke = Stroke::new(self.color, style.px(self.elinewidth_pt));
        let cap = style.px(self.capsize_pt);

        if let Some(wpt) = self.line_width_pt {
            let pts: Vec<(f32, f32)> = (0..n)
                .filter(|&i| self.drawable(i))
                .map(|i| t.pt((self.xs[i], self.ys[i])))
                .collect();
            if pts.len() >= 2 {
                g.push(DrawCommand::Polyline {
                    pts,
                    stroke: Stroke::new(self.color, style.px(wpt)),
                });
            }
        }

        for i in 0..n {
            if !self.drawable(i) {
                continue;
            }
            let (px, py) = t.pt((self.xs[i], self.ys[i]));
            let (yl, yh) = Self::err_at(&self.yerr, i);
            if yl != 0.0 || yh != 0.0 {
                let ytop = t.y(self.ys[i] + yh);
                let ybot = t.y(self.ys[i] - yl);
                g.push(DrawCommand::Line {
                    p0: (px, ybot),
                    p1: (px, ytop),
                    stroke: estroke.clone(),
                });
                if cap > 0.0 {
                    for cy in [ytop, ybot] {
                        g.push(DrawCommand::Line {
                            p0: (px - cap, cy),
                            p1: (px + cap, cy),
                            stroke: estroke.clone(),
                        });
                    }
                }
            }
            let (xl, xh) = Self::err_at(&self.xerr, i);
            if xl != 0.0 || xh != 0.0 {
                let xleft = t.x(self.xs[i] - xl);
                let xright = t.x(self.xs[i] + xh);
                g.push(DrawCommand::Line {
                    p0: (xleft, py),
                    p1: (xright, py),
                    stroke: estroke.clone(),
                });
                if cap > 0.0 {
                    for cx in [xleft, xright] {
                        g.push(DrawCommand::Line {
                            p0: (cx, py - cap),
                            p1: (cx, py + cap),
                            stroke: estroke.clone(),
                        });
                    }
                }
            }
        }

        // Markers on top.
        if self.marker != Marker::None {
            let size = style.px(self.marker_size_pt);
            for i in (0..n).filter(|&i| self.drawable(i)) {
                let p = t.pt((self.xs[i], self.ys[i]));
                draw_marker(g, self.marker, p, size, Some(self.color), None);
            }
        }
    }

    fn legend(&self) -> Option<LegendItem> {
        self.label.clone().map(|label| LegendItem {
            label,
            handle: LegendHandle::Line {
                color: self.color,
                width_pt: self.line_width_pt.unwrap_or(0.0),
                dash: None,
                marker: self.marker,
                marker_size_pt: self.marker_size_pt,
            },
        })
    }
}

/// Bins from which a heatmap is drawn as one image rather than a rectangle
/// per bin (with the `png` feature, which encodes the image for SVG).
#[cfg(feature = "png")]
pub(crate) const MESH_IMAGE_MIN_CELLS: usize = 64 * 64;

/// A 2-D color mesh of a `Hist2D` (matplotlib `pcolormesh`). `values[ix][iy]` are
/// in-range bin contents; cell `(ix, iy)` spans `xedges[ix..ix+1]` × `yedges`.
#[derive(Debug, Clone)]
pub(crate) struct MeshArtist {
    pub xedges: Vec<f64>,
    pub yedges: Vec<f64>,
    pub values: Vec<Vec<f64>>,
    pub cmap: crate::cmap::Colormap,
    pub vmin: f64,
    pub vmax: f64,
    pub norm: crate::norm::Norm,
}

impl MeshArtist {
    fn bounds(&self) -> Option<Bounds> {
        let nx = self.values.len();
        if nx == 0 {
            return None;
        }
        let ny = self.values[0].len();
        if self.xedges.len() < nx + 1 || self.yedges.len() < ny + 1 {
            return None;
        }
        Some(Bounds::new(
            self.xedges[0],
            self.xedges[nx],
            self.yedges[0],
            self.yedges[ny],
        ))
    }

    /// The cell colour of a bin value, or `None` to leave it undrawn: empty
    /// (exactly 0) and non-finite bins, and values the norm cannot place.
    fn cell_color(&self, v: f64) -> Option<crate::color::Color> {
        if v == 0.0 || !v.is_finite() {
            return None;
        }
        let frac = self.norm.normalize(v, self.vmin, self.vmax)?;
        Some(self.cmap.sample(frac))
    }

    /// Pixel positions of `edges` when they are evenly spaced on screen (a
    /// regular binning on a linear axis), else `None`.
    #[cfg(feature = "png")]
    fn even_pixels(edges: &[f64], to_px: impl Fn(f64) -> f32) -> Option<(f32, f32)> {
        let (first, last) = (to_px(*edges.first()?), to_px(*edges.last()?));
        let n = edges.len() - 1;
        let step = (last - first) / n as f32;
        let even = edges.iter().enumerate().all(|(i, &e)| {
            (to_px(e) - (first + step * i as f32)).abs() <= 0.05 * step.abs().max(1e-3)
        });
        (even && n > 0 && step != 0.0).then_some((first, last))
    }

    /// Draw the mesh as one image, one pixel per bin, when it is large and
    /// evenly spaced on screen: a 500 × 500 map is then one picture instead of
    /// 250 000 rectangles in an SVG or PDF. Returns whether it did.
    #[cfg(feature = "png")]
    fn draw_as_image(&self, t: &Transform, g: &mut DrawGroup) -> bool {
        let nx = self.values.len();
        let ny = self.values.first().map_or(0, Vec::len);
        if nx * ny < MESH_IMAGE_MIN_CELLS
            || self.values.iter().any(|col| col.len() != ny)
            || self.xedges.len() != nx + 1
            || self.yedges.len() != ny + 1
        {
            return false;
        }
        let (Some((x0, x1)), Some((y0, y1))) = (
            Self::even_pixels(&self.xedges, |x| t.x(x)),
            Self::even_pixels(&self.yedges, |y| t.y(y)),
        ) else {
            return false;
        };
        // Image rows run top to bottom, columns left to right; an inverted
        // axis flips the order bins are read in.
        let (flip_x, flip_y) = (x1 < x0, y1 > y0);
        let mut rgba = vec![0u8; nx * ny * 4];
        for row in 0..ny {
            let iy = if flip_y { row } else { ny - 1 - row };
            for col in 0..nx {
                let ix = if flip_x { nx - 1 - col } else { col };
                if let Some(c) = self.cell_color(self.values[ix][iy]) {
                    let k = (row * nx + col) * 4;
                    rgba[k..k + 4].copy_from_slice(&[c.r, c.g, c.b, c.a]);
                }
            }
        }
        g.push(DrawCommand::Image {
            rect: Rect::new(x0.min(x1), y0.min(y1), (x1 - x0).abs(), (y1 - y0).abs()),
            width: u32::try_from(nx).unwrap_or(u32::MAX),
            height: u32::try_from(ny).unwrap_or(u32::MAX),
            rgba: std::sync::Arc::new(rgba),
        });
        true
    }

    fn draw(&self, t: &Transform, _style: &Style, g: &mut DrawGroup) {
        let nx = self.values.len();
        if nx == 0 {
            return;
        }
        #[cfg(feature = "png")]
        if self.draw_as_image(t, g) {
            return;
        }
        for ix in 0..nx {
            if self.xedges.len() < ix + 2 {
                break;
            }
            let x0 = t.x(self.xedges[ix]);
            let x1 = t.x(self.xedges[ix + 1]);
            let (rx, rw) = (x0.min(x1), (x1 - x0).abs());
            let col = &self.values[ix];
            // Offset indexing into yedges (iy, iy+1) keeps the index explicit.
            #[allow(clippy::needless_range_loop)]
            for iy in 0..col.len() {
                if self.yedges.len() < iy + 2 {
                    break;
                }
                // Empty bins (no data) are left undrawn so the page background
                // shows through, instead of being painted the colormap's value-0
                // color (ROOT COLZ / mplhep `cmin` behavior). Values the norm
                // cannot place (e.g. ≤ 0 under a log norm) are likewise skipped.
                let Some(color) = self.cell_color(col[iy]) else {
                    continue;
                };
                let y0 = t.y(self.yedges[iy]);
                let y1 = t.y(self.yedges[iy + 1]);
                let (ry, rh) = (y0.min(y1), (y1 - y0).abs());
                // Overlap each cell ~0.5 px into its neighbours so no sliver of the
                // page background shows through sub-pixel seams between cells.
                g.push(DrawCommand::Rect {
                    rect: Rect::new(rx - 0.5, ry - 0.5, rw + 1.0, rh + 1.0),
                    fill: Some(color),
                    stroke: None,
                });
            }
        }
    }
}

/// The set of artist kinds an axes can hold.
#[derive(Debug, Clone)]
pub(crate) enum Artist {
    Line(LineArtist),
    Step(StepArtist),
    Errorbar(ErrorbarArtist),
    Mesh(MeshArtist),
    Band(BandArtist),
    RefLine(RefLineArtist),
    Segments(SegmentsArtist),
    Span(SpanArtist),
}

/// A shaded span across the whole frame between two x or two y values
/// (matplotlib `axvspan`/`axhspan`): a signal region, a blinded window. It
/// does not widen the limits.
#[derive(Debug, Clone)]
pub(crate) struct SpanArtist {
    /// Whether the span runs between two y values (a horizontal strip).
    pub horizontal: bool,
    pub lo: f64,
    pub hi: f64,
    pub face: Option<Color>,
    pub edge: Option<(Color, f32)>,
    /// Lines across the span, and their colour.
    pub hatch: Option<(Hatch, Color)>,
    pub label: Option<String>,
}

impl SpanArtist {
    fn draw(&self, t: &Transform, style: &Style, g: &mut DrawGroup) {
        if !self.lo.is_finite() || !self.hi.is_finite() {
            return;
        }
        let frame = t.frame();
        let rect = if self.horizontal {
            let (a, b) = (t.y(self.lo), t.y(self.hi));
            Rect::new(frame.x, a.min(b), frame.w, (a - b).abs())
        } else {
            let (a, b) = (t.x(self.lo), t.x(self.hi));
            Rect::new(a.min(b), frame.y, (a - b).abs(), frame.h)
        };
        g.push(DrawCommand::Rect {
            rect,
            fill: self.face,
            stroke: self.edge.map(|(c, w)| Stroke::new(c, style.px(w))),
        });
        if let Some((hatch, color)) = self.hatch {
            let stroke = Stroke::new(color, style.px(0.6));
            hatch_rect(g, rect, hatch, style.px(4.5), &stroke);
        }
    }

    fn legend(&self) -> Option<LegendItem> {
        self.label.clone().map(|label| LegendItem {
            label,
            handle: LegendHandle::Patch {
                face: self.face,
                edge: self.edge.map(|(c, _)| c),
                width_pt: self.edge.map_or(0.0, |(_, w)| w),
                hatch: self.hatch,
            },
        })
    }
}

/// A filled region between a lower and an upper curve over the same `xs`
/// (matplotlib `fill_between`): an uncertainty band, a bar.
#[derive(Debug, Clone)]
pub(crate) struct BandArtist {
    pub xs: Vec<f64>,
    pub lo: Vec<f64>,
    pub hi: Vec<f64>,
    pub face: Option<Color>,
    pub edge: Option<(Color, f32)>,
    pub label: Option<String>,
}

impl BandArtist {
    /// The runs of consecutive points where `x`, `lo` and `hi` are finite:
    /// a missing value splits the band.
    fn runs(&self) -> Vec<Vec<usize>> {
        let mut runs = vec![Vec::new()];
        for i in 0..self.xs.len().min(self.lo.len()).min(self.hi.len()) {
            if self.xs[i].is_finite() && self.lo[i].is_finite() && self.hi[i].is_finite() {
                runs.last_mut().expect("one run").push(i);
            } else if !runs.last().expect("one run").is_empty() {
                runs.push(Vec::new());
            }
        }
        runs.retain(|r| !r.is_empty());
        runs
    }

    fn bounds(&self) -> Option<Bounds> {
        let lo = data_bounds(&self.xs, &self.lo);
        let hi = data_bounds(&self.xs, &self.hi);
        match (lo, hi) {
            (Some(a), Some(b)) => Some(a.union(b)),
            (a, b) => a.or(b),
        }
    }

    fn draw(&self, t: &Transform, style: &Style, g: &mut DrawGroup) {
        for run in self.runs() {
            let mut pts: Vec<(f32, f32)> = run
                .iter()
                .map(|&i| t.pt((self.xs[i], self.hi[i])))
                .collect();
            pts.extend(run.iter().rev().map(|&i| t.pt((self.xs[i], self.lo[i]))));
            g.push(DrawCommand::Polygon {
                pts,
                fill: self.face,
                stroke: self.edge.map(|(c, w)| Stroke::new(c, style.px(w))),
            });
        }
    }

    fn legend(&self) -> Option<LegendItem> {
        self.label.clone().map(|label| LegendItem {
            label,
            handle: LegendHandle::Patch {
                face: self.face,
                edge: self.edge.map(|(c, _)| c),
                width_pt: self.edge.map_or(0.0, |(_, w)| w),
                hatch: None,
            },
        })
    }
}

/// A horizontal or vertical line across the whole frame at a data value
/// (matplotlib `axhline`/`axvline`); it does not widen the axis limits.
#[derive(Debug, Clone)]
pub(crate) struct RefLineArtist {
    pub horizontal: bool,
    pub value: f64,
    pub color: Color,
    pub width_pt: f32,
    pub dash: Option<Vec<f32>>,
    pub label: Option<String>,
}

impl RefLineArtist {
    fn draw(&self, t: &Transform, style: &Style, g: &mut DrawGroup) {
        if !self.value.is_finite() {
            return;
        }
        let frame = t.frame();
        let (p0, p1) = if self.horizontal {
            let y = t.y(self.value);
            ((frame.x, y), (frame.right(), y))
        } else {
            let x = t.x(self.value);
            ((x, frame.y), (x, frame.bottom()))
        };
        let mut stroke = Stroke::new(self.color, style.px(self.width_pt));
        stroke.dash = self
            .dash
            .as_ref()
            .map(|d| d.iter().map(|v| style.px(*v)).collect());
        g.push(DrawCommand::Line { p0, p1, stroke });
    }

    fn legend(&self) -> Option<LegendItem> {
        self.label.clone().map(|label| LegendItem {
            label,
            handle: LegendHandle::Line {
                color: self.color,
                width_pt: self.width_pt,
                dash: self.dash.clone(),
                marker: Marker::None,
                marker_size_pt: 0.0,
            },
        })
    }
}

/// Unconnected line segments sharing one style: contour lines.
#[derive(Debug, Clone)]
pub(crate) struct SegmentsArtist {
    pub segments: Vec<[(f64, f64); 2]>,
    pub color: Color,
    pub width_pt: f32,
    pub dash: Option<Vec<f32>>,
    pub label: Option<String>,
}

impl SegmentsArtist {
    fn bounds(&self) -> Option<Bounds> {
        let (xs, ys): (Vec<f64>, Vec<f64>) = self.segments.iter().flatten().copied().unzip();
        data_bounds(&xs, &ys)
    }

    fn draw(&self, t: &Transform, style: &Style, g: &mut DrawGroup) {
        let mut stroke = Stroke::new(self.color, style.px(self.width_pt));
        stroke.dash = self
            .dash
            .as_ref()
            .map(|d| d.iter().map(|v| style.px(*v)).collect());
        for [a, b] in &self.segments {
            g.push(DrawCommand::Line {
                p0: t.pt(*a),
                p1: t.pt(*b),
                stroke: stroke.clone(),
            });
        }
    }

    fn legend(&self) -> Option<LegendItem> {
        self.label.clone().map(|label| LegendItem {
            label,
            handle: LegendHandle::Line {
                color: self.color,
                width_pt: self.width_pt,
                dash: self.dash.clone(),
                marker: Marker::None,
                marker_size_pt: 0.0,
            },
        })
    }
}

impl Artist {
    pub(crate) fn bounds(&self) -> Option<Bounds> {
        match self {
            Artist::Line(a) => a.bounds(),
            Artist::Step(a) => a.bounds(),
            Artist::Errorbar(a) => a.bounds(),
            Artist::Mesh(a) => a.bounds(),
            Artist::Band(a) => a.bounds(),
            Artist::RefLine(_) | Artist::Span(_) => None,
            Artist::Segments(a) => a.bounds(),
        }
    }

    pub(crate) fn draw(&self, t: &Transform, style: &Style, g: &mut DrawGroup) {
        match self {
            Artist::Line(a) => a.draw(t, style, g),
            Artist::Step(a) => a.draw(t, style, g),
            Artist::Errorbar(a) => a.draw(t, style, g),
            Artist::Mesh(a) => a.draw(t, style, g),
            Artist::Band(a) => a.draw(t, style, g),
            Artist::RefLine(a) => a.draw(t, style, g),
            Artist::Segments(a) => a.draw(t, style, g),
            Artist::Span(a) => a.draw(t, style, g),
        }
    }

    /// The smallest finite central y value (a bin's content, a point's y),
    /// leaving out error bars; `None` for a heatmap or no finite values.
    pub(crate) fn value_ymin(&self) -> Option<f64> {
        let ys: &[f64] = match self {
            Artist::Line(a) => &a.ys,
            Artist::Step(a) => &a.values,
            Artist::Errorbar(a) => &a.ys,
            Artist::Band(a) => &a.lo,
            Artist::Mesh(_) | Artist::RefLine(_) | Artist::Segments(_) | Artist::Span(_) => {
                return None;
            }
        };
        ys.iter()
            .copied()
            .filter(|v| v.is_finite())
            .reduce(f64::min)
    }

    /// Points along the artist, in pixels, for judging how much of it a box
    /// would hide (the legend's [`Loc::Best`](crate::Loc::Best)). Outlines
    /// are sampled along their length, so a wide flat bin still counts.
    pub(crate) fn sample_points(&self, t: &Transform) -> Vec<(f32, f32)> {
        /// Points per segment.
        const STEPS: usize = 8;
        /// Spacing of the samples inside a filled area, in pixels.
        const AREA_SPACING: f32 = 8.0;
        let along = |pts: &[(f64, f64)]| -> Vec<(f32, f32)> {
            let px: Vec<(f32, f32)> = pts
                .iter()
                .filter(|(x, y)| x.is_finite() && y.is_finite())
                .map(|&p| t.pt(p))
                .collect();
            if px.len() < 2 {
                return px;
            }
            px.windows(2)
                .flat_map(|w| {
                    (0..STEPS).map(move |k| {
                        let f = k as f32 / STEPS as f32;
                        (
                            w[0].0 + f * (w[1].0 - w[0].0),
                            w[0].1 + f * (w[1].1 - w[0].1),
                        )
                    })
                })
                .chain(px.last().copied())
                .collect()
        };
        match self {
            Artist::Line(a) => along(
                &a.xs
                    .iter()
                    .copied()
                    .zip(a.ys.iter().copied())
                    .collect::<Vec<_>>(),
            ),
            Artist::Step(a) => {
                let mut pts = along(&staircase(&a.edges, &a.values, false));
                for i in 0..a.n() {
                    let (v, (_, e)) = (a.values[i], a.err(i));
                    if v.is_finite() && e > 0.0 {
                        let c = 0.5 * (a.edges[i] + a.edges[i + 1]);
                        pts.push(t.pt((c, v + e)));
                    }
                }
                // A filled histogram or a band covers an area, not just its
                // outline: sample inside it too, so a legend is not placed
                // over the fill. Within the frame only, so a log axis's
                // baseline far below does not spread the samples out of view.
                if matches!(a.histtype, HistType::Fill | HistType::Band) {
                    let frame = t.frame();
                    let clamp = |y: f32| y.clamp(frame.y, frame.bottom());
                    for i in 0..a.n() {
                        let v = a.values[i];
                        if !v.is_finite() {
                            continue;
                        }
                        // A fill spans [0, v]; a band [v − e, v + e].
                        let (hi, lo) = if a.histtype == HistType::Band {
                            (v + a.err(i).1, v - a.err(i).0)
                        } else {
                            (v, 0.0)
                        };
                        let (top, base) = (clamp(t.y(hi)), clamp(t.y(lo)));
                        let (x0, x1) = (t.x(a.edges[i]), t.x(a.edges[i + 1]));
                        // A grid of about AREA_SPACING pixels: finer than any
                        // legend, so none fits between the samples.
                        let nx = crate::num::to_index(f64::from(
                            ((x1 - x0).abs() / AREA_SPACING).ceil().max(1.0),
                        ));
                        let ny = crate::num::to_index(f64::from(
                            ((base - top).abs() / AREA_SPACING).ceil().max(1.0),
                        ));
                        for jx in 0..nx {
                            let x = x0 + (jx as f32 + 0.5) / nx as f32 * (x1 - x0);
                            for jy in 0..=ny {
                                pts.push((x, top + jy as f32 / ny as f32 * (base - top)));
                            }
                        }
                    }
                }
                pts
            }
            Artist::Errorbar(a) => (0..a.n())
                .filter(|&i| a.drawable(i))
                .flat_map(|i| {
                    let (x, y) = (a.xs[i], a.ys[i]);
                    let (yl, yh) = ErrorbarArtist::err_at(&a.yerr, i);
                    [t.pt((x, y)), t.pt((x, y + yh)), t.pt((x, y - yl))]
                })
                .collect(),
            // A heatmap fills the frame: there is no spot it leaves free.
            Artist::Mesh(_) => Vec::new(),
            Artist::Band(a) => a
                .runs()
                .iter()
                .flatten()
                .flat_map(|&i| {
                    let (x, lo, hi) = (a.xs[i], a.lo[i], a.hi[i]);
                    [t.pt((x, lo)), t.pt((x, 0.5 * (lo + hi))), t.pt((x, hi))]
                })
                .collect(),
            Artist::RefLine(a) => {
                let frame = t.frame();
                (0..=STEPS)
                    .map(|k| {
                        let f = k as f32 / STEPS as f32;
                        if a.horizontal {
                            (frame.x + f * frame.w, t.y(a.value))
                        } else {
                            (t.x(a.value), frame.y + f * frame.h)
                        }
                    })
                    .collect()
            }
            Artist::Segments(a) => a.segments.iter().flatten().map(|&p| t.pt(p)).collect(),
            // A span is ground the data stands on: a legend may sit over it.
            Artist::Span(_) => Vec::new(),
        }
    }

    pub(crate) fn legend(&self) -> Option<LegendItem> {
        match self {
            Artist::Line(a) => a.legend(),
            Artist::Step(a) => a.legend(),
            Artist::Errorbar(a) => a.legend(),
            Artist::Mesh(_) => None,
            Artist::Band(a) => a.legend(),
            Artist::RefLine(a) => a.legend(),
            Artist::Segments(a) => a.legend(),
            Artist::Span(a) => a.legend(),
        }
    }
}
