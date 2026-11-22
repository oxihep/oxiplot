//! Bands, reference lines, text and contours, and the HEP result plots built
//! from them: limits with their Brazil band, pulls, impact rankings and
//! credible regions.

use crate::artists::{Artist, BandArtist, LineArtist, Marker, RefLineArtist, SegmentsArtist};
use crate::axes::{Axes, CurveOpts};
use crate::color::Color;
use crate::draw::DrawGroup;
use crate::style::Style;
use crate::text::{HAlign, VAlign};
use crate::transform::Transform;

/// The green of a ±1σ expected band.
const BAND_1SIGMA: Color = Color::rgb(0, 204, 0);
/// The yellow of a ±2σ expected band.
const BAND_2SIGMA: Color = Color::rgb(255, 204, 0);

/// Options for [`Axes::fill_between_with`]: the fill color (default: the
/// next cycle color at 30 % opacity), an outline, a legend label.
#[derive(Debug, Clone, Default)]
pub struct FillOpts {
    pub(crate) color: Option<Color>,
    pub(crate) alpha: Option<f32>,
    pub(crate) edge: Option<(Color, f32)>,
    pub(crate) no_face: bool,
    pub(crate) label: Option<String>,
    pub(crate) hatch: Option<crate::artists::Hatch>,
}

impl FillOpts {
    /// Default options.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// The fill color (its own alpha is kept unless [`alpha`](Self::alpha)
    /// is set).
    #[must_use]
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
    /// The fill opacity, 0–1.
    #[must_use]
    pub fn alpha(mut self, a: f32) -> Self {
        self.alpha = Some(a);
        self
    }
    /// An outline of `width` points.
    #[must_use]
    pub fn edge(mut self, c: Color, width: f32) -> Self {
        self.edge = Some((c, width));
        self
    }
    /// With `on`, no fill: the outline only (with [`edge`](Self::edge)).
    #[must_use]
    pub fn hollow(mut self, on: bool) -> Self {
        self.no_face = on;
        self
    }
    /// The legend label.
    #[must_use]
    pub fn label(mut self, s: impl Into<String>) -> Self {
        self.label = Some(s.into());
        self
    }
    /// Hatch the region in its colour. Only a span
    /// ([`axvspan_with`](crate::Axes::axvspan_with),
    /// [`axhspan_with`](crate::Axes::axhspan_with)) draws it.
    #[must_use]
    pub fn hatch(mut self, hatch: crate::artists::Hatch) -> Self {
        self.hatch = Some(hatch);
        self
    }
}

/// Options for [`Axes::text_with`]: size, color, anchoring, and whether the
/// position is in data or axes coordinates.
#[derive(Debug, Clone)]
pub struct TextOpts {
    pub(crate) size_pt: Option<f32>,
    pub(crate) color: Option<Color>,
    pub(crate) halign: HAlign,
    pub(crate) valign: VAlign,
    pub(crate) axes_coords: bool,
}

impl Default for TextOpts {
    fn default() -> Self {
        TextOpts {
            size_pt: None,
            color: None,
            halign: HAlign::Left,
            valign: VAlign::Baseline,
            axes_coords: false,
        }
    }
}

impl TextOpts {
    /// Default options: the style's font size and color, anchored at the
    /// text's left baseline, in data coordinates.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// The font size in points.
    #[must_use]
    pub fn size(mut self, pt: f32) -> Self {
        self.size_pt = Some(pt);
        self
    }
    /// The text color.
    #[must_use]
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
    /// Which point of the text sits at the position.
    #[must_use]
    pub fn align(mut self, h: HAlign, v: VAlign) -> Self {
        self.halign = h;
        self.valign = v;
        self
    }
    /// With `on`, read the position as a fraction of the frame — `(0, 0)` its
    /// lower left, `(1, 1)` its upper right — instead of data values.
    #[must_use]
    pub fn axes_coords(mut self, on: bool) -> Self {
        self.axes_coords = on;
        self
    }
}

/// A text placed on the axes.
#[derive(Debug, Clone)]
pub(crate) struct Annotation {
    x: f64,
    y: f64,
    text: String,
    opts: TextOpts,
}

impl Annotation {
    /// The anchor in pixels; `None` for a non-finite data position.
    fn anchor(&self, t: &Transform) -> Option<(f32, f32)> {
        if self.opts.axes_coords {
            let f = t.frame();
            Some((
                f.x + crate::num::to_px(self.x) * f.w,
                f.bottom() - crate::num::to_px(self.y) * f.h,
            ))
        } else if self.x.is_finite() && self.y.is_finite() {
            Some(t.pt((self.x, self.y)))
        } else {
            None
        }
    }

    /// The text's box in pixels, as drawn: what the legend keeps clear of.
    pub(crate) fn rect(&self, t: &Transform, style: &Style) -> Option<crate::draw::Rect> {
        let (px, py) = self.anchor(t)?;
        let size = style.px(self.opts.size_pt.unwrap_or(style.font_size_pt));
        let (w, ascent, descent) = crate::mathtext::label_extents(&style.fonts, &self.text, size);
        let h = ascent + descent;
        let x = match self.opts.halign {
            HAlign::Center => px - w / 2.0,
            HAlign::Right => px - w,
            _ => px,
        };
        let y = match self.opts.valign {
            VAlign::Top => py,
            VAlign::Middle => py - h / 2.0,
            VAlign::Bottom => py - h,
            _ => py - ascent,
        };
        Some(crate::draw::Rect::new(x, y, w, h))
    }

    pub(crate) fn draw(&self, t: &Transform, style: &Style, g: &mut DrawGroup) {
        let (px, py) = if self.opts.axes_coords {
            let f = t.frame();
            #[allow(
                clippy::cast_possible_truncation,
                reason = "a fraction of the frame, in pixels"
            )]
            let p = (f.x + self.x as f32 * f.w, f.bottom() - self.y as f32 * f.h);
            p
        } else {
            if !self.x.is_finite() || !self.y.is_finite() {
                return;
            }
            t.pt((self.x, self.y))
        };
        crate::mathtext::layout_label(
            g,
            &style.fonts,
            &self.text,
            px,
            py,
            style.px(self.opts.size_pt.unwrap_or(style.font_size_pt)),
            self.opts.color.unwrap_or(style.fg_color),
            self.opts.halign,
            self.opts.valign,
            0.0,
        );
    }
}

#[cfg(feature = "fit")]
/// `grid` smoothed by a Gaussian of one cell's width (as corner.py does
/// before contouring a histogram of draws), renormalized at the edges.
fn smooth(grid: &[Vec<f64>]) -> Vec<Vec<f64>> {
    const KERNEL: [f64; 5] = [0.135_335, 0.606_531, 1.0, 0.606_531, 0.135_335];
    let pass = |g: &[Vec<f64>], along_x: bool| -> Vec<Vec<f64>> {
        let (nx, ny) = (g.len(), g.first().map_or(0, Vec::len));
        let mut out = vec![vec![0.0; ny]; nx];
        for (i, row) in out.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                let (mut sum, mut w) = (0.0, 0.0);
                for (k, kw) in KERNEL.iter().enumerate() {
                    let (a, b) = if along_x {
                        ((i + k).checked_sub(2), Some(j))
                    } else {
                        (Some(i), (j + k).checked_sub(2))
                    };
                    if let (Some(a), Some(b)) = (a, b)
                        && a < nx
                        && b < ny
                    {
                        sum += kw * g[a][b];
                        w += kw;
                    }
                }
                *cell = sum / w;
            }
        }
        out
    };
    pass(&pass(grid, true), false)
}

/// The segments of the `level` contour of `z` on the grid `x × y` (`z[i][j]`
/// at `(x[i], y[j])`), by marching squares with linear interpolation.
fn marching_squares(x: &[f64], y: &[f64], z: &[Vec<f64>], level: f64) -> Vec<[(f64, f64); 2]> {
    let mut out = Vec::new();
    if x.len() < 2 || y.len() < 2 || z.len() < x.len() {
        return out;
    }
    let at = |i: usize, j: usize| z[i].get(j).copied().unwrap_or(f64::NAN);
    // The crossing on the edge from corner a to corner b.
    let cross = |(xa, ya, za): (f64, f64, f64), (xb, yb, zb): (f64, f64, f64)| {
        let f = (level - za) / (zb - za);
        (xa + f * (xb - xa), ya + f * (yb - ya))
    };
    for i in 0..x.len() - 1 {
        for j in 0..y.len() - 1 {
            // Corners counter-clockwise from the lower left.
            let c = [
                (x[i], y[j], at(i, j)),
                (x[i + 1], y[j], at(i + 1, j)),
                (x[i + 1], y[j + 1], at(i + 1, j + 1)),
                (x[i], y[j + 1], at(i, j + 1)),
            ];
            if c.iter().any(|p| !p.2.is_finite()) {
                continue;
            }
            let above: Vec<bool> = c.iter().map(|p| p.2 >= level).collect();
            // The edges whose ends lie on either side of the level.
            let edges: Vec<(f64, f64)> = (0..4)
                .filter(|&k| above[k] != above[(k + 1) % 4])
                .map(|k| cross(c[k], c[(k + 1) % 4]))
                .collect();
            match edges.len() {
                2 => out.push([edges[0], edges[1]]),
                4 => {
                    // A saddle: split by the cell's mean.
                    let mean = c.iter().map(|p| p.2).sum::<f64>() / 4.0;
                    if (mean >= level) == above[0] {
                        out.push([edges[0], edges[1]]);
                        out.push([edges[2], edges[3]]);
                    } else {
                        out.push([edges[0], edges[3]]);
                        out.push([edges[1], edges[2]]);
                    }
                }
                _ => {}
            }
        }
    }
    out
}

impl Axes {
    /// The text annotations' boxes in pixels, on a frame at `box_`.
    pub(crate) fn annotation_rects_px(&self, box_: crate::draw::Rect) -> Vec<crate::draw::Rect> {
        let t = self.frame_transform(box_);
        self.annotations
            .iter()
            .filter_map(|a| a.rect(&t, &self.style))
            .collect()
    }

    /// Fill the region between `lo` and `hi` over `xs` (matplotlib
    /// `fill_between`), in the next cycle color at 30 % opacity. A
    /// non-finite value splits the band.
    ///
    /// # Errors
    /// [`Error::LengthMismatch`](crate::Error::LengthMismatch) if `lo` or `hi`
    /// is not as long as `xs`; nothing is drawn.
    pub fn fill_between(&mut self, xs: &[f64], lo: &[f64], hi: &[f64]) -> crate::Result<&mut Self> {
        self.fill_between_with(xs, lo, hi, FillOpts::new())
    }

    /// [`fill_between`](Self::fill_between) with options.
    ///
    /// # Errors
    /// As [`fill_between`](Self::fill_between).
    ///
    /// # Examples
    /// ```
    /// use oxiplot::{Axes, Color, FillOpts};
    /// let x = [0.0, 1.0, 2.0];
    /// let mut ax = Axes::new();
    /// ax.fill_between_with(&x, &[0.5, 0.8, 1.2], &[1.5, 1.9, 2.4],
    ///     FillOpts::new().color(Color::hex("#00cc00")).label("±1σ"))?;
    /// # Ok::<(), oxiplot::Error>(())
    /// ```
    pub fn fill_between_with(
        &mut self,
        xs: &[f64],
        lo: &[f64],
        hi: &[f64],
        opts: FillOpts,
    ) -> crate::Result<&mut Self> {
        for (what, v) in [("fill_between lo", lo), ("fill_between hi", hi)] {
            if v.len() != xs.len() {
                return Err(crate::Error::LengthMismatch {
                    what,
                    expected: xs.len(),
                    found: v.len(),
                });
            }
        }
        let face = if opts.no_face {
            None
        } else {
            let c = opts.color.unwrap_or_else(|| self.next_color());
            Some(match (opts.alpha, opts.color) {
                (Some(a), _) => c.with_alpha(a),
                (None, Some(_)) => c,
                (None, None) => c.with_alpha(0.3),
            })
        };
        self.add_artist(Artist::Band(BandArtist {
            xs: xs.to_vec(),
            lo: lo.to_vec(),
            hi: hi.to_vec(),
            face,
            edge: opts.edge,
            label: opts.label,
        }));
        Ok(self)
    }

    fn ref_line(&mut self, horizontal: bool, value: f64, opts: CurveOpts) -> &mut Self {
        let color = opts.color.unwrap_or(Color::rgb(128, 128, 128));
        self.add_artist(Artist::RefLine(RefLineArtist {
            horizontal,
            value,
            color,
            width_pt: opts.linewidth_pt.unwrap_or(self.style.line_width_pt),
            dash: opts.dash,
            label: opts.label,
        }));
        self
    }

    fn span(&mut self, horizontal: bool, lo: f64, hi: f64, opts: FillOpts) -> &mut Self {
        let color = opts.color.unwrap_or(Color::rgb(128, 128, 128));
        let face = (!opts.no_face).then(|| color.with_alpha(opts.alpha.unwrap_or(0.2)));
        self.add_artist(Artist::Span(crate::artists::SpanArtist {
            horizontal,
            lo,
            hi,
            face,
            edge: opts.edge,
            hatch: opts.hatch.map(|h| (h, color)),
            label: opts.label,
        }));
        self
    }

    /// Shade the frame over a range of x values in translucent gray
    /// (matplotlib's `axvspan`): a signal region, a blinded window. It does
    /// not widen the limits.
    pub fn axvspan(&mut self, range: std::ops::Range<f64>) -> &mut Self {
        self.span(false, range.start, range.end, FillOpts::new())
    }

    /// [`axvspan`](Self::axvspan) with a colour, opacity, edge, hatch and
    /// legend label.
    ///
    /// ```
    /// use oxiplot::{Axes, Color, FillOpts, Hatch};
    /// let mut ax = Axes::new();
    /// ax.plot(&[100.0, 160.0], &[9.0, 2.0])?;
    /// ax.axvspan_with(
    ///     120.0..130.0,
    ///     FillOpts::new().hollow(true).color(Color::BLACK).hatch(Hatch::Diagonal).label("blinded"),
    /// );
    /// # Ok::<(), oxiplot::Error>(())
    /// ```
    pub fn axvspan_with(&mut self, range: std::ops::Range<f64>, opts: FillOpts) -> &mut Self {
        self.span(false, range.start, range.end, opts)
    }

    /// Shade the frame over a range of y values (matplotlib's `axhspan`): a
    /// ±1σ strip in a pull or ratio panel.
    pub fn axhspan(&mut self, range: std::ops::Range<f64>) -> &mut Self {
        self.span(true, range.start, range.end, FillOpts::new())
    }

    /// [`axhspan`](Self::axhspan) with options.
    pub fn axhspan_with(&mut self, range: std::ops::Range<f64>, opts: FillOpts) -> &mut Self {
        self.span(true, range.start, range.end, opts)
    }

    /// A gray horizontal line across the frame at `y` (it does not widen
    /// the limits).
    pub fn axhline(&mut self, y: f64) -> &mut Self {
        self.ref_line(true, y, CurveOpts::new())
    }

    /// [`axhline`](Self::axhline) with a color, width, dash and label.
    pub fn axhline_with(&mut self, y: f64, opts: CurveOpts) -> &mut Self {
        self.ref_line(true, y, opts)
    }

    /// A gray vertical line across the frame at `x`.
    pub fn axvline(&mut self, x: f64) -> &mut Self {
        self.ref_line(false, x, CurveOpts::new())
    }

    /// [`axvline`](Self::axvline) with options.
    pub fn axvline_with(&mut self, x: f64, opts: CurveOpts) -> &mut Self {
        self.ref_line(false, x, opts)
    }

    /// Write `text` (`$…$` is typeset) with its left baseline at the data
    /// point `(x, y)`.
    pub fn text(&mut self, x: f64, y: f64, text: impl Into<String>) -> &mut Self {
        self.text_with(x, y, text, TextOpts::new())
    }

    /// [`text`](Self::text) with options — size, color, anchoring, and axes
    /// coordinates (`TextOpts::new().axes_coords(true)` for a fixed spot, such as
    /// `(0.05, 0.9)` near the upper left).
    pub fn text_with(
        &mut self,
        x: f64,
        y: f64,
        text: impl Into<String>,
        opts: TextOpts,
    ) -> &mut Self {
        self.annotations_mut().push(Annotation {
            x,
            y,
            text: text.into(),
            opts,
        });
        self
    }

    /// Name the y axis's values `0, 1, …, n − 1` with `labels`, bottom to
    /// top (a ranking, a list of parameters): the ticks become the labels.
    pub fn y_categories<S: AsRef<str>>(&mut self, labels: &[S]) -> &mut Self {
        let cats = labels
            .iter()
            .enumerate()
            .map(|(i, l)| (i as f64, l.as_ref().to_string()))
            .collect();
        self.set_y_categories(cats);
        self
    }

    /// Contour lines of `z` on the grid `x × y` (`z[i][j]` at `(x[i], y[j])`)
    /// at each of `levels`, all in one style (the next cycle color unless
    /// set). Lines are drawn by marching squares between the grid points.
    pub fn contour(
        &mut self,
        x: &[f64],
        y: &[f64],
        z: &[Vec<f64>],
        levels: &[f64],
        opts: CurveOpts,
    ) -> &mut Self {
        let segments: Vec<[(f64, f64); 2]> = levels
            .iter()
            .flat_map(|&l| marching_squares(x, y, z, l))
            .collect();
        let color = opts.color.unwrap_or_else(|| self.next_color());
        self.add_artist(Artist::Segments(SegmentsArtist {
            segments,
            color,
            width_pt: opts.linewidth_pt.unwrap_or(self.style.line_width_pt),
            dash: opts.dash,
            label: opts.label,
        }));
        self
    }

    /// An upper-limit plot against `x` (a mass, a coupling): the expected
    /// limit's ±2σ (yellow) and ±1σ (green) bands, its median (dashed), and
    /// the observed limit (solid black with points). `expected[i]` holds the
    /// −2σ, −1σ, median, +1σ, +2σ expected limits at `x[i]`.
    ///
    /// # Errors
    /// [`Error::LengthMismatch`](crate::Error::LengthMismatch) if `observed`
    /// or `expected` is not as long as `x`; nothing is drawn.
    ///
    /// # Examples
    /// ```
    /// use oxiplot::Axes;
    /// let m = [100.0, 150.0, 200.0];
    /// let obs = [1.1, 0.8, 1.3];
    /// let exp = [[0.5, 0.7, 1.0, 1.4, 1.9], [0.4, 0.6, 0.9, 1.3, 1.8], [0.6, 0.8, 1.1, 1.5, 2.1]];
    /// let mut ax = Axes::new();
    /// ax.limit_band(&m, &obs, &exp)?.axhline(1.0).legend();
    /// # Ok::<(), oxiplot::Error>(())
    /// ```
    pub fn limit_band(
        &mut self,
        x: &[f64],
        observed: &[f64],
        expected: &[[f64; 5]],
    ) -> crate::Result<&mut Self> {
        for (what, n) in [
            ("limit_band observed", observed.len()),
            ("limit_band expected", expected.len()),
        ] {
            if n != x.len() {
                return Err(crate::Error::LengthMismatch {
                    what,
                    expected: x.len(),
                    found: n,
                });
            }
        }
        let band = |k: usize| -> Vec<f64> { expected.iter().map(|e| e[k]).collect() };
        self.fill_between_with(
            x,
            &band(0),
            &band(4),
            FillOpts::new().color(BAND_2SIGMA).label("Expected ±2σ"),
        )?;
        self.fill_between_with(
            x,
            &band(1),
            &band(3),
            FillOpts::new().color(BAND_1SIGMA).label("Expected ±1σ"),
        )?;
        let width = self.style.line_width_pt;
        self.add_artist(Artist::Line(LineArtist {
            xs: x.to_vec(),
            ys: band(2),
            color: Color::BLACK,
            width_pt: width,
            dash: Some(vec![4.0, 3.0]),
            marker: Marker::None,
            marker_size_pt: 0.0,
            label: Some("Expected".to_string()),
        }));
        self.add_artist(Artist::Line(LineArtist {
            xs: x.to_vec(),
            ys: observed.to_vec(),
            color: Color::BLACK,
            width_pt: width,
            dash: None,
            marker: Marker::Circle,
            marker_size_pt: 0.6 * self.style.marker_size_pt,
            label: Some("Observed".to_string()),
        }));
        Ok(self)
    }
}

#[cfg(feature = "fit")]
impl Axes {
    /// [`limit_band`](Self::limit_band) from oxifit's
    /// [`HypoTest::upper_limit`](oxifit::HypoTest::upper_limit) results, one
    /// per `x`.
    ///
    /// # Errors
    /// As [`limit_band`](Self::limit_band).
    pub fn fit_limits(
        &mut self,
        x: &[f64],
        limits: &[oxifit::UpperLimit],
    ) -> crate::Result<&mut Self> {
        let observed: Vec<f64> = limits.iter().map(|l| l.observed).collect();
        let expected: Vec<[f64; 5]> = limits.iter().map(|l| l.expected).collect();
        self.limit_band(x, &observed, &expected)
    }

    /// The nuisance-parameter pulls of a template fit
    /// ([`TemplateModel::pulls`](oxifit::TemplateModel::pulls): name, shift
    /// and post-fit error in prior widths): one row per parameter, the first
    /// at the top, its shift with a horizontal error bar, over ±1σ (green)
    /// and ±2σ (yellow) bands.
    pub fn fit_pulls(&mut self, pulls: &[(String, f64, f64)]) -> &mut Self {
        let n = pulls.len();
        let rows: Vec<f64> = (0..n).map(|i| (n - 1 - i) as f64).collect();
        let span = [-0.5, n as f64 - 0.5];
        // The bands are vertical: fill across x between the two y extremes.
        for (k, color) in [(2.0, BAND_2SIGMA), (1.0, BAND_1SIGMA)] {
            self.add_artist(Artist::Band(BandArtist {
                xs: vec![-k, k],
                lo: vec![span[0]; 2],
                hi: vec![span[1]; 2],
                face: Some(color),
                edge: None,
                label: None,
            }));
        }
        self.axvline_with(0.0, CurveOpts::new().dashed(vec![3.0, 2.0]));
        let xs: Vec<f64> = pulls.iter().map(|p| p.1).collect();
        let errs: Vec<f64> = pulls.iter().map(|p| p.2).collect();
        self.add_artist(Artist::Errorbar(crate::artists::ErrorbarArtist {
            xs,
            ys: rows,
            xerr: Some((errs.clone(), errs)),
            yerr: None,
            color: Color::BLACK,
            marker: Marker::Circle,
            marker_size_pt: self.style.marker_size_pt,
            elinewidth_pt: self.style.line_width_pt,
            capsize_pt: 0.0,
            line_width_pt: None,
            label: None,
        }));
        let names: Vec<&str> = pulls.iter().rev().map(|p| p.0.as_str()).collect();
        self.y_categories(&names);
        self.ylim(span[0]..span[1]);
        // Symmetric about 0, past the ±2σ band and every error bar.
        let reach = pulls
            .iter()
            .map(|p| p.1.abs() + p.2.abs())
            .filter(|v| v.is_finite())
            .fold(2.0_f64, f64::max);
        self.xlim(-1.1 * reach..1.1 * reach);
        self.xlabel(r"$(\hat\theta - \theta_0)/\Delta\theta$");
        self
    }

    /// The impact ranking of a template fit
    /// ([`TemplateModel::impacts`](oxifit::TemplateModel::impacts)): the `top`
    /// largest, the largest at the top, each a pair of bars — the shift of
    /// the parameter of interest with the nuisance parameter moved up (blue)
    /// and down (red) by its post-fit error (filled) and its prior width
    /// (outlined).
    pub fn fit_impacts(&mut self, impacts: &[oxifit::Impact], top: usize) -> &mut Self {
        let mut ranked: Vec<&oxifit::Impact> = impacts.iter().collect();
        ranked.sort_by(|a, b| b.size().total_cmp(&a.size()));
        ranked.truncate(top);
        let n = ranked.len();
        let up = Color::rgb(31, 119, 180);
        let down = Color::rgb(214, 39, 40);
        let bar = |ax: &mut Axes, row: f64, shift: f64, color: Color, filled: bool, label| {
            let (face, edge) = if filled {
                (Some(color.with_alpha(0.75)), None)
            } else {
                (None, Some((color, 1.0)))
            };
            ax.add_artist(Artist::Band(BandArtist {
                xs: vec![0.0, shift],
                lo: vec![row - 0.35; 2],
                hi: vec![row + 0.35; 2],
                face,
                edge,
                label,
            }));
        };
        for (i, imp) in ranked.iter().enumerate() {
            let row = (n - 1 - i) as f64;
            let first = i == 0;
            bar(
                self,
                row,
                imp.prefit.0,
                up,
                false,
                first.then(|| "+1σ prior".to_string()),
            );
            bar(
                self,
                row,
                imp.prefit.1,
                down,
                false,
                first.then(|| "−1σ prior".to_string()),
            );
            bar(
                self,
                row,
                imp.postfit.0,
                up,
                true,
                first.then(|| "+1σ post-fit".to_string()),
            );
            bar(
                self,
                row,
                imp.postfit.1,
                down,
                true,
                first.then(|| "−1σ post-fit".to_string()),
            );
        }
        self.axvline(0.0);
        let names: Vec<&str> = ranked.iter().rev().map(|i| i.name.as_str()).collect();
        self.y_categories(&names);
        self.ylim(-0.5..n as f64 - 0.5);
        self.xlabel(r"$\Delta\mu$");
        self
    }

    /// The 68 % and 95 % credible regions of parameters `x` and `y` from an
    /// MCMC [`Chain`](oxifit::Chain): a 2-D histogram of the draws in
    /// `bins × bins` cells, contoured at the densities that enclose 68 %
    /// (solid) and 95 % (dashed) of them, and the posterior median as a
    /// point. Returns `self` unchanged for an unknown name.
    pub fn fit_posterior(
        &mut self,
        chain: &oxifit::Chain,
        x: &str,
        y: &str,
        bins: usize,
    ) -> &mut Self {
        let (Some(xs), Some(ys)) = (chain.values(x), chain.values(y)) else {
            return self;
        };
        let bins = bins.max(4);
        let range = |v: &[f64]| {
            let lo = v.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let pad = 0.05 * (hi - lo).max(f64::MIN_POSITIVE);
            (lo - pad, hi + pad)
        };
        let ((x0, x1), (y0, y1)) = (range(&xs), range(&ys));
        let (wx, wy) = ((x1 - x0) / bins as f64, (y1 - y0) / bins as f64);
        let mut grid = vec![vec![0.0; bins]; bins];
        for (&a, &b) in xs.iter().zip(&ys) {
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "inside the padded range: a cell index"
            )]
            let (i, j) = (((a - x0) / wx) as usize, ((b - y0) / wy) as usize);
            grid[i.min(bins - 1)][j.min(bins - 1)] += 1.0;
        }
        let grid = smooth(&grid);
        // The density levels enclosing 68 % and 95 % of the draws.
        let mut counts: Vec<f64> = grid.iter().flatten().copied().collect();
        counts.sort_by(|a, b| b.total_cmp(a));
        let total: f64 = counts.iter().sum();
        let level = |p: f64| {
            let mut acc = 0.0;
            for &c in &counts {
                acc += c;
                if acc >= p * total {
                    return c;
                }
            }
            0.0
        };
        let cx: Vec<f64> = (0..bins).map(|i| x0 + (i as f64 + 0.5) * wx).collect();
        let cy: Vec<f64> = (0..bins).map(|j| y0 + (j as f64 + 0.5) * wy).collect();
        let color = self.next_color();
        self.contour(
            &cx,
            &cy,
            &grid,
            &[level(0.6827)],
            CurveOpts::new().color(color).label("68 % credible"),
        );
        self.contour(
            &cx,
            &cy,
            &grid,
            &[level(0.9545)],
            CurveOpts::new()
                .color(color)
                .dashed(vec![4.0, 3.0])
                .label("95 % credible"),
        );
        if let (Some(mx), Some(my)) = (chain.quantile(x, 0.5), chain.quantile(y, 0.5)) {
            self.add_artist(Artist::Line(LineArtist {
                xs: vec![mx],
                ys: vec![my],
                color,
                width_pt: 0.0,
                dash: None,
                marker: Marker::Circle,
                marker_size_pt: self.style.marker_size_pt,
                label: None,
            }));
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::marching_squares;
    use crate::Axes;
    use crate::artists::Artist;

    #[test]
    fn a_circle_contour_lies_on_the_circle() {
        let g: Vec<f64> = (0..41).map(|i| -2.0 + 0.1 * f64::from(i)).collect();
        let z: Vec<Vec<f64>> = g
            .iter()
            .map(|&x| g.iter().map(|&y| x * x + y * y).collect())
            .collect();
        let segs = marching_squares(&g, &g, &z, 1.0);
        assert!(segs.len() > 40);
        for p in segs.iter().flatten() {
            let r = (p.0 * p.0 + p.1 * p.1).sqrt();
            assert!((r - 1.0).abs() < 0.01, "{r}");
        }
    }

    #[test]
    fn limit_band_and_reference_lines_draw_and_label() {
        let mut ax = Axes::new();
        let m = [1.0, 2.0, 3.0];
        let e = [[0.5, 0.7, 1.0, 1.4, 1.9]; 3];
        ax.limit_band(&m, &[1.1, 0.8, 1.3], &e)
            .unwrap()
            .axhline(1.0)
            .text(1.5, 2.0, "95 % CL");
        let labels: Vec<String> = ax.legend_items().into_iter().map(|i| i.label).collect();
        assert_eq!(
            labels,
            ["Expected ±2σ", "Expected ±1σ", "Expected", "Observed"]
        );
        assert!(ax.limit_band(&m, &[1.0], &e).is_err());
        // The reference line does not widen the limits; the bands do.
        let (lo, hi) = ax.resolved_ylim();
        assert!(lo <= 0.5 && (1.9..3.0).contains(&hi), "{lo} {hi}");
        assert!(ax.to_svg_string().contains("<svg"));
        assert!(matches!(ax.artists[4], Artist::RefLine(_)));
    }

    #[test]
    fn y_categories_label_the_ticks() {
        let mut ax = Axes::new();
        ax.plot(&[0.0, 1.0], &[0.0, 2.0]).unwrap();
        ax.y_categories(&["a", "b", "c"]);
        assert!(ax.to_svg_string().contains("<svg"));
    }
}
