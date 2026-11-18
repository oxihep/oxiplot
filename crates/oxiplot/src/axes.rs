//! The `Axes` — a single plot panel: data limits, the frame, ticks, labels, and
//! the artists drawn within it. Mirrors matplotlib's `Axes` API.

use std::ops::Range;

use crate::artists::{
    Artist, ErrorbarArtist, HistType, LineArtist, Marker, MeshArtist, StepArtist,
};
use crate::cmap::Colormap;
use crate::color::Color;
use crate::colorbar::ColorbarSpec;
use crate::data::{Hist1dData, Hist2dData, PointData};
use crate::draw::{DrawCommand, DrawGroup, Rect, Stroke};
use crate::figure::SaveOpts;
use crate::legend::Loc;
use crate::style::{Style, TickDir};
use crate::text::{self, FontStyle, HAlign, VAlign};
use crate::ticker;
use crate::timeaxis::TimeFormat;
use crate::transform::{Bounds, Scale, Transform};

/// Gap between the frame and the bottom of the title, in points.
const TITLE_PAD_PT: f32 = 6.0;
/// The least room left between the top of a title and the top of the figure,
/// in points.
const TITLE_EDGE_PT: f32 = 2.0;
/// Gap between the tick labels and the axis label, in points.
const AXIS_LABEL_PAD_PT: f32 = 4.0;
/// Gap between the frame and the experiment label's baseline, in points.
const HEP_LABEL_PAD_PT: f32 = 5.0;

/// How far an axes' decorations reach outside its frame, in pixels.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Insets {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
    /// The part of `left` taken by the y tick marks and labels (the y-axis
    /// label sits beyond it).
    pub ytick_area: f32,
}

/// Major tick positions and labels for one frame.
struct TickLayout {
    limits: (f64, f64, f64, f64),
    xticks: Vec<f64>,
    yticks: Vec<f64>,
    xlabels: Vec<String>,
    ylabels: Vec<String>,
}

/// A single plot panel.
#[derive(Clone)]
pub struct Axes {
    pub(crate) style: Style,
    /// Limits set by the caller (`xlim`/`ylim`); they win over everything.
    xlim: Option<(f64, f64)>,
    ylim: Option<(f64, f64)>,
    /// Limits the data asks for: a histogram's outer edges (matplotlib's
    /// "sticky edges", so the bars meet the frame). Used when the caller set
    /// no limit, instead of the padded autoscale.
    data_xlim: Option<(f64, f64)>,
    data_ylim: Option<(f64, f64)>,
    xlabel: Option<String>,
    ylabel: Option<String>,
    title: Option<String>,
    /// mplhep experiment label `(experiment, status)`, drawn above the axes.
    hep_label: Option<(String, String)>,
    /// Right-hand label above the axes (luminosity / energy).
    hep_rhs: Option<String>,
    pub(crate) artists: Vec<Artist>,
    color_idx: usize,
    show_legend: bool,
    /// When set (by `hist`/`hist_with`), the y autoscale starts at 0.
    pub(crate) y_from_zero: bool,
    colorbar: Option<ColorbarSpec>,
    grid_minor: bool,
    /// Whether the x tick labels and x-axis label are drawn (hidden on the upper
    /// panel of a shared-x layout such as a ratio plot).
    show_xticklabels: bool,
    /// Whether the y tick labels and y-axis label are drawn (hidden on the
    /// non-left columns of a shared-y grid).
    show_yticklabels: bool,
    /// Drop the `(bottom, top)` y tick labels (avoids overlap at a shared seam).
    ylabel_prune: (bool, bool),
    /// The width left of the frame given to the y tick labels, when a figure
    /// aligns the y-axis labels of a column (otherwise this axes' own).
    pub(crate) ytick_area: Option<f32>,
    /// Where the legend goes.
    legend_loc: Loc,
    /// How each axis maps values onto its length.
    xscale: Scale,
    yscale: Scale,
    /// A ROOT-style fit statistics box (`TPaveStats`), set by [`Axes::fit_stats`].
    #[cfg(feature = "fit")]
    stats: Option<crate::statbox::StatData>,
    /// When set, the x values are times and its labels are drawn as dates and
    /// clock times (`Axes::x_time_format`, or a plotted time axis).
    x_time: Option<TimeFormat>,
}

impl Axes {
    /// A new empty axes with the given style.
    #[must_use]
    pub fn with_style(style: Style) -> Self {
        Axes {
            style,
            xlim: None,
            ylim: None,
            data_xlim: None,
            data_ylim: None,
            xlabel: None,
            ylabel: None,
            title: None,
            hep_label: None,
            hep_rhs: None,
            artists: Vec::new(),
            color_idx: 0,
            show_legend: false,
            y_from_zero: false,
            x_time: None,
            colorbar: None,
            grid_minor: false,
            show_xticklabels: true,
            show_yticklabels: true,
            ylabel_prune: (false, false),
            ytick_area: None,
            legend_loc: Loc::Best,
            xscale: Scale::Linear,
            yscale: Scale::Linear,
            #[cfg(feature = "fit")]
            stats: None,
        }
    }

    /// Drop the bottom-most and/or top-most y tick label (used to avoid overlap
    /// at the shared seam of a ratio plot).
    pub(crate) fn prune_ylabels(&mut self, bottom: bool, top: bool) {
        self.ylabel_prune = (bottom, top);
    }

    /// A new empty axes with the default (matplotlib) style.
    #[must_use]
    pub fn new() -> Self {
        Axes::with_style(Style::default())
    }

    /// Next color from the style's cycle (advances the cycle).
    pub(crate) fn next_color(&mut self) -> Color {
        let c = self.style.cycle(self.color_idx);
        self.color_idx += 1;
        c
    }

    pub(crate) fn add_artist(&mut self, a: Artist) {
        self.artists.push(a);
    }

    pub(crate) fn legend_items(&self) -> Vec<crate::artists::LegendItem> {
        self.artists.iter().filter_map(Artist::legend).collect()
    }

    /// Set the x-axis label (supports `$…$` math).
    pub fn xlabel(&mut self, s: impl Into<String>) -> &mut Self {
        self.xlabel = Some(s.into());
        self
    }
    /// Set the y-axis label.
    pub fn ylabel(&mut self, s: impl Into<String>) -> &mut Self {
        self.ylabel = Some(s.into());
        self
    }
    /// Add an mplhep-style experiment label above the axes: a bold `experiment`
    /// name (e.g. `"CMS"`, `"ATLAS"`) followed by an italic `status`
    /// (e.g. `"Preliminary"`, `"Simulation"`; pass `""` for none). Pair it with
    /// [`hep_rhs`](Axes::hep_rhs) for the luminosity / energy on the right.
    ///
    /// # Examples
    /// ```
    /// # use oxiplot::{Axes, Style};
    /// let mut ax = Axes::with_style(Style::mplhep());
    /// ax.hep_label("CMS", "Preliminary").hep_rhs("138 fb$^{-1}$ (13 TeV)");
    /// ```
    pub fn hep_label(
        &mut self,
        experiment: impl Into<String>,
        status: impl Into<String>,
    ) -> &mut Self {
        self.hep_label = Some((experiment.into(), status.into()));
        self
    }

    /// Set the right-hand text above the axes (luminosity / energy, e.g.
    /// `"138 fb$^{-1}$ (13 TeV)"`), right-aligned to the frame. Supports `$…$`.
    pub fn hep_rhs(&mut self, text: impl Into<String>) -> &mut Self {
        self.hep_rhs = Some(text.into());
        self
    }

    /// Set the title.
    pub fn title(&mut self, s: impl Into<String>) -> &mut Self {
        self.title = Some(s.into());
        self
    }
    /// Set the x-axis limits, e.g. `ax.xlim(0.0..100.0)`.
    pub fn xlim(&mut self, range: Range<f64>) -> &mut Self {
        self.xlim = Some((range.start, range.end));
        self
    }
    /// Set the y-axis limits, e.g. `ax.ylim(0.5..1.5)`.
    pub fn ylim(&mut self, range: Range<f64>) -> &mut Self {
        self.ylim = Some((range.start, range.end));
        self
    }
    /// Set the x axis's scale, e.g. `ax.xscale(Scale::Log)` (matplotlib's
    /// `set_xscale`).
    pub fn xscale(&mut self, scale: Scale) -> &mut Self {
        self.xscale = scale;
        self
    }

    /// Set the y axis's scale: `ax.yscale(Scale::Log)` for a log axis, ticked
    /// at the powers of ten. A histogram on a log axis starts below its
    /// smallest positive bin rather than at 0 (which a log axis cannot show).
    ///
    /// # Examples
    /// ```
    /// use oxiplot::{Axes, Binned1D, Scale};
    /// let h = Binned1D::new(vec![0.0, 1.0, 2.0, 3.0], vec![1e4, 300.0, 2.0])?;
    /// let mut ax = Axes::new();
    /// ax.hist(&h).yscale(Scale::Log);
    /// # Ok::<(), oxiplot::Error>(())
    /// ```
    pub fn yscale(&mut self, scale: Scale) -> &mut Self {
        self.yscale = scale;
        self
    }

    /// Enable the legend, placed where it covers the least data
    /// ([`Loc::Best`], matplotlib's default).
    pub fn legend(&mut self) -> &mut Self {
        self.show_legend = true;
        self
    }

    /// Enable the legend at a chosen spot (matplotlib's `legend(loc=…)`).
    pub fn legend_loc(&mut self, loc: Loc) -> &mut Self {
        self.legend_loc = loc;
        self.show_legend = true;
        self
    }

    /// Where the legend goes.
    pub(crate) fn legend_location(&self) -> Loc {
        self.legend_loc
    }

    /// Every artist's sample points in pixels on a frame at `box_`: what
    /// [`Loc::Best`] keeps the legend off.
    pub(crate) fn data_points_px(&self, box_: Rect) -> Vec<(f32, f32)> {
        let t = self.transform(box_, self.limits());
        self.artists
            .iter()
            .flat_map(|a| a.sample_points(&t))
            .collect()
    }

    /// Show a matplotlib-style grid at the major tick positions (light grey
    /// solid lines behind the data).
    ///
    /// # Examples
    /// ```
    /// # use oxiplot::Axes;
    /// let mut ax = Axes::new();
    /// ax.plot(&[0.0, 1.0, 2.0], &[0.0, 1.0, 0.5]);
    /// ax.grid();
    /// ```
    pub fn grid(&mut self) -> &mut Self {
        self.style.grid = true;
        self
    }

    /// Also show fainter grid lines at the minor tick positions (implies the
    /// major grid and minor ticks).
    pub fn grid_minor(&mut self) -> &mut Self {
        self.grid_minor = true;
        self.style.grid = true;
        self.style.minor_ticks = true;
        self
    }

    /// Hide the x tick labels and the x-axis label (e.g. on the upper panel of a
    /// shared-x layout). The tick marks themselves stay.
    pub fn hide_xticklabels(&mut self) -> &mut Self {
        self.show_xticklabels = false;
        self
    }

    /// Hide the y tick labels and the y-axis label (e.g. on the non-left columns
    /// of a shared-y grid). The tick marks themselves stay.
    pub fn hide_yticklabels(&mut self) -> &mut Self {
        self.show_yticklabels = false;
        self
    }

    /// Set the text + math fonts (e.g. `FontSet::dejavu()` for the matplotlib
    /// look, or a custom font from `FontSet::from_path`).
    ///
    /// # Examples
    /// ```
    /// # use oxiplot::{Axes, FontSet};
    /// let mut ax = Axes::new();
    /// ax.fonts(FontSet::dejavu());
    /// ```
    pub fn fonts(&mut self, fonts: crate::fonts::FontSet) -> &mut Self {
        self.style.fonts = fonts;
        self
    }

    /// The x range the caller or the data asks for (`xlim`, or a histogram's
    /// edges), if any: what a shared x axis keeps.
    pub(crate) fn declared_xlim(&self) -> Option<(f64, f64)> {
        self.xlim.or(self.data_xlim)
    }

    /// The y range the caller or the data asks for, if any.
    pub(crate) fn declared_ylim(&self) -> Option<(f64, f64)> {
        self.ylim.or(self.data_ylim)
    }

    /// The resolved x-axis limits (for sharing the x-axis across panels).
    #[must_use]
    pub(crate) fn resolved_xlim(&self) -> (f64, f64) {
        let (xmin, xmax, _, _) = self.limits();
        (xmin, xmax)
    }

    /// The resolved y-axis limits (for sharing the y-axis across panels).
    #[must_use]
    pub(crate) fn resolved_ylim(&self) -> (f64, f64) {
        let (_, _, ymin, ymax) = self.limits();
        (ymin, ymax)
    }

    /// Render this single axes as a full figure and save to `path` (`.png`,
    /// `.svg`, or `.pdf`) — the convenient path for a one-panel plot.
    pub fn save(&self, path: impl AsRef<std::path::Path>) -> crate::error::Result<()> {
        self.save_with(path, SaveOpts::default())
    }

    /// Like [`Axes::save`] with explicit options (e.g. a higher DPI for a sharper
    /// PNG, or a transparent background).
    ///
    /// # Examples
    /// ```no_run
    /// use oxiplot::{Axes, SaveOpts};
    /// let mut ax = Axes::new();
    /// ax.plot(&[0.0, 1.0], &[0.0, 1.0]);
    /// ax.save_with("plot.png", SaveOpts::new().dpi(300.0)).unwrap();
    /// ```
    pub fn save_with(
        &self,
        path: impl AsRef<std::path::Path>,
        opts: SaveOpts,
    ) -> crate::error::Result<()> {
        self.figure().save_with(path, opts)
    }

    /// Render to an in-memory PNG (honoring [`SaveOpts`] DPI/transparency) instead
    /// of writing a file.
    pub fn to_png_bytes(&self, opts: SaveOpts) -> crate::error::Result<Vec<u8>> {
        self.figure().to_png_bytes(opts)
    }

    /// Render to an in-memory SVG string.
    #[must_use]
    pub fn to_svg_string(&self) -> String {
        self.figure().to_svg_string()
    }

    /// Render to in-memory PDF bytes.
    #[must_use]
    pub fn to_pdf_bytes(&self) -> Vec<u8> {
        self.figure().to_pdf_bytes()
    }

    /// This axes alone on a figure of its style's size: how it is saved.
    fn figure(&self) -> crate::figure::Figure {
        crate::figure::Figure::with_style(self.style.clone()).with_axes([self.clone()])
    }

    /// Draw the x values as times, with `format` saying how a label reads: a
    /// `strftime` format, and `%F` after it naming the epoch the values count
    /// from, exactly as ROOT's `fTimeFormat` spells it
    /// (`"%H:%M%F2024-01-01 00:00:00"`). The ticks then step in seconds,
    /// minutes, hours or days rather than in decimals.
    ///
    /// Plotting a histogram or graph whose axis is already a time axis sets
    /// this on its own, so this is for data that carries no axis of its own.
    pub fn x_time_format(&mut self, format: &str) -> &mut Self {
        self.x_time = Some(TimeFormat::parse(format));
        self
    }

    /// The `(bottom, top)` y tick labels dropped, for the crate's own tests.
    #[cfg(test)]
    pub(crate) fn ylabel_prune_for_test(&self) -> (bool, bool) {
        self.ylabel_prune
    }

    /// The time format in force, for the crate's own tests.
    #[cfg(test)]
    pub(crate) fn x_time_format_for_test(&self) -> Option<&TimeFormat> {
        self.x_time.as_ref()
    }

    /// Take the time format from plotted data, unless the caller set one.
    fn adopt_time_format(&mut self, format: Option<String>) {
        if let (None, Some(format)) = (&self.x_time, format) {
            self.x_time = Some(TimeFormat::parse(&format));
        }
    }

    /// Plot a 1-D histogram (a `Hist1D`, or any [`Hist1dData`]) as an mplhep step
    /// staircase (the matplotlib `hist` analog).
    pub fn hist(&mut self, h: &impl Hist1dData) -> &mut Self {
        self.hist_with(h, HistOpts::default())
    }

    /// Plot a 1-D histogram with explicit options (histtype, error bars, color,
    /// label).
    ///
    /// # Examples
    /// ```no_run
    /// use oxiplot::{Axes, Binned1D, HistOpts, HistType};
    /// let edges: Vec<f64> = (0..=20).map(|i| 0.5 * i as f64).collect();
    /// let h = Binned1D::from_samples(edges, &[2.5, 3.1, 3.3, 4.8]);
    /// let mut ax = Axes::new();
    /// ax.hist_with(&h, HistOpts::new().histtype(HistType::Step).yerr().label("MC"));
    /// ax.save("h.svg").unwrap();
    /// ```
    pub fn hist_with(&mut self, h: &impl Hist1dData, opts: HistOpts) -> &mut Self {
        self.adopt_time_format(h.x_time_format());
        let edges = h.edges();
        let values = h.values();
        let n = values.len();
        let color = opts.color.unwrap_or_else(|| self.next_color());
        // A band *is* the uncertainty, and an errorbar histogram is data
        // points with their errors (mplhep draws both from `yerr` by default).
        let with_errors = opts.yerr || matches!(opts.histtype, HistType::Band | HistType::Errorbar);
        let errs = if with_errors {
            Some((0..n).map(|i| h.error(i)).collect())
        } else {
            None
        };
        if n > 0 && edges.len() > n {
            self.data_xlim = Some(union_range(self.data_xlim, (edges[0], edges[n])));
        }
        self.y_from_zero = true;
        let width = opts.linewidth_pt.unwrap_or(self.style.line_width_pt);
        let msize = self.style.marker_size_pt * 0.7;
        self.add_artist(Artist::Step(StepArtist {
            edges,
            values,
            errs,
            histtype: opts.histtype,
            color,
            fill_color: opts.fill_color,
            width_pt: width,
            marker: Marker::Circle,
            marker_size_pt: msize,
            label: opts.label,
        }));
        self
    }

    /// Plot points with their error bars: a `Graph` (any error variant), or any
    /// [`PointData`].
    ///
    /// # Examples
    /// ```no_run
    /// use oxiplot::{Axes, Points};
    /// let g = Points::new(vec![1.0, 2.0], vec![3.0, 4.0])?
    ///     .with_x_errors(vec![0.1, 0.1])?
    ///     .with_y_errors(vec![0.2, 0.3])?;
    /// let mut ax = Axes::new();
    /// ax.errorbar(&g);
    /// ax.save("g.png")?;
    /// # Ok::<(), oxiplot::Error>(())
    /// ```
    pub fn errorbar(&mut self, g: &impl PointData) -> &mut Self {
        self.errorbar_with(g, ErrorbarOpts::default())
    }

    /// Plot points with explicit options.
    pub fn errorbar_with(&mut self, g: &impl PointData, opts: ErrorbarOpts) -> &mut Self {
        self.adopt_time_format(g.x_time_format());
        let xs = g.xs();
        let ys = g.ys();
        let (xerr, yerr) = (g.x_errors(), g.y_errors());
        let color = opts.color.unwrap_or_else(|| self.next_color());
        self.add_artist(Artist::Errorbar(ErrorbarArtist {
            xs,
            ys,
            xerr,
            yerr,
            color,
            marker: opts.marker,
            marker_size_pt: opts
                .marker_size_pt
                .unwrap_or(self.style.marker_size_pt * 0.8),
            elinewidth_pt: 1.0,
            capsize_pt: opts.capsize_pt,
            line_width_pt: opts.line.then_some(self.style.line_width_pt),
            label: opts.label,
        }));
        self
    }

    /// Plot a profile (a `Profile1D`, or any [`Hist1dData`]) as data points with y
    /// error bars at bin centers.
    ///
    /// # Examples
    /// ```no_run
    /// use oxiplot::{Axes, Binned1D};
    /// // Per-bin means and their errors, as a profile holds them.
    /// let tp = Binned1D::new(vec![0.0, 1.0, 2.0, 3.0], vec![1.2, 1.9, 3.1])?
    ///     .with_errors(vec![0.1, 0.2, 0.15])?;
    /// let mut ax = Axes::new();
    /// ax.profile(&tp);
    /// ax.save("profile.png")?;
    /// # Ok::<(), oxiplot::Error>(())
    /// ```
    pub fn profile(&mut self, tp: &impl Hist1dData) -> &mut Self {
        self.profile_with(tp, ErrorbarOpts::default())
    }

    /// Plot a profile with explicit options (color, marker, legend label).
    ///
    /// # Examples
    /// ```
    /// use oxiplot::{Axes, Binned1D, Color, ErrorbarOpts};
    /// let tp = Binned1D::new(vec![0.0, 1.0, 2.0], vec![1.2, 1.9])?
    ///     .with_errors(vec![0.1, 0.2])?;
    /// let mut ax = Axes::new();
    /// ax.profile_with(&tp, ErrorbarOpts::new().color(Color::BLACK).label("mean"));
    /// ax.legend();
    /// # Ok::<(), oxiplot::Error>(())
    /// ```
    pub fn profile_with(&mut self, tp: &impl Hist1dData, opts: ErrorbarOpts) -> &mut Self {
        self.adopt_time_format(tp.x_time_format());
        let edges = tp.edges();
        let vals = tp.values();
        let n = vals.len();
        if edges.len() < n + 1 {
            return self;
        }
        let xs: Vec<f64> = (0..n).map(|i| 0.5 * (edges[i] + edges[i + 1])).collect();
        let yerr: Vec<f64> = (0..n).map(|i| tp.error(i)).collect();
        let color = opts.color.unwrap_or_else(|| self.next_color());
        self.add_artist(Artist::Errorbar(ErrorbarArtist {
            xs,
            ys: vals,
            xerr: None,
            yerr: Some((yerr.clone(), yerr)),
            color,
            marker: opts.marker,
            marker_size_pt: opts
                .marker_size_pt
                .unwrap_or(self.style.marker_size_pt * 0.8),
            elinewidth_pt: 1.0,
            capsize_pt: opts.capsize_pt,
            line_width_pt: opts.line.then_some(self.style.line_width_pt),
            label: opts.label,
        }));
        self
    }

    /// Plot a 2-D histogram (a `Hist2D`, or any [`Hist2dData`]) as a color mesh
    /// with a colorbar (matplotlib `pcolormesh`).
    pub fn hist2d(&mut self, h: &impl Hist2dData) -> &mut Self {
        self.hist2d_with(h, Hist2dOpts::default())
    }

    /// Plot a 2-D histogram with explicit options (colormap, value range,
    /// colorbar label).
    ///
    /// # Examples
    /// ```no_run
    /// use oxiplot::{Axes, Binned2D, Colormap, Hist2dOpts};
    /// let edges: Vec<f64> = (0..=10).map(|i| 0.1 * i as f64).collect();
    /// let grid = vec![vec![1.0; 10]; 10];
    /// let h = Binned2D::new(edges.clone(), edges, grid)?;
    /// let mut ax = Axes::new();
    /// ax.hist2d_with(&h, Hist2dOpts::new().cmap(Colormap::Viridis).label("entries"));
    /// ax.save("h2.png")?;
    /// # Ok::<(), oxiplot::Error>(())
    /// ```
    pub fn hist2d_with(&mut self, h: &impl Hist2dData, opts: Hist2dOpts) -> &mut Self {
        let xedges = h.x_edges();
        let yedges = h.y_edges();
        let values = h.grid();
        // Autoscale over the bins that actually hold data. Empty bins (content
        // exactly 0) are not drawn — they show the page background, like ROOT's
        // COLZ — so they must not anchor the color scale at 0 either.
        let (mut dmin, mut dmax) = (f64::INFINITY, f64::NEG_INFINITY);
        let mut dmin_pos = f64::INFINITY; // smallest strictly-positive value, for log
        for row in &values {
            for &v in row {
                if v != 0.0 && v.is_finite() {
                    dmin = dmin.min(v);
                    dmax = dmax.max(v);
                }
                if v > 0.0 {
                    dmin_pos = dmin_pos.min(v);
                }
            }
        }
        if !dmin.is_finite() {
            // No filled bins at all.
            dmin = 0.0;
            dmax = 1.0;
        }
        // A log scale needs a strictly-positive floor; fall back to the smallest
        // positive bin (or 1) when the user did not set an explicit vmin.
        let default_min = if matches!(opts.norm, crate::norm::Norm::Log) {
            if dmin_pos.is_finite() { dmin_pos } else { 1.0 }
        } else {
            dmin
        };
        let vmin = opts.vmin.unwrap_or(default_min);
        let vmax = opts.vmax.unwrap_or(dmax);
        let nx = values.len();
        let ny = values.first().map_or(0, Vec::len);
        if nx > 0 && ny > 0 && xedges.len() > nx && yedges.len() > ny {
            self.data_xlim = Some(union_range(self.data_xlim, (xedges[0], xedges[nx])));
            self.data_ylim = Some(union_range(self.data_ylim, (yedges[0], yedges[ny])));
        }
        self.colorbar = Some(ColorbarSpec {
            vmin,
            vmax,
            cmap: opts.cmap,
            norm: opts.norm,
            label: opts.label,
        });
        self.add_artist(Artist::Mesh(MeshArtist {
            xedges,
            yedges,
            values,
            cmap: opts.cmap,
            vmin,
            vmax,
            norm: opts.norm,
        }));
        self
    }

    /// Plot a connected line through `(x, y)` points (matplotlib `plot`).
    ///
    /// # Errors
    /// [`Error::LengthMismatch`](crate::Error::LengthMismatch) if `ys` is not as
    /// long as `xs` (matplotlib raises a `ValueError`); nothing is drawn.
    pub fn plot(&mut self, xs: &[f64], ys: &[f64]) -> crate::Result<&mut Self> {
        self.plot_with(xs, ys, CurveOpts::default())
    }

    /// Plot a connected line with explicit options (color, legend label, line
    /// width, dash pattern). [`CurveOpts::samples`] does not apply: the line
    /// goes through the given points.
    ///
    /// # Errors
    /// [`Error::LengthMismatch`](crate::Error::LengthMismatch) if `ys` is not as
    /// long as `xs`; nothing is drawn.
    ///
    /// # Examples
    /// ```
    /// use oxiplot::{Axes, CurveOpts};
    /// let mut ax = Axes::new();
    /// ax.plot_with(&[0.0, 1.0, 2.0], &[0.0, 1.0, 4.0], CurveOpts::new().label("x²"))?;
    /// ax.legend();
    /// # Ok::<(), oxiplot::Error>(())
    /// ```
    pub fn plot_with(
        &mut self,
        xs: &[f64],
        ys: &[f64],
        opts: CurveOpts,
    ) -> crate::Result<&mut Self> {
        if ys.len() != xs.len() {
            return Err(crate::Error::LengthMismatch {
                what: "plot ys",
                expected: xs.len(),
                found: ys.len(),
            });
        }
        let color = opts.color.unwrap_or_else(|| self.next_color());
        self.add_artist(Artist::Line(LineArtist {
            xs: xs.to_vec(),
            ys: ys.to_vec(),
            color,
            width_pt: opts.linewidth_pt.unwrap_or(self.style.line_width_pt),
            dash: opts.dash,
            marker: Marker::None,
            marker_size_pt: self.style.marker_size_pt,
            label: opts.label,
        }));
        Ok(self)
    }

    /// Plot a function `f` sampled over `range` as a smooth curve — e.g. to
    /// overlay a fitted model on a histogram.
    ///
    /// # Examples
    /// ```
    /// # use oxiplot::Axes;
    /// let mut ax = Axes::new();
    /// // Overlay a Gaussian curve over x ∈ [0, 10].
    /// ax.function(|x| 1000.0 * (-(x - 5.0_f64).powi(2) / 2.0).exp(), 0.0..10.0);
    /// ```
    pub fn function<F: Fn(f64) -> f64>(&mut self, f: F, range: Range<f64>) -> &mut Self {
        self.function_with(f, range, CurveOpts::default())
    }

    /// Plot a function with explicit options (color, label, line width, samples).
    pub fn function_with<F: Fn(f64) -> f64>(
        &mut self,
        f: F,
        range: Range<f64>,
        opts: CurveOpts,
    ) -> &mut Self {
        let n = opts.samples.max(2);
        let (x0, x1) = if range.start <= range.end {
            (range.start, range.end)
        } else {
            (range.end, range.start)
        };
        let xs: Vec<f64> = (0..n)
            .map(|i| x0 + (x1 - x0) * i as f64 / (n - 1) as f64)
            .collect();
        let ys: Vec<f64> = xs.iter().map(|&x| f(x)).collect();
        let color = opts.color.unwrap_or_else(|| self.next_color());
        self.add_artist(Artist::Line(LineArtist {
            xs,
            ys,
            color,
            width_pt: opts.linewidth_pt.unwrap_or(self.style.line_width_pt),
            dash: opts.dash,
            marker: Marker::None,
            marker_size_pt: self.style.marker_size_pt,
            label: opts.label,
        }));
        self
    }

    /// Overlay a fitted [`oxifit::Model`] curve over `range` (the `fit`
    /// feature). The model is evaluated with its current parameters, so fit it
    /// first (`let r = h.fit(&model)`, then pass `model.with_params(r.params)`).
    #[cfg(feature = "fit")]
    pub fn model(&mut self, model: &oxifit::Model, range: Range<f64>) -> &mut Self {
        self.function_with(|x| model.eval(x), range, CurveOpts::default())
    }

    /// Overlay a fitted [`oxifit::Model`] curve with explicit options.
    #[cfg(feature = "fit")]
    pub fn model_with(
        &mut self,
        model: &oxifit::Model,
        range: Range<f64>,
        opts: CurveOpts,
    ) -> &mut Self {
        self.function_with(|x| model.eval(x), range, opts)
    }

    /// Draw a ROOT-style fit statistics box (the analog of `TPaveStats` with
    /// `gStyle->SetOptFit`): the function name, the χ²/ndf of the fit, and every
    /// fitted parameter with its uncertainty. The parameter *names* come from
    /// `model`, the *values* and errors from `result`, so pass the model you
    /// fitted and the [`FitResult`](oxifit::FitResult) it produced.
    ///
    /// Anchored top-right by default; when a legend is also shown there, the box
    /// is stacked beneath it. Use [`fit_stats_with`](Axes::fit_stats_with) to
    /// pick the corner or change what is listed.
    ///
    /// # Examples
    /// ```no_run
    /// # use oxiplot::Axes;
    /// # use oxifit::{FitData, FitExt, Model};
    /// # fn demo(data: &impl FitData) {
    /// let model = Model::gaussian("gaus");
    /// let result = data.fit(&model);
    /// let fitted = model.with_params(result.params.clone());
    /// let mut ax = Axes::new();
    /// ax.model(&fitted, 0.0..10.0);
    /// ax.fit_stats(&fitted, &result); // function name + χ²/ndf + parameters
    /// # }
    /// ```
    #[cfg(feature = "fit")]
    pub fn fit_stats(&mut self, model: &oxifit::Model, result: &oxifit::FitResult) -> &mut Self {
        self.fit_stats_with(model, result, crate::statbox::StatBox::new())
    }

    /// Draw a fit statistics box with explicit [`StatBox`](crate::StatBox)
    /// options (corner, which lines to show, significant figures, header text).
    #[cfg(feature = "fit")]
    pub fn fit_stats_with(
        &mut self,
        model: &oxifit::Model,
        result: &oxifit::FitResult,
        opts: crate::statbox::StatBox,
    ) -> &mut Self {
        self.stats = Some(crate::statbox::build(model, result, &opts));
        self
    }

    /// Resolve the data limits: the caller's limits first, then the data's
    /// own (a histogram's outer edges), then a padded autoscale over every
    /// artist. Always finite with a nonzero span, whatever the data.
    fn limits(&self) -> (f64, f64, f64, f64) {
        let auto = self
            .artists
            .iter()
            .filter_map(Artist::bounds)
            .reduce(Bounds::union);
        let m = self.style.margin as f64;

        let (xmin, xmax) = match self.xscale {
            Scale::Log => log_range(
                self.xlim.or(self.data_xlim),
                auto.map(|b| (b.xpos, b.xmax)),
                m,
            ),
            _ => sane_range_or(self.xlim.or(self.data_xlim).unwrap_or_else(|| match auto {
                Some(b) => padded(b.xmin, b.xmax, m),
                None => (0.0, 1.0),
            })),
        };
        let (ymin, ymax) = match self.yscale {
            // No sticky zero on a log axis: it cannot show 0.
            Scale::Log => log_range(
                self.ylim.or(self.data_ylim),
                auto.map(|b| (b.ypos, b.ymax)),
                m,
            ),
            _ => sane_range_or(self.ylim.or(self.data_ylim).unwrap_or_else(|| match auto {
                // Histograms stand on y = 0 (a sticky edge, as in matplotlib):
                // pad only the side(s) away from zero, so negative bins (a
                // weighted or subtracted histogram) stay in view. Only a
                // negative *value* takes the range below 0: an error bar
                // reaching below an empty bin's count does not.
                Some(b) if self.y_from_zero => {
                    let negative = self
                        .artists
                        .iter()
                        .filter_map(Artist::value_ymin)
                        .any(|v| v < 0.0);
                    let lo = if negative { b.ymin.min(0.0) } else { 0.0 };
                    let (lo, hi) = (lo, b.ymax.max(0.0));
                    let pad = (hi - lo).max(1.0) * m;
                    (
                        if lo < 0.0 { lo - pad } else { 0.0 },
                        if hi > 0.0 || lo == 0.0 { hi + pad } else { 0.0 },
                    )
                }
                Some(b) => padded(b.ymin, b.ymax, m),
                None => (0.0, 1.0),
            })),
        };
        (xmin, xmax, ymin, ymax)
    }

    /// The data → pixel map for a frame at `box_` with these limits.
    fn transform(&self, box_: Rect, (xmin, xmax, ymin, ymax): (f64, f64, f64, f64)) -> Transform {
        Transform::new(box_, xmin, xmax, ymin, ymax).with_scales(self.xscale, self.yscale)
    }

    /// Minor tick positions between `majors` on an axis of `scale`.
    fn minor_ticks(lo: f64, hi: f64, majors: &[f64], scale: Scale) -> Vec<f64> {
        match scale {
            Scale::Log => ticker::log_minor_ticks(lo, hi),
            _ => ticker::minor_ticks(lo, hi, majors, 5),
        }
    }

    /// The width of a tick label as drawn (`$…$` labels are typeset).
    fn tick_label_width(&self, label: &str) -> f32 {
        let s = &self.style;
        crate::mathtext::label_extents(&s.fonts, label, s.px(s.tick_label_size_pt)).0
    }

    /// The height of the x tick label row: the tallest label, and at least a
    /// digit's height.
    fn xtick_row_height(&self, labels: &[String]) -> f32 {
        let s = &self.style;
        let tlab = s.px(s.tick_label_size_pt);
        labels
            .iter()
            .map(|l| {
                let (_, a, d) = crate::mathtext::label_extents(&s.fonts, l, tlab);
                a + d
            })
            .fold(
                text::measure(&s.fonts, "0", tlab, FontStyle::Regular).height(),
                f32::max,
            )
    }

    /// Build the draw groups for this axes alone on a `fig_w × fig_h` figure,
    /// laid out as [`Axes::save`] does.
    #[cfg(test)]
    pub(crate) fn render(&self, fig_w: u32, fig_h: u32) -> Vec<DrawGroup> {
        crate::figure::Figure::with_style(self.style.clone())
            .with_axes([self.clone()])
            .groups_at(fig_w, fig_h)
    }

    /// The tick positions and labels on a frame at `box_` (their number
    /// follows the frame's size).
    fn tick_layout(&self, box_: Rect) -> TickLayout {
        let (xmin, xmax, ymin, ymax) = self.limits();
        let xtarget = ((box_.w / 70.0).round() as usize).clamp(3, 11);
        let ytarget = ((box_.h / 50.0).round() as usize).clamp(3, 9);
        // A time axis steps in minutes, hours or days, not in "nice" decimals.
        let (xticks, xlabels) = match (&self.x_time, self.xscale) {
            (Some(time), _) => {
                let ticks = crate::timeaxis::time_ticks(xmin, xmax, xtarget);
                let labels = ticks.iter().map(|&v| time.label(v)).collect();
                (ticks, labels)
            }
            (None, Scale::Log) => {
                let ticks = ticker::log_ticks(xmin, xmax, xtarget);
                let labels = ticker::format_log_ticks(&ticks);
                (ticks, labels)
            }
            (None, _) => {
                let ticks = ticker::ticks(xmin, xmax, xtarget);
                let labels = ticker::format_ticks(&ticks, ticker::nice_step(xmin, xmax, xtarget));
                (ticks, labels)
            }
        };
        let (yticks, ylabels) = match self.yscale {
            Scale::Log => {
                let ticks = ticker::log_ticks(ymin, ymax, ytarget);
                let labels = ticker::format_log_ticks(&ticks);
                (ticks, labels)
            }
            _ => {
                let ticks = ticker::ticks(ymin, ymax, ytarget);
                let labels = ticker::format_ticks(&ticks, ticker::nice_step(ymin, ymax, ytarget));
                (ticks, labels)
            }
        };
        TickLayout {
            limits: (xmin, xmax, ymin, ymax),
            xticks,
            yticks,
            xlabels,
            ylabels,
        }
    }

    /// The frame left of a colorbar, and the colorbar's own rectangle, when
    /// the axes has one: the bar and its tick labels and label take the right
    /// of `box_`, sized to what they hold.
    fn split_colorbar(&self, box_: Rect) -> (Rect, Option<Rect>) {
        let Some(spec) = &self.colorbar else {
            return (box_, None);
        };
        let s = &self.style;
        let (cb_w, gap) = (s.px(14.0), s.px(12.0));
        let outer = crate::colorbar::outer_width(spec, s, box_.h);
        let mut frame = box_;
        frame.w = (frame.w - gap - cb_w - outer).max(box_.w * 0.3);
        let bar = Rect::new(frame.right() + gap, frame.y, cb_w, frame.h);
        (frame, Some(bar))
    }

    /// The length of the major ticks outside and inside the frame.
    fn tick_lengths(&self) -> (f32, f32) {
        let major = self.style.px(self.style.tick_major_len_pt);
        match self.style.tick_dir {
            TickDir::Out => (major, 0.0),
            TickDir::In => (0.0, major),
            TickDir::InOut => (major, major),
        }
    }

    /// The height of the experiment-label row above the frame (`hep_label`,
    /// `hep_rhs`), or 0 when there is none.
    fn hep_row_height(&self) -> f32 {
        let s = &self.style;
        let base = s.px(s.font_size_pt);
        let mut ascent = 0.0_f32;
        if let Some((exp, _)) = &self.hep_label {
            ascent = text::measure(&s.fonts, exp, base * 1.3, FontStyle::Bold).ascent;
        }
        if let Some(rhs) = &self.hep_rhs {
            ascent = ascent.max(crate::mathtext::label_extents(&s.fonts, rhs, base).1);
        }
        if ascent > 0.0 {
            s.px(HEP_LABEL_PAD_PT) + ascent
        } else {
            0.0
        }
    }

    /// The width taken left of the frame by the y tick marks and labels.
    fn ytick_label_area(&self, ticks: &TickLayout) -> f32 {
        let s = &self.style;
        if !self.show_yticklabels {
            return 0.0;
        }
        let widest = ticks
            .ylabels
            .iter()
            .map(|l| self.tick_label_width(l))
            .fold(0.0_f32, f32::max);
        self.tick_lengths().0 + s.px(s.tick_pad_pt) + widest
    }

    /// How far the axes' decorations (tick labels, axis labels, title,
    /// experiment label) reach outside a frame at `box_`, in pixels: what a
    /// figure must leave around the frame so nothing overlaps.
    pub(crate) fn insets(&self, box_: Rect) -> Insets {
        let s = &self.style;
        let (frame, _) = self.split_colorbar(box_);
        let ticks = self.tick_layout(frame);
        let (out_len, _) = self.tick_lengths();
        let labsize = s.px(s.label_size_pt);
        let label_height = |l: &str| {
            let (_, a, d) = crate::mathtext::label_extents(&s.fonts, l, labsize);
            a + d
        };

        let ytick_area = self.ytick_label_area(&ticks);
        let mut left = ytick_area;
        if let (Some(yl), true) = (&self.ylabel, self.show_yticklabels) {
            left += s.px(AXIS_LABEL_PAD_PT) + label_height(yl);
        }

        let mut bottom = 0.0;
        let mut right = 0.0_f32;
        if self.show_xticklabels && !ticks.xticks.is_empty() {
            let tick_h = self.xtick_row_height(&ticks.xlabels);
            bottom = out_len + s.px(s.tick_pad_pt) + tick_h;
            if let Some(xl) = &self.xlabel {
                bottom += s.px(AXIS_LABEL_PAD_PT) + label_height(xl);
            }
            // The outer x tick labels are centred on their ticks, so the ones
            // at the ends of the axis hang over the frame by half their width.
            let (xmin, xmax, ymin, ymax) = ticks.limits;
            let t = self.transform(frame, (xmin, xmax, ymin, ymax));
            for (&v, l) in ticks.xticks.iter().zip(&ticks.xlabels) {
                let half = self.tick_label_width(l) / 2.0;
                left = left.max(frame.x - (t.x(v) - half));
                right = right.max(t.x(v) + half - box_.right());
            }
        }

        let hep = self.hep_row_height();
        let title = self.title.as_ref().map_or(0.0, |tt| {
            let (_, a, d) = crate::mathtext::label_extents(&s.fonts, tt, s.px(s.title_size_pt));
            s.px(TITLE_PAD_PT) + a + d
        });
        Insets {
            left,
            right,
            // A title sits above the experiment-label row, not on it.
            top: hep + title,
            bottom,
            ytick_area,
        }
    }

    /// Build the draw groups with the data area placed at an explicit pixel box
    /// (used by [`crate::figure::Figure`] for grid/ratio layouts).
    pub(crate) fn render_at(&self, mut box_: Rect) -> Vec<DrawGroup> {
        let s = &self.style;
        // A figure leaves room for the title; should it still not fit (a
        // title taller than the figure allows), lower the frame just enough
        // to keep it in.
        if self.title.is_some() {
            let top = box_.y - self.insets(box_).top;
            let edge = s.px(TITLE_EDGE_PT);
            if top < edge {
                // Never take more than half the frame, however tall the title.
                let shift = (edge - top).min(box_.h / 2.0);
                box_.y += shift;
                box_.h -= shift;
            }
        }
        // Reserve space on the right for a colorbar, if present.
        let (frame, cb_rect) = self.split_colorbar(box_);
        let box_ = frame;
        let TickLayout {
            limits: (xmin, xmax, ymin, ymax),
            xticks,
            yticks,
            xlabels,
            ylabels,
        } = self.tick_layout(box_);
        let t = self.transform(box_, (xmin, xmax, ymin, ymax));

        let mut grid = DrawGroup::new(Some(box_));
        let mut data = DrawGroup::new(Some(box_));
        let mut axis = DrawGroup::new(None);

        let fg = s.fg_color;
        let lw = s.px(s.axes_linewidth_pt);
        let major = s.px(s.tick_major_len_pt);
        let minor = s.px(s.tick_minor_len_pt);
        let pad = s.px(s.tick_pad_pt);
        let tlab = s.px(s.tick_label_size_pt);

        // Grid (drawn below data when enabled).
        if s.grid {
            let gstroke = Stroke::line(s.grid_color, s.px(s.grid_width_pt));
            for &xv in &xticks {
                let px = t.x(xv);
                grid.push(DrawCommand::Line {
                    p0: (px, box_.y),
                    p1: (px, box_.bottom()),
                    stroke: gstroke.clone(),
                });
            }
            for &yv in &yticks {
                let py = t.y(yv);
                grid.push(DrawCommand::Line {
                    p0: (box_.x, py),
                    p1: (box_.right(), py),
                    stroke: gstroke.clone(),
                });
            }
            // Fainter grid lines at the minor ticks.
            if self.grid_minor {
                let gminor =
                    Stroke::line(s.grid_color.with_alpha(0.5), s.px(s.grid_width_pt * 0.7));
                for &xv in &Self::minor_ticks(xmin, xmax, &xticks, self.xscale) {
                    let px = t.x(xv);
                    grid.push(DrawCommand::Line {
                        p0: (px, box_.y),
                        p1: (px, box_.bottom()),
                        stroke: gminor.clone(),
                    });
                }
                for &yv in &Self::minor_ticks(ymin, ymax, &yticks, self.yscale) {
                    let py = t.y(yv);
                    grid.push(DrawCommand::Line {
                        p0: (box_.x, py),
                        p1: (box_.right(), py),
                        stroke: gminor.clone(),
                    });
                }
            }
        }

        // Data artists.
        for a in &self.artists {
            a.draw(&t, s, &mut data);
        }

        // Frame: all four spines (matplotlib draws the full rectangle).
        axis.push(DrawCommand::Rect {
            rect: box_,
            fill: None,
            stroke: Some(Stroke::line(fg, lw)),
        });

        // Tick mark deltas for a given direction (outside, inside).
        let (out_len, in_len) = match s.tick_dir {
            TickDir::Out => (major, 0.0),
            TickDir::In => (0.0, major),
            TickDir::InOut => (major, major),
        };
        let (out_minor, in_minor) = match s.tick_dir {
            TickDir::Out => (minor, 0.0),
            TickDir::In => (0.0, minor),
            TickDir::InOut => (minor, minor),
        };
        let tickstroke = Stroke::line(fg, s.px(s.tick_major_width_pt));

        // X ticks (bottom and optionally top).
        for &xv in &xticks {
            let px = t.x(xv);
            if s.tick_sides.bottom {
                axis.push(DrawCommand::Line {
                    p0: (px, box_.bottom() - in_len),
                    p1: (px, box_.bottom() + out_len),
                    stroke: tickstroke.clone(),
                });
            }
            if s.tick_sides.top {
                axis.push(DrawCommand::Line {
                    p0: (px, box_.y + in_len),
                    p1: (px, box_.y - out_len),
                    stroke: tickstroke.clone(),
                });
            }
        }
        // Y ticks (left and optionally right).
        for &yv in &yticks {
            let py = t.y(yv);
            if s.tick_sides.left {
                axis.push(DrawCommand::Line {
                    p0: (box_.x + in_len, py),
                    p1: (box_.x - out_len, py),
                    stroke: tickstroke.clone(),
                });
            }
            if s.tick_sides.right {
                axis.push(DrawCommand::Line {
                    p0: (box_.right() - in_len, py),
                    p1: (box_.right() + out_len, py),
                    stroke: tickstroke.clone(),
                });
            }
        }
        // Minor ticks, drawn a touch thinner than major (matplotlib uses a 0.6 pt
        // minor width against a 0.8 pt major).
        if s.minor_ticks {
            let minorstroke = Stroke::line(fg, s.px(s.tick_major_width_pt * 0.75));
            let xmin_t = Self::minor_ticks(xmin, xmax, &xticks, self.xscale);
            let ymin_t = Self::minor_ticks(ymin, ymax, &yticks, self.yscale);
            for &xv in &xmin_t {
                let px = t.x(xv);
                if s.tick_sides.bottom {
                    axis.push(DrawCommand::Line {
                        p0: (px, box_.bottom() - in_minor),
                        p1: (px, box_.bottom() + out_minor),
                        stroke: minorstroke.clone(),
                    });
                }
                if s.tick_sides.top {
                    axis.push(DrawCommand::Line {
                        p0: (px, box_.y + in_minor),
                        p1: (px, box_.y - out_minor),
                        stroke: minorstroke.clone(),
                    });
                }
            }
            for &yv in &ymin_t {
                let py = t.y(yv);
                if s.tick_sides.left {
                    axis.push(DrawCommand::Line {
                        p0: (box_.x + in_minor, py),
                        p1: (box_.x - out_minor, py),
                        stroke: minorstroke.clone(),
                    });
                }
                if s.tick_sides.right {
                    axis.push(DrawCommand::Line {
                        p0: (box_.right() - in_minor, py),
                        p1: (box_.right() + out_minor, py),
                        stroke: minorstroke.clone(),
                    });
                }
            }
        }

        // X tick labels (below the bottom spine); hidden on shared-x upper panels.
        let label_y = box_.bottom() + out_len.max(0.0) + pad;
        if self.show_xticklabels {
            for (&xv, lab) in xticks.iter().zip(&xlabels) {
                let px = t.x(xv);
                // Through the label layout: log-axis labels are `$10^{k}$`.
                crate::mathtext::layout_label(
                    &mut axis,
                    &s.fonts,
                    lab,
                    px,
                    label_y,
                    tlab,
                    fg,
                    HAlign::Center,
                    VAlign::Top,
                    0.0,
                );
            }
        }
        // Y tick labels (left of the left spine); track max width for the ylabel.
        let mut max_ylabel_w = 0.0_f32;
        let label_x = box_.x - out_len.max(0.0) - pad;
        let nyt = yticks.len();
        if self.show_yticklabels {
            for (i, (&yv, lab)) in yticks.iter().zip(&ylabels).enumerate() {
                let py = t.y(yv);
                max_ylabel_w = max_ylabel_w.max(self.tick_label_width(lab));
                // yticks are ascending: index 0 is the bottom-most, last is the top.
                if (self.ylabel_prune.0 && i == 0) || (self.ylabel_prune.1 && i + 1 == nyt) {
                    continue;
                }
                crate::mathtext::layout_label(
                    &mut axis,
                    &s.fonts,
                    lab,
                    label_x,
                    py,
                    tlab,
                    fg,
                    HAlign::Right,
                    VAlign::Middle,
                    0.0,
                );
            }
        }

        // Axis labels.
        let labsize = s.px(s.label_size_pt);
        if let (Some(xl), true) = (&self.xlabel, self.show_xticklabels) {
            let tick_h = self.xtick_row_height(&xlabels);
            let y = label_y + tick_h + s.px(AXIS_LABEL_PAD_PT);
            crate::mathtext::layout_label(
                &mut axis,
                &s.fonts,
                xl,
                box_.x + box_.w / 2.0,
                y,
                labsize,
                fg,
                HAlign::Center,
                VAlign::Top,
                0.0,
            );
        }
        if let (Some(yl), true) = (&self.ylabel, self.show_yticklabels) {
            // In a figure the labels of a column line up: they sit beyond the
            // widest tick labels of the column, not just of this axes.
            let own = out_len.max(0.0) + pad + max_ylabel_w;
            let area = self.ytick_area.map_or(own, |a| a.max(own));
            let x = box_.x - area - s.px(AXIS_LABEL_PAD_PT);
            // Rotated to read upwards, the label's bottom (descenders) faces
            // the tick labels: anchor it there so nothing crosses into them.
            crate::mathtext::layout_label(
                &mut axis,
                &s.fonts,
                yl,
                x,
                box_.y + box_.h / 2.0,
                labsize,
                fg,
                HAlign::Center,
                VAlign::Bottom,
                -90.0,
            );
        }
        if let Some(tt) = &self.title {
            // Above the experiment-label row, when there is one.
            let y = box_.y - self.hep_row_height() - s.px(TITLE_PAD_PT);
            crate::mathtext::layout_label(
                &mut axis,
                &s.fonts,
                tt,
                box_.x + box_.w / 2.0,
                y,
                s.px(s.title_size_pt),
                fg,
                HAlign::Center,
                VAlign::Bottom,
                0.0,
            );
        }

        // mplhep experiment label: a bold experiment name + italic status above
        // the top-left of the frame, the luminosity / energy above the top-right.
        if let Some((exp, status)) = &self.hep_label {
            let base = s.px(s.font_size_pt);
            let baseline = box_.y - s.px(HEP_LABEL_PAD_PT);
            let exp_size = base * 1.3;
            let exp_w = text::measure(&s.fonts, exp, exp_size, FontStyle::Bold).width;
            axis.cmds.extend(text::layout(
                &s.fonts,
                exp,
                box_.x,
                baseline,
                exp_size,
                FontStyle::Bold,
                fg,
                HAlign::Left,
                VAlign::Baseline,
                0.0,
            ));
            if !status.is_empty() {
                axis.cmds.extend(text::layout(
                    &s.fonts,
                    status,
                    box_.x + exp_w + base * 0.4,
                    baseline,
                    base * 1.05,
                    FontStyle::Italic,
                    fg,
                    HAlign::Left,
                    VAlign::Baseline,
                    0.0,
                ));
            }
        }
        if let Some(rhs) = &self.hep_rhs {
            crate::mathtext::layout_label(
                &mut axis,
                &s.fonts,
                rhs,
                box_.right(),
                box_.y - s.px(HEP_LABEL_PAD_PT),
                s.px(s.font_size_pt),
                fg,
                HAlign::Right,
                VAlign::Baseline,
                0.0,
            );
        }

        // Legend.
        if self.show_legend {
            crate::legend::draw_legend(&mut axis, self, box_);
        }

        // Fit statistics box (ROOT's TPaveStats). Stacked beneath the legend when
        // both are anchored top-right.
        #[cfg(feature = "fit")]
        if let Some(data) = &self.stats {
            let legend_rect = if self.show_legend {
                crate::legend::legend_rect(self, box_)
            } else {
                None
            };
            crate::statbox::draw_stats(&mut axis, self, box_, data, legend_rect);
        }

        // Colorbar.
        if let (Some(spec), Some(cbr)) = (&self.colorbar, cb_rect) {
            crate::colorbar::draw_colorbar(&mut axis, cbr, spec, s);
        }

        vec![grid, data, axis]
    }
}

impl Default for Axes {
    fn default() -> Self {
        Axes::new()
    }
}

/// The smallest range holding both `a` (if any) and `b`.
fn union_range(a: Option<(f64, f64)>, b: (f64, f64)) -> (f64, f64) {
    match a {
        Some((lo, hi)) => (lo.min(b.0), hi.max(b.1)),
        None => b,
    }
}

/// `[lo, hi]` widened by `margin` of its span on each side (±0.5 when the
/// span is zero, e.g. a single point).
fn padded(lo: f64, hi: f64, margin: f64) -> (f64, f64) {
    let pad = (hi - lo) * margin;
    let pad = if pad.abs() < f64::EPSILON { 0.5 } else { pad };
    (lo - pad, hi + pad)
}

/// [`sane_range`] of a `(lo, hi)` pair.
fn sane_range_or((lo, hi): (f64, f64)) -> (f64, f64) {
    sane_range(lo, hi)
}

/// The range of a log axis: the requested one when both ends are > 0,
/// otherwise the data's positive span (`(smallest > 0, largest)`) padded by
/// `margin` of its decades on each side; `[1, 10]` with nothing to show.
/// A requested end ≤ 0 (a histogram's edge at 0, say) is replaced by the
/// data's. The order of a requested range is kept.
fn log_range(requested: Option<(f64, f64)>, data: Option<(f64, f64)>, margin: f64) -> (f64, f64) {
    let ok = |v: f64| v > 0.0 && v.is_finite();
    let auto = match data {
        Some((pos, max)) if ok(pos) && ok(max) => {
            let (a, b) = (pos.log10(), max.log10());
            let pad = if b - a < 1e-9 { 0.5 } else { (b - a) * margin };
            (10f64.powf(a - pad), 10f64.powf(b + pad))
        }
        _ => (1.0, 10.0),
    };
    let (lo, hi) = match requested {
        Some((lo, hi)) => (
            if ok(lo) { lo } else { auto.0 },
            if ok(hi) { hi } else { auto.1 },
        ),
        None => auto,
    };
    if lo == hi {
        (lo / 10f64.sqrt(), hi * 10f64.sqrt())
    } else {
        (lo, hi)
    }
}

/// A drawable axis range from `(lo, hi)`: a non-finite end, or a span that
/// overflows `f64`, falls back to `[0, 1]` (anchored on the finite end when
/// there is one); an empty range is widened around its value. The order is
/// kept, so an inverted range (`xlim(10.0..0.0)`) still flips the axis.
fn sane_range(lo: f64, hi: f64) -> (f64, f64) {
    match (lo.is_finite(), hi.is_finite()) {
        (true, true) if (hi - lo).is_finite() => {
            if hi == lo {
                let half = (lo.abs() * 0.05).max(0.5);
                (lo - half, hi + half)
            } else {
                (lo, hi)
            }
        }
        (true, true) => (0.0, 1.0),
        (true, false) => (lo, lo + 1.0),
        (false, true) => (hi - 1.0, hi),
        (false, false) => (0.0, 1.0),
    }
}

/// Options for [`Axes::hist_with`]. Build with [`HistOpts::new`] and the chained
/// setters. Defaults to an mplhep step outline with no error bars and the next
/// cycle color.
#[derive(Debug, Clone, Default)]
pub struct HistOpts {
    pub(crate) histtype: HistType,
    pub(crate) yerr: bool,
    pub(crate) color: Option<Color>,
    pub(crate) fill_color: Option<Color>,
    pub(crate) label: Option<String>,
    pub(crate) linewidth_pt: Option<f32>,
}

impl HistOpts {
    /// New default options.
    #[must_use]
    pub fn new() -> Self {
        HistOpts::default()
    }
    /// Set the histogram type.
    #[must_use]
    pub fn histtype(mut self, t: HistType) -> Self {
        self.histtype = t;
        self
    }
    /// Draw `√N`/Sumw2 error bars at the bin centers.
    #[must_use]
    pub fn yerr(mut self) -> Self {
        self.yerr = true;
        self
    }
    /// Set the color.
    #[must_use]
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
    /// Set the fill color.
    #[must_use]
    pub fn fill_color(mut self, c: Color) -> Self {
        self.fill_color = Some(c);
        self
    }
    /// Set the legend label.
    #[must_use]
    pub fn label(mut self, s: impl Into<String>) -> Self {
        self.label = Some(s.into());
        self
    }
    /// Set the line width in points.
    #[must_use]
    pub fn linewidth(mut self, pt: f32) -> Self {
        self.linewidth_pt = Some(pt);
        self
    }
}

/// Options for [`Axes::errorbar_with`]. Build with [`ErrorbarOpts::new`] and the
/// chained setters. Defaults to the HEP data-point look: round markers,
/// vertical/horizontal error bars, no caps, no connecting line.
#[derive(Debug, Clone)]
pub struct ErrorbarOpts {
    pub(crate) color: Option<Color>,
    pub(crate) marker: Marker,
    pub(crate) marker_size_pt: Option<f32>,
    pub(crate) capsize_pt: f32,
    pub(crate) line: bool,
    pub(crate) label: Option<String>,
}

impl Default for ErrorbarOpts {
    fn default() -> Self {
        ErrorbarOpts {
            color: None,
            marker: Marker::Circle,
            marker_size_pt: None,
            capsize_pt: 0.0,
            line: false,
            label: None,
        }
    }
}

impl ErrorbarOpts {
    /// New default options.
    #[must_use]
    pub fn new() -> Self {
        ErrorbarOpts::default()
    }
    /// Set the color.
    #[must_use]
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
    /// Set the marker shape.
    #[must_use]
    pub fn marker(mut self, m: Marker) -> Self {
        self.marker = m;
        self
    }
    /// Set the marker size in points.
    #[must_use]
    pub fn marker_size(mut self, pt: f32) -> Self {
        self.marker_size_pt = Some(pt);
        self
    }
    /// Set the error-bar cap size in points.
    #[must_use]
    pub fn capsize(mut self, pt: f32) -> Self {
        self.capsize_pt = pt;
        self
    }
    /// Draw a connecting line through the points.
    #[must_use]
    pub fn line(mut self) -> Self {
        self.line = true;
        self
    }
    /// Set the legend label.
    #[must_use]
    pub fn label(mut self, s: impl Into<String>) -> Self {
        self.label = Some(s.into());
        self
    }
}

/// Options for [`Axes::hist2d_with`]. Build with [`Hist2dOpts::new`] and the
/// chained setters.
#[derive(Debug, Clone, Default)]
pub struct Hist2dOpts {
    pub(crate) cmap: Colormap,
    pub(crate) vmin: Option<f64>,
    pub(crate) vmax: Option<f64>,
    pub(crate) norm: crate::norm::Norm,
    pub(crate) label: Option<String>,
}

impl Hist2dOpts {
    /// New default options.
    #[must_use]
    pub fn new() -> Self {
        Hist2dOpts::default()
    }
    /// Set the colormap.
    #[must_use]
    pub fn cmap(mut self, c: Colormap) -> Self {
        self.cmap = c;
        self
    }
    /// Set the value (color) range, e.g. `Hist2dOpts::new().vrange(0.0..100.0)`.
    #[must_use]
    pub fn vrange(mut self, range: Range<f64>) -> Self {
        self.vmin = Some(range.start);
        self.vmax = Some(range.end);
        self
    }
    /// Set the color normalization ([`Norm::Linear`](crate::Norm::Linear) default,
    /// [`Norm::Log`](crate::Norm::Log), or [`Norm::SymLog`](crate::Norm::SymLog)).
    #[must_use]
    pub fn norm(mut self, norm: crate::norm::Norm) -> Self {
        self.norm = norm;
        self
    }
    /// Shorthand for a base-10 log color scale (`.norm(Norm::Log)`): the colorbar
    /// gets decade ticks and non-positive bins are masked.
    #[must_use]
    pub fn log(self) -> Self {
        self.norm(crate::norm::Norm::Log)
    }
    /// Set the colorbar label.
    #[must_use]
    pub fn label(mut self, s: impl Into<String>) -> Self {
        self.label = Some(s.into());
        self
    }
}

/// Options for [`Axes::function_with`] / `Axes::model_with` — a sampled analytic
/// curve. Build with [`CurveOpts::new`] and the chained setters.
#[derive(Debug, Clone)]
pub struct CurveOpts {
    pub(crate) color: Option<Color>,
    pub(crate) label: Option<String>,
    pub(crate) linewidth_pt: Option<f32>,
    pub(crate) dash: Option<Vec<f32>>,
    pub(crate) samples: usize,
}

impl Default for CurveOpts {
    fn default() -> Self {
        CurveOpts {
            color: None,
            label: None,
            linewidth_pt: None,
            dash: None,
            samples: 256,
        }
    }
}

impl CurveOpts {
    /// New default options.
    #[must_use]
    pub fn new() -> Self {
        CurveOpts::default()
    }
    /// Set the color.
    #[must_use]
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
    /// Set the legend label.
    #[must_use]
    pub fn label(mut self, s: impl Into<String>) -> Self {
        self.label = Some(s.into());
        self
    }
    /// Set the line width in points.
    #[must_use]
    pub fn linewidth(mut self, pt: f32) -> Self {
        self.linewidth_pt = Some(pt);
        self
    }
    /// Set a dash pattern (points).
    #[must_use]
    pub fn dashed(mut self, pattern: Vec<f32>) -> Self {
        self.dash = Some(pattern);
        self
    }
    /// Set the number of sample points.
    #[must_use]
    pub fn samples(mut self, n: usize) -> Self {
        self.samples = n;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Binned1D, Binned2D, Points};

    fn hist(edges: &[f64], values: &[f64]) -> Binned1D {
        Binned1D::new(edges.to_vec(), values.to_vec()).unwrap()
    }

    #[test]
    fn a_histogram_sets_sticky_x_edges() {
        let mut ax = Axes::new();
        ax.hist(&hist(&[0.0, 50.0, 100.0], &[1.0, 2.0]));
        assert_eq!(ax.resolved_xlim(), (0.0, 100.0));
        // A second histogram widens them.
        ax.hist(&hist(&[-20.0, 0.0], &[1.0]));
        assert_eq!(ax.resolved_xlim(), (-20.0, 100.0));
    }

    #[test]
    fn user_limits_survive_later_plots() {
        // Setting the range first, then plotting, must not widen it.
        let mut ax = Axes::new();
        ax.xlim(20.0..80.0);
        ax.hist(&hist(&[0.0, 50.0, 100.0], &[1.0, 2.0]));
        assert_eq!(ax.resolved_xlim(), (20.0, 80.0));

        let mut ax = Axes::new();
        ax.xlim(0.25..0.75).ylim(0.2..0.8);
        ax.hist2d(&Binned2D::new(vec![0.0, 1.0], vec![0.0, 1.0], vec![vec![1.0]]).unwrap());
        assert_eq!(ax.resolved_xlim(), (0.25, 0.75));
        assert_eq!(ax.resolved_ylim(), (0.2, 0.8));
    }

    #[test]
    fn histogram_y_range_starts_at_zero_and_keeps_negative_bins() {
        let mut ax = Axes::new();
        ax.hist(&hist(&[0.0, 1.0, 2.0], &[2.0, 10.0]));
        let (lo, hi) = ax.resolved_ylim();
        assert_eq!(lo, 0.0);
        assert!(hi > 10.0 && hi < 11.0, "{hi}");

        // A subtracted or weighted histogram goes below zero: show it.
        let mut ax = Axes::new();
        ax.hist(&hist(&[0.0, 1.0, 2.0], &[-5.0, 3.0]));
        let (lo, hi) = ax.resolved_ylim();
        assert!(lo < -5.0 && hi > 3.0, "({lo}, {hi})");

        let mut ax = Axes::new();
        ax.hist(&hist(&[0.0, 1.0, 2.0], &[-5.0, -3.0]));
        let (lo, hi) = ax.resolved_ylim();
        assert!(lo < -5.0 && hi == 0.0, "({lo}, {hi})");
    }

    #[test]
    fn error_bars_below_zero_do_not_lower_a_histogram_axis() {
        // Counts near 0 with error bars reaching below it: the axis still
        // stands on 0.
        let mut ax = Axes::new();
        ax.hist(&hist(&[0.0, 1.0, 2.0], &[0.0, 10.0]));
        ax.errorbar(
            &Points::new(vec![0.5, 1.5], vec![0.0, 9.0])
                .unwrap()
                .with_y_errors(vec![1.5, 3.0])
                .unwrap(),
        );
        assert_eq!(ax.resolved_ylim().0, 0.0);
        // A step histogram's own error bars likewise.
        let mut ax = Axes::new();
        ax.hist_with(
            &hist(&[0.0, 1.0, 2.0], &[0.5, 10.0]),
            HistOpts::new().yerr(),
        );
        assert_eq!(ax.resolved_ylim().0, 0.0);
        // A negative value does take it below, error bars and all.
        let mut ax = Axes::new();
        ax.hist_with(
            &hist(&[0.0, 1.0, 2.0], &[-2.0, 10.0]),
            HistOpts::new().yerr(),
        );
        let lo = ax.resolved_ylim().0;
        assert!(lo < -2.0 - 2f64.sqrt(), "{lo}");
    }

    #[test]
    fn limits_are_always_drawable() {
        let finite = |(a, b): (f64, f64)| a.is_finite() && b.is_finite() && a != b;

        let mut ax = Axes::new();
        ax.plot(&[0.0, 1.0], &[0.0, 1.0]).unwrap();
        ax.xlim(5.0..5.0).ylim(f64::NAN..1.0);
        assert!(finite(ax.resolved_xlim()) && finite(ax.resolved_ylim()));

        // A span too wide for f64.
        let mut ax = Axes::new();
        ax.plot(&[-1e308, 1e308], &[0.0, 1.0]).unwrap();
        assert!(finite(ax.resolved_xlim()));

        // Infinite and NaN data are left out of the autoscale.
        let mut ax = Axes::new();
        ax.hist(&hist(
            &[0.0, 1.0, 2.0, 3.0],
            &[f64::INFINITY, 4.0, f64::NAN],
        ));
        let (lo, hi) = ax.resolved_ylim();
        assert!(lo == 0.0 && hi > 4.0 && hi < 5.0, "({lo}, {hi})");

        // An inverted range is the caller's choice: it is kept.
        let mut ax = Axes::new();
        ax.xlim(10.0..0.0);
        assert_eq!(ax.resolved_xlim(), (10.0, 0.0));
    }

    #[test]
    fn band_and_errorbar_histograms_carry_their_errors() {
        for histtype in [HistType::Band, HistType::Errorbar] {
            let mut ax = Axes::new();
            ax.hist_with(
                &hist(&[0.0, 1.0, 2.0], &[4.0, 9.0]),
                HistOpts::new().histtype(histtype),
            );
            let Artist::Step(a) = &ax.artists[0] else {
                panic!("not a histogram")
            };
            assert_eq!(a.errs, Some(vec![2.0, 3.0]), "{histtype}");
        }
        // A plain step outline draws no error bars unless asked.
        let mut ax = Axes::new();
        ax.hist(&hist(&[0.0, 1.0], &[4.0]));
        let Artist::Step(a) = &ax.artists[0] else {
            panic!("not a histogram")
        };
        assert_eq!(a.errs, None);
    }

    #[test]
    fn profile_and_plot_options_reach_the_legend() {
        let mut ax = Axes::new();
        let tp = hist(&[0.0, 1.0, 2.0], &[1.0, 2.0]);
        ax.profile_with(
            &tp,
            ErrorbarOpts::new().color(Color::BLACK).label("profile"),
        );
        ax.plot_with(&[0.0, 1.0], &[1.0, 2.0], CurveOpts::new().label("line"))
            .unwrap();
        ax.errorbar(&Points::new(vec![0.5], vec![1.0]).unwrap());
        let labels: Vec<String> = ax.legend_items().into_iter().map(|i| i.label).collect();
        assert_eq!(labels, ["profile", "line"]);
        let Artist::Errorbar(p) = &ax.artists[0] else {
            panic!("not points")
        };
        assert_eq!(p.color, Color::BLACK);
    }

    #[test]
    fn a_log_axis_autoscales_to_the_positive_data() {
        // A histogram on a log y axis cannot stand on 0: it starts below its
        // smallest positive bin, and empty bins do not drag it down.
        let mut ax = Axes::new();
        ax.hist(&hist(&[0.0, 1.0, 2.0, 3.0], &[1e4, 0.0, 3.0]))
            .yscale(Scale::Log);
        let (lo, hi) = ax.resolved_ylim();
        assert!(lo > 0.3 && lo < 3.0 && hi > 1e4 && hi < 1e5, "({lo}, {hi})");

        // A log x axis keeps a histogram's edges, except one at 0.
        let mut ax = Axes::new();
        ax.hist(&hist(&[0.0, 10.0, 100.0], &[1.0, 2.0]))
            .xscale(Scale::Log);
        let (lo, hi) = ax.resolved_xlim();
        assert!(lo > 0.0 && lo <= 10.0 && hi == 100.0, "({lo}, {hi})");

        // Requested limits win; a non-positive end is replaced.
        ax.xlim(5.0..500.0);
        assert_eq!(ax.resolved_xlim(), (5.0, 500.0));
        ax.xlim(-1.0..500.0);
        assert!(ax.resolved_xlim().0 > 0.0);

        // Nothing positive at all: one decade.
        let mut ax = Axes::new();
        ax.plot(&[-1.0, 0.0], &[-2.0, 0.0]).unwrap();
        ax.yscale(Scale::Log);
        assert_eq!(ax.resolved_ylim(), (1.0, 10.0));
    }

    #[test]
    fn a_log_axis_places_decades_evenly() {
        let mut ax = Axes::new();
        ax.ylim(1.0..1000.0).yscale(Scale::Log);
        let t = ax.transform(Rect::new(0.0, 0.0, 100.0, 300.0), ax.limits());
        assert_eq!(t.y(1.0), 300.0);
        assert_eq!(t.y(10.0), 200.0);
        assert_eq!(t.y(1000.0), 0.0);
        // Values a log axis cannot show land far below the frame (clipped).
        assert!(t.y(0.0) > 1000.0 && t.y(-5.0) > 1000.0);
    }

    #[test]
    fn profile_adopts_a_time_axis() {
        struct Timed;
        impl Hist1dData for Timed {
            fn edges(&self) -> Vec<f64> {
                vec![0.0, 60.0]
            }
            fn values(&self) -> Vec<f64> {
                vec![1.0]
            }
            fn error(&self, _: usize) -> f64 {
                0.1
            }
            fn x_time_format(&self) -> Option<String> {
                Some("%H:%M".into())
            }
        }
        let mut ax = Axes::new();
        ax.profile(&Timed);
        assert!(ax.x_time_format_for_test().is_some());
    }
}
