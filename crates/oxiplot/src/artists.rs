//! Artists — the drawable elements added to an [`crate::axes::Axes`]. Each artist
//! owns its data in data-coordinates and knows how to compute its data bounds
//! (for autoscaling) and how to draw itself through a [`Transform`].

use std::fmt;
use std::str::FromStr;

use crate::color::Color;
use crate::draw::{DrawCommand, DrawGroup, Rect, Stroke};
use crate::style::Style;
use crate::transform::{Bounds, Transform};

/// Marker shape for data points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
}

impl Marker {
    /// The matplotlib marker spec (`"none"`, `"o"`, `"s"`, `"^"`).
    #[must_use]
    pub fn spec(self) -> &'static str {
        match self {
            Marker::None => "none",
            Marker::Circle => "o",
            Marker::Square => "s",
            Marker::TriangleUp => "^",
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
    },
}

/// One legend entry.
#[derive(Debug, Clone)]
pub(crate) struct LegendItem {
    pub label: String,
    pub handle: LegendHandle,
}

/// Draw a marker centered at `c` with pixel diameter `size_px`.
pub(crate) fn draw_marker(
    g: &mut DrawGroup,
    shape: Marker,
    c: (f32, f32),
    size_px: f32,
    fill: Option<Color>,
    edge: Option<Stroke>,
) {
    let r = size_px / 2.0;
    match shape {
        Marker::None => {}
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
        Marker::TriangleUp => g.push(DrawCommand::Polygon {
            pts: vec![
                (c.0, c.1 - r),
                (c.0 + r * 0.866, c.1 + r * 0.5),
                (c.0 - r * 0.866, c.1 + r * 0.5),
            ],
            fill,
            stroke: edge,
        }),
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
fn finite_or_zero(v: f64) -> f64 {
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
    pub errs: Option<Vec<f64>>,
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
            let e = self.err(i);
            let (lo, hi) = (self.edges[i], self.edges[i + 1]);
            b = b
                .union(Bounds::new(lo, hi, v - e, v + e))
                .union(Bounds::new(lo, hi, v, v));
        }
        Some(b)
    }

    /// The error of bin `i`, or 0 when there are none or it is not finite.
    fn err(&self, i: usize) -> f64 {
        self.errs
            .as_ref()
            .and_then(|e| e.get(i))
            .copied()
            .map_or(0.0, finite_or_zero)
    }

    fn map(&self, t: &Transform, pts: &[(f64, f64)]) -> Vec<(f32, f32)> {
        pts.iter().map(|&p| t.pt(p)).collect()
    }

    fn draw_errorbars(&self, t: &Transform, style: &Style, g: &mut DrawGroup) {
        if self.errs.is_none() {
            return;
        }
        let stroke = Stroke::new(self.color, style.px(1.0));
        for i in 0..self.n() {
            let (v, e) = (self.values[i], self.err(i));
            if !v.is_finite() || e == 0.0 {
                continue;
            }
            let c = 0.5 * (self.edges[i] + self.edges[i + 1]);
            let cx = t.x(c);
            let ytop = t.y(v + e);
            let ybot = t.y(v - e);
            g.push(DrawCommand::Line {
                p0: (cx, ybot),
                p1: (cx, ytop),
                stroke: stroke.clone(),
            });
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
                self.draw_errorbars(t, style, g);
            }
            HistType::Errorbar => {
                let size = style.px(self.marker_size_pt);
                for i in 0..n {
                    if !self.values[i].is_finite() {
                        continue;
                    }
                    let c = 0.5 * (self.edges[i] + self.edges[i + 1]);
                    let p = t.pt((c, self.values[i]));
                    draw_marker(g, self.marker, p, size, Some(self.color), None);
                }
                self.draw_errorbars(t, style, g);
            }
            HistType::Band => {
                let upper: Vec<f64> = (0..n).map(|i| self.values[i] + self.err(i)).collect();
                let lower: Vec<f64> = (0..n).map(|i| self.values[i] - self.err(i)).collect();
                let mut up = staircase(&self.edges, &upper, false);
                let mut lo = staircase(&self.edges, &lower, false);
                lo.reverse();
                up.append(&mut lo);
                g.push(DrawCommand::Polygon {
                    pts: self.map(t, &up),
                    fill: Some(self.fill_color.unwrap_or(self.color.with_alpha(0.3))),
                    stroke: None,
                });
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
                    face: Some(self.fill_color.unwrap_or(self.color.with_alpha(0.3))),
                    edge: None,
                    width_pt: self.width_pt,
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
            width: nx as u32,
            height: ny as u32,
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
}

impl Artist {
    pub(crate) fn bounds(&self) -> Option<Bounds> {
        match self {
            Artist::Line(a) => a.bounds(),
            Artist::Step(a) => a.bounds(),
            Artist::Errorbar(a) => a.bounds(),
            Artist::Mesh(a) => a.bounds(),
        }
    }

    pub(crate) fn draw(&self, t: &Transform, style: &Style, g: &mut DrawGroup) {
        match self {
            Artist::Line(a) => a.draw(t, style, g),
            Artist::Step(a) => a.draw(t, style, g),
            Artist::Errorbar(a) => a.draw(t, style, g),
            Artist::Mesh(a) => a.draw(t, style, g),
        }
    }

    /// The smallest finite central y value (a bin's content, a point's y),
    /// leaving out error bars; `None` for a heatmap or no finite values.
    pub(crate) fn value_ymin(&self) -> Option<f64> {
        let ys: &[f64] = match self {
            Artist::Line(a) => &a.ys,
            Artist::Step(a) => &a.values,
            Artist::Errorbar(a) => &a.ys,
            Artist::Mesh(_) => return None,
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
                    let (v, e) = (a.values[i], a.err(i));
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
                            (v + a.err(i), v - a.err(i))
                        } else {
                            (v, 0.0)
                        };
                        let (top, base) = (clamp(t.y(hi)), clamp(t.y(lo)));
                        let (x0, x1) = (t.x(a.edges[i]), t.x(a.edges[i + 1]));
                        // A grid of about AREA_SPACING pixels: finer than any
                        // legend, so none fits between the samples.
                        let nx = ((x1 - x0).abs() / AREA_SPACING).ceil().max(1.0) as usize;
                        let ny = ((base - top).abs() / AREA_SPACING).ceil().max(1.0) as usize;
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
        }
    }

    pub(crate) fn legend(&self) -> Option<LegendItem> {
        match self {
            Artist::Line(a) => a.legend(),
            Artist::Step(a) => a.legend(),
            Artist::Errorbar(a) => a.legend(),
            Artist::Mesh(_) => None,
        }
    }
}
