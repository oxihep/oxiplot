//! The `Figure` — the top-level canvas. Holds one or more axes laid out by a
//! [`GridSpec`] and writes the result to PNG, SVG, or PDF (chosen by the output
//! file extension).

use std::path::Path;

use crate::axes::{Axes, Insets};
use crate::color::Color;
use crate::draw::{DrawGroup, Rect};
use crate::error::{Error, Result};
use crate::gridspec::GridSpec;
use crate::render::{pdf, raster, svg};
use crate::style::Style;

/// Options for saving a figure. Build with [`SaveOpts::new`] and the chained
/// setters.
///
/// # Examples
/// ```
/// use oxiplot::SaveOpts;
/// let opts = SaveOpts::new().dpi(300.0).transparent();
/// ```
#[derive(Debug, Clone, Default)]
pub struct SaveOpts {
    pub(crate) dpi: Option<f32>,
    pub(crate) transparent: bool,
}

impl SaveOpts {
    /// New default options.
    #[must_use]
    pub fn new() -> Self {
        SaveOpts::default()
    }
    /// Set the output DPI (raster resolution). `None` uses the style's dpi (100).
    /// Higher values give a sharper PNG. A PDF page is the figure's size in
    /// points whatever the dpi; an SVG's pixel size follows it.
    #[must_use]
    pub fn dpi(mut self, dpi: f32) -> Self {
        self.dpi = Some(dpi);
        self
    }
    /// Render with a transparent background (no opaque page fill).
    #[must_use]
    pub fn transparent(mut self) -> Self {
        self.transparent = true;
        self
    }
}

/// The top-level figure: a grid of axes panels.
pub struct Figure {
    style: Style,
    grid: GridSpec,
    axes: Vec<Axes>,
    sharex: bool,
    sharey: bool,
    suptitle: Option<String>,
}

impl Figure {
    /// A new figure with the default style and a 1×1 grid.
    #[must_use]
    pub fn new() -> Self {
        Figure::with_style(Style::default())
    }

    /// A new figure with a custom style and a 1×1 grid.
    #[must_use]
    pub fn with_style(style: Style) -> Self {
        let grid = GridSpec::new(1, 1).margins(style.margins_frac);
        Figure {
            style,
            grid,
            axes: Vec::new(),
            sharex: false,
            sharey: false,
            suptitle: None,
        }
    }

    fn with_grid(style: Style, grid: GridSpec, sharex: bool, sharey: bool) -> Self {
        Figure {
            style,
            grid,
            axes: Vec::new(),
            sharex,
            sharey,
            suptitle: None,
        }
    }

    /// Share the x-axis across all panels (common x range; only the bottom row
    /// keeps its x tick labels and x-axis label).
    #[must_use]
    pub fn sharex(mut self) -> Self {
        self.sharex = true;
        self
    }

    /// Share the y-axis across all panels (common y range; only the left column
    /// keeps its y tick labels and y-axis label).
    #[must_use]
    pub fn sharey(mut self) -> Self {
        self.sharey = true;
        self
    }

    /// Set a figure-level title centered above the panels (supports `$…$` math).
    #[must_use]
    pub fn suptitle(mut self, s: impl Into<String>) -> Self {
        self.suptitle = Some(s.into());
        self
    }

    /// Place axes into the grid, row-major. Accepts anything iterable — a single
    /// `[ax]`, a `Vec<Axes>`, etc. With a shared x- or y-axis the panels are
    /// given a common range and the inner tick labels are hidden (only the bottom
    /// row keeps x labels, only the left column keeps y labels).
    #[must_use]
    pub fn with_axes(mut self, axes: impl IntoIterator<Item = Axes>) -> Self {
        let mut axes: Vec<Axes> = axes.into_iter().collect();
        let ncols = self.grid.ncols.max(1);
        if self.sharex && !axes.is_empty() {
            let (mut lo, mut hi) = axes[0].resolved_xlim();
            for ax in &axes[1..] {
                let (l, h) = ax.resolved_xlim();
                lo = lo.min(l);
                hi = hi.max(h);
            }
            let last_row = axes.len().saturating_sub(1) / ncols;
            // Touching rows share a seam: drop the top y label of every row
            // below the first, or it would print over the bottom label of
            // the panel above (as `ratio_subplots` does for its ratio panel).
            let touching = self.grid.hspace == 0.0;
            for (i, ax) in axes.iter_mut().enumerate() {
                ax.xlim(lo..hi);
                if i / ncols < last_row {
                    ax.hide_xticklabels();
                }
                if touching && i >= ncols {
                    ax.prune_ylabels(false, true);
                }
            }
        }
        if self.sharey && !axes.is_empty() {
            let (mut lo, mut hi) = axes[0].resolved_ylim();
            for ax in &axes[1..] {
                let (l, h) = ax.resolved_ylim();
                lo = lo.min(l);
                hi = hi.max(h);
            }
            for (i, ax) in axes.iter_mut().enumerate() {
                ax.ylim(lo..hi);
                if i % ncols != 0 {
                    ax.hide_yticklabels();
                }
            }
        }
        self.axes = axes;
        self
    }

    /// Convenience for a ratio plot: place the main and ratio panels.
    #[must_use]
    pub fn ratio(self, main: Axes, ratio: Axes) -> Self {
        self.with_axes([main, ratio])
    }

    /// Render every panel and save to `path` (`.png`, `.svg`, or `.pdf`).
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        self.save_with(path, SaveOpts::default())
    }

    /// Render and save with explicit options (DPI, transparency).
    pub fn save_with(&self, path: impl AsRef<Path>, opts: SaveOpts) -> Result<()> {
        let (groups, w, h) = self.render_groups(opts.dpi);
        write_groups(
            &groups,
            w,
            h,
            self.background(opts.transparent),
            self.dpi(opts.dpi),
            path.as_ref(),
        )
    }

    /// Render to an in-memory PNG (honoring [`SaveOpts`] DPI/transparency) instead
    /// of writing a file.
    pub fn to_png_bytes(&self, opts: SaveOpts) -> Result<Vec<u8>> {
        let (groups, w, h) = self.render_groups(opts.dpi);
        raster::render_png(&groups, w, h, self.background(opts.transparent))
    }

    /// Render to an in-memory SVG string.
    #[must_use]
    pub fn to_svg_string(&self) -> String {
        let (groups, w, h) = self.render_groups(None);
        svg::render(&groups, w, h, self.background(false))
    }

    /// Render to in-memory PDF bytes.
    #[must_use]
    pub fn to_pdf_bytes(&self) -> Vec<u8> {
        let (groups, w, h) = self.render_groups(None);
        pdf::render(&groups, w, h, self.background(false), self.style.dpi)
    }

    /// The page background color (`face_color`, or fully transparent).
    pub(crate) fn background(&self, transparent: bool) -> Color {
        if transparent {
            Color::TRANSPARENT
        } else {
            self.style.face_color
        }
    }

    /// The effective dpi for `opts`.
    fn dpi(&self, dpi: Option<f32>) -> f32 {
        dpi.unwrap_or(self.style.dpi)
    }

    /// Render all panels (and the suptitle) into draw groups at the effective
    /// dpi, returning them plus the pixel size used.
    pub(crate) fn render_groups(&self, dpi: Option<f32>) -> (Vec<DrawGroup>, u32, u32) {
        let dpi = self.dpi(dpi);
        let w = (self.style.figsize_in.0 * dpi).round() as u32;
        let h = (self.style.figsize_in.1 * dpi).round() as u32;
        // Every panel draws at the figure's dpi.
        let axes: Vec<Axes> = self
            .axes
            .iter()
            .map(|ax| {
                let mut ax = ax.clone();
                ax.style.dpi = dpi;
                ax
            })
            .collect();
        (self.groups_for(&axes, dpi, w, h), w, h)
    }

    /// Render at an explicit pixel size (the style's dpi).
    #[cfg(test)]
    pub(crate) fn groups_at(&self, w: u32, h: u32) -> Vec<DrawGroup> {
        self.groups_for(&self.axes, self.style.dpi, w, h)
    }

    fn groups_for(&self, axes: &[Axes], dpi: f32, w: u32, h: u32) -> Vec<DrawGroup> {
        let layout = self.layout(axes, dpi, w as f32, h as f32);
        let mut groups: Vec<DrawGroup> = Vec::new();
        for ((ax, &box_), &area) in axes.iter().zip(&layout.boxes).zip(&layout.ytick_area) {
            let mut ax = ax.clone();
            ax.ytick_area = Some(area);
            groups.extend(ax.render_at(box_));
        }
        if let (Some(text), Some(y)) = (&self.suptitle, layout.suptitle_y) {
            let mut g = DrawGroup::new(None);
            crate::mathtext::layout_label(
                &mut g,
                &self.style.fonts,
                text,
                w as f32 / 2.0,
                y,
                self.suptitle_size(dpi),
                self.style.fg_color,
                crate::text::HAlign::Center,
                crate::text::VAlign::Bottom,
                0.0,
            );
            groups.push(g);
        }
        groups
    }

    /// The suptitle's font size in pixels at `dpi`.
    fn suptitle_size(&self, dpi: f32) -> f32 {
        self.style.title_size_pt * 1.2 * dpi / 72.0
    }

    /// Place the panels' frames so their decorations fit: the grid's margins
    /// and gaps are the least room left, widened wherever a panel's tick
    /// labels, axis labels or title (or the suptitle) need more, the way
    /// matplotlib's constrained layout does. Panels set to touch (`hspace` or
    /// `wspace` of 0, as in a ratio plot) stay touching. In each column the
    /// y-axis labels are aligned.
    fn layout(&self, axes: &[Axes], dpi: f32, w: f32, h: f32) -> Layout {
        let g = &self.grid;
        let (nrows, ncols) = (g.nrows.max(1), g.ncols.max(1));
        let px = |pt: f32| pt * dpi / 72.0;
        // Panel i's cell; panels beyond the grid share its last cell.
        let cell = |i: usize| ((i / ncols).min(nrows - 1), i % ncols);
        let plain: Vec<Rect> = (0..axes.len())
            .map(|i| {
                let (r, c) = cell(i);
                g.cell_box(w, h, r, r, c, c)
            })
            .collect();

        let sup_h = self.suptitle.as_ref().map_or(0.0, |t| {
            let (_, a, d) =
                crate::mathtext::label_extents(&self.style.fonts, t, self.suptitle_size(dpi));
            a + d + px(SUPTITLE_PAD_PT)
        });
        let (edge, gap_pad) = (px(EDGE_PAD_PT), px(PANEL_PAD_PT));

        // The user's gaps between neighbouring cells, in pixels.
        let col_gaps: Vec<f32> = (0..ncols.saturating_sub(1))
            .map(|c| g.cell_box(w, h, 0, 0, c + 1, c + 1).x - g.cell_box(w, h, 0, 0, c, c).right())
            .collect();
        let row_gaps: Vec<f32> = (0..nrows.saturating_sub(1))
            .map(|r| g.cell_box(w, h, r + 1, r + 1, 0, 0).y - g.cell_box(w, h, r, r, 0, 0).bottom())
            .collect();
        let (l, r, b, t) = g.margins;

        // Two passes: the tick labels (and so the room they need) depend on
        // the frame size, which the first pass changes.
        let mut boxes = plain.clone();
        let mut insets = Vec::new();
        for _ in 0..2 {
            insets = axes
                .iter()
                .zip(&boxes)
                .map(|(ax, &bx)| ax.insets(bx))
                .collect::<Vec<Insets>>();
            let mut left = vec![0.0_f32; ncols];
            let mut right = vec![0.0_f32; ncols];
            let mut top = vec![0.0_f32; nrows];
            let mut bottom = vec![0.0_f32; nrows];
            for (i, ins) in insets.iter().enumerate() {
                let (row, col) = cell(i);
                left[col] = left[col].max(ins.left);
                right[col] = right[col].max(ins.right);
                top[row] = top[row].max(ins.top);
                bottom[row] = bottom[row].max(ins.bottom);
            }
            let m_left = (l * w).max(left[0] + edge);
            let m_right = ((1.0 - r) * w).max(right[ncols - 1] + edge);
            let m_top = ((1.0 - t) * h).max(top[0] + sup_h + edge);
            let m_bottom = (b * h).max(bottom[nrows - 1] + edge);
            let gaps = |user: &[f32], after: &[f32], before: &[f32]| -> Vec<f32> {
                user.iter()
                    .enumerate()
                    .map(|(k, &u)| {
                        if u <= 0.0 {
                            u.max(0.0)
                        } else {
                            u.max(after[k] + before[k + 1] + gap_pad)
                        }
                    })
                    .collect()
            };
            let cgaps = gaps(&col_gaps, &right, &left);
            let rgaps = gaps(&row_gaps, &bottom, &top);
            let avail_w = w - m_left - m_right - cgaps.iter().sum::<f32>();
            let avail_h = h - m_top - m_bottom - rgaps.iter().sum::<f32>();
            // Decorations that leave the frames almost nothing: keep the
            // grid as it is rather than squash the data away.
            if avail_w < 0.25 * w || avail_h < 0.25 * h {
                boxes = plain.clone();
                break;
            }
            let xs = spans(m_left, avail_w, &g.width_ratios, &cgaps, ncols);
            let ys = spans(m_top, avail_h, &g.height_ratios, &rgaps, nrows);
            boxes = (0..axes.len())
                .map(|i| {
                    let (row, col) = cell(i);
                    Rect::new(xs[col].0, ys[row].0, xs[col].1, ys[row].1)
                })
                .collect();
        }

        // Align the y-axis labels of each column on its widest tick labels.
        let mut area = vec![0.0_f32; ncols];
        for (i, ins) in insets.iter().enumerate() {
            let col = cell(i).1;
            area[col] = area[col].max(ins.ytick_area);
        }
        let top_frame = boxes.iter().map(|b| b.y).fold(f32::INFINITY, f32::min);
        let top_deco = insets.iter().map(|i| i.top).fold(0.0_f32, f32::max);
        Layout {
            ytick_area: (0..axes.len()).map(|i| area[cell(i).1]).collect(),
            suptitle_y: self.suptitle.as_ref().map(|_| {
                if top_frame.is_finite() {
                    (top_frame - top_deco - px(SUPTITLE_PAD_PT)).max(sup_h - px(SUPTITLE_PAD_PT))
                } else {
                    sup_h
                }
            }),
            boxes,
        }
    }
}

/// Room left at the figure's edges beyond the outermost decorations, in points.
const EDGE_PAD_PT: f32 = 3.0;
/// Room left between the decorations of neighbouring panels, in points.
const PANEL_PAD_PT: f32 = 6.0;
/// Room between the suptitle and the panels' titles, in points.
const SUPTITLE_PAD_PT: f32 = 6.0;

/// Where a figure's panels and suptitle go.
struct Layout {
    /// Each panel's frame.
    boxes: Vec<Rect>,
    /// Each panel's y tick label width, aligned within its column.
    ytick_area: Vec<f32>,
    /// The baseline-bottom of the suptitle.
    suptitle_y: Option<f32>,
}

/// `(start, size)` of `n` cells laid from `start` over `avail` pixels, sized
/// by `ratios` (missing ones count 1), with `gaps[k]` after cell `k`.
fn spans(start: f32, avail: f32, ratios: &[f32], gaps: &[f32], n: usize) -> Vec<(f32, f32)> {
    let ratio = |k: usize| ratios.get(k).copied().unwrap_or(1.0).max(0.0);
    let total: f32 = (0..n).map(ratio).sum::<f32>().max(f32::EPSILON);
    let mut pos = start;
    (0..n)
        .map(|k| {
            let size = avail * ratio(k) / total;
            let span = (pos, size);
            pos += size + gaps.get(k).copied().unwrap_or(0.0);
            span
        })
        .collect()
}

impl Default for Figure {
    fn default() -> Self {
        Figure::new()
    }
}

/// Collect several figures into a single multi-page vector PDF (matplotlib's
/// `PdfPages`). Each added figure becomes one page, at its own size; save the
/// whole document with [`save`](PdfPages::save).
///
/// # Examples
/// ```no_run
/// use oxiplot::{subplots, PdfPages};
/// let mut pdf = PdfPages::new();
/// for k in 0..3 {
///     let (fig, mut ax) = subplots();
///     ax.plot(&[0.0, 1.0], &[0.0, k as f64]).unwrap();
///     pdf.add(&fig);
/// }
/// pdf.save("report.pdf").unwrap(); // one PDF, three pages
/// ```
#[derive(Default)]
pub struct PdfPages {
    pages: Vec<pdf::PdfPage>,
}

impl PdfPages {
    /// A new, empty document.
    #[must_use]
    pub fn new() -> Self {
        PdfPages::default()
    }

    /// Append `fig` as the next page (default save options). Returns `&mut self`
    /// so calls can be chained.
    pub fn add(&mut self, fig: &Figure) -> &mut Self {
        self.add_with(fig, SaveOpts::default())
    }

    /// Append `fig` as the next page with explicit options (DPI, transparency).
    pub fn add_with(&mut self, fig: &Figure, opts: SaveOpts) -> &mut Self {
        let (groups, w, h) = fig.render_groups(opts.dpi);
        self.pages.push(pdf::page(
            &groups,
            w,
            h,
            fig.background(opts.transparent),
            fig.dpi(opts.dpi),
        ));
        self
    }

    /// Append a single axes as the next page, at its own style's size (as
    /// [`Axes::save`] would draw it).
    pub fn add_axes(&mut self, ax: &Axes) -> &mut Self {
        self.add(&Figure::with_style(ax.style.clone()).with_axes([ax.clone()]))
    }

    /// The number of pages added so far.
    #[must_use]
    pub fn len(&self) -> usize {
        self.pages.len()
    }

    /// Whether no pages have been added yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    /// The assembled multi-page PDF as bytes.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        pdf::render_pages(&self.pages)
    }

    /// Write the multi-page PDF to `path`.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        std::fs::write(path, self.to_bytes())?;
        Ok(())
    }
}

/// Create a figure and a single axes sharing the default style (matplotlib's
/// `subplots()`).
#[must_use]
pub fn subplots() -> (Figure, Axes) {
    let style = Style::default();
    (Figure::with_style(style.clone()), Axes::with_style(style))
}

/// Like [`subplots`] but with a custom style (e.g. [`Style::mplhep`]).
#[must_use]
pub fn subplots_with(style: Style) -> (Figure, Axes) {
    (Figure::with_style(style.clone()), Axes::with_style(style))
}

/// Create a figure with an `nrows × ncols` grid of axes (row-major), matplotlib
/// `subplots(nrows, ncols)`. Fill the returned axes, then `fig.with_axes(axes)`.
///
/// # Examples
/// ```no_run
/// use oxiplot::subplots_grid;
/// let (fig, mut axs) = subplots_grid(1, 2);
/// axs[0].plot(&[0.0, 1.0], &[0.0, 1.0]);
/// axs[1].plot(&[0.0, 1.0], &[1.0, 0.0]);
/// fig.with_axes(axs).save("two.png").unwrap();
/// ```
#[must_use]
pub fn subplots_grid(nrows: usize, ncols: usize) -> (Figure, Vec<Axes>) {
    subplots_grid_with(Style::default(), GridSpec::new(nrows, ncols))
}

/// Like [`subplots_grid`] with a custom style and [`GridSpec`].
#[must_use]
pub fn subplots_grid_with(style: Style, grid: GridSpec) -> (Figure, Vec<Axes>) {
    let n = grid.nrows * grid.ncols;
    let grid = if grid.margins == GridSpec::new(1, 1).margins {
        grid.margins(style.margins_frac)
    } else {
        grid
    };
    let axes = (0..n).map(|_| Axes::with_style(style.clone())).collect();
    (Figure::with_grid(style, grid, false, false), axes)
}

/// A two-panel ratio plot: a main panel over a shorter ratio panel sharing the
/// x-axis (height ratios 3:1, touching). Returns `(figure, main, ratio)`; fill
/// both panels, then `fig.ratio(main, ratio).save(...)`.
///
/// # Examples
/// ```no_run
/// use oxiplot::{ratio_subplots, Binned1D};
/// let edges: Vec<f64> = (0..=20).map(|i| 0.5 * i as f64).collect();
/// let h = Binned1D::from_samples(edges, &[2.5, 3.1, 3.3, 4.8]);
/// let (fig, mut main, mut ratio) = ratio_subplots();
/// main.hist(&h);
/// main.ylabel("Events");
/// ratio.ylim(0.5..1.5);
/// ratio.ylabel("data/MC");
/// ratio.xlabel("x");
/// fig.ratio(main, ratio).save("ratio.svg").unwrap();
/// ```
#[must_use]
pub fn ratio_subplots() -> (Figure, Axes, Axes) {
    ratio_subplots_with(Style::default())
}

/// Like [`ratio_subplots`] with a custom style.
#[must_use]
pub fn ratio_subplots_with(style: Style) -> (Figure, Axes, Axes) {
    let grid = GridSpec::new(2, 1)
        .height_ratios(vec![3.0, 1.0])
        .hspace(0.0)
        .margins(style.margins_frac);
    let fig = Figure::with_grid(style.clone(), grid, true, false);
    let main = Axes::with_style(style.clone());
    let mut ratio = Axes::with_style(style);
    // Drop the ratio panel's top y label so it doesn't collide with the main
    // panel's bottom label at the shared seam.
    ratio.prune_ylabels(false, true);
    (fig, main, ratio)
}

/// Render draw groups and write the chosen image format to `path`.
pub(crate) fn write_groups(
    groups: &[DrawGroup],
    w: u32,
    h: u32,
    bg: Color,
    dpi: f32,
    path: &Path,
) -> Result<()> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" => std::fs::write(path, raster::render_png(groups, w, h, bg)?)?,
        "svg" => std::fs::write(path, svg::render(groups, w, h, bg))?,
        "pdf" => std::fs::write(path, pdf::render(groups, w, h, bg, dpi))?,
        other => return Err(Error::UnknownFormat(other.to_string())),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Binned1D, Binned2D, CurveOpts, Hist2dOpts};

    /// The frames and insets a figure lays out, at the style's size.
    fn laid_out(fig: &Figure) -> (Layout, Vec<Insets>) {
        let (w, h) = fig.style.figsize_px();
        let layout = fig.layout(&fig.axes, fig.style.dpi, w as f32, h as f32);
        let insets = fig
            .axes
            .iter()
            .zip(&layout.boxes)
            .map(|(ax, &b)| ax.insets(b))
            .collect();
        (layout, insets)
    }

    fn decorated(n: usize) -> Vec<Axes> {
        (0..n)
            .map(|k| {
                let mut ax = Axes::new();
                let e: Vec<f64> = (0..=10).map(|i| 100.0 * i as f64).collect();
                ax.hist(&Binned1D::new(e, vec![12345.0 * (k + 1) as f64; 10]).unwrap());
                ax.title(format!("Panel {k}"))
                    .xlabel("$p_T$ [GeV]")
                    .ylabel("Events / bin");
                ax
            })
            .collect()
    }

    #[test]
    fn grid_panels_leave_room_for_their_labels() {
        let (fig, _) = subplots_grid(2, 2);
        let fig = fig.suptitle("Suptitle").with_axes(decorated(4));
        let (l, ins) = laid_out(&fig);
        let (w, h) = fig.style.figsize_px();
        let b = &l.boxes;
        // Row 0's x labels end above row 1's titles.
        for (top, below) in [(0, 2), (1, 3)] {
            assert!(
                b[top].bottom() + ins[top].bottom <= b[below].y - ins[below].top,
                "row 0 decorations reach row 1"
            );
        }
        // Column 0 ends left of column 1's y label.
        for (left, right) in [(0, 1), (2, 3)] {
            assert!(b[left].right() + ins[left].right <= b[right].x - ins[right].left);
        }
        // Everything stays on the figure, and the suptitle above the titles.
        for (bx, i) in b.iter().zip(&ins) {
            assert!(bx.x - i.left >= 0.0 && bx.right() + i.right <= w as f32);
            assert!(bx.y - i.top >= 0.0 && bx.bottom() + i.bottom <= h as f32);
        }
        let sup = l.suptitle_y.unwrap();
        assert!(sup > 0.0 && sup <= b[0].y - ins[0].top, "{sup}");
    }

    #[test]
    fn touching_panels_stay_touching_with_aligned_y_labels() {
        let (fig, mut main, mut ratio) = ratio_subplots();
        let e: Vec<f64> = (0..=10).map(f64::from).collect();
        main.hist(&Binned1D::new(e, vec![123456.0; 10]).unwrap())
            .ylabel("Events");
        ratio.function(|_| 1.0, 0.0..10.0);
        ratio.ylim(0.5..1.5).ylabel("Ratio").xlabel("x");
        let fig = fig.ratio(main, ratio);
        let (l, _) = laid_out(&fig);
        assert_eq!(l.boxes[0].bottom(), l.boxes[1].y, "the panels came apart");
        assert_eq!(l.boxes[0].x, l.boxes[1].x);
        // Both y labels sit beyond the main panel's wider tick labels.
        assert_eq!(l.ytick_area[0], l.ytick_area[1]);
    }

    #[test]
    fn stacked_shared_panels_drop_the_label_at_each_seam() {
        let grid = GridSpec::new(3, 1).hspace(0.0);
        let (fig, axs) = subplots_grid_with(Style::default(), grid);
        let fig = fig.sharex().with_axes(axs);
        assert!(!fig.axes[0].ylabel_prune_for_test().1);
        assert!(fig.axes[1].ylabel_prune_for_test().1);
        assert!(fig.axes[2].ylabel_prune_for_test().1);
    }

    #[test]
    fn a_colorbar_makes_room_for_its_tick_labels() {
        let bar = |scale: f64| {
            let mut ax = Axes::new();
            let e = vec![0.0, 1.0, 2.0];
            let g = vec![
                vec![1.0 * scale, 2.0 * scale],
                vec![3.0 * scale, 8.0 * scale],
            ];
            ax.hist2d_with(
                &Binned2D::new(e.clone(), e, g).unwrap(),
                Hist2dOpts::new().label("entries"),
            );
            ax.render_at(Rect::new(80.0, 40.0, 500.0, 380.0))
                .into_iter()
                .find_map(|g| g.clip)
                .unwrap()
                .w
        };
        // Wider tick labels (80000 vs 8) leave a narrower frame.
        assert!(bar(10000.0) < bar(1.0));
    }

    #[test]
    fn a_title_sits_above_the_experiment_label() {
        let mut ax = Axes::new();
        ax.plot(&[0.0, 1.0], &[0.0, 1.0]).unwrap();
        let alone = ax.insets(Rect::new(80.0, 40.0, 500.0, 380.0)).top;
        ax.title("A title");
        let titled = ax.insets(Rect::new(80.0, 40.0, 500.0, 380.0)).top;
        ax.hep_label("Demo", "Preliminary");
        let both = ax.insets(Rect::new(80.0, 40.0, 500.0, 380.0)).top;
        assert!(alone == 0.0 && titled > 0.0 && both > titled);
    }

    #[test]
    fn pdf_pages_are_sized_in_points() {
        let media = |pdf: &[u8]| -> Vec<String> {
            let text = String::from_utf8_lossy(pdf);
            text.split("/MediaBox [0 0 ")
                .skip(1)
                .map(|r| r.split(']').next().unwrap().to_string())
                .collect()
        };
        let mut ax = Axes::new();
        ax.plot_with(&[0.0, 1.0], &[0.0, 1.0], CurveOpts::new().label("a"))
            .unwrap();
        // 6.4 × 4.8 in is 460.8 × 345.6 pt, at any dpi.
        assert_eq!(media(&ax.to_pdf_bytes()), ["460.8 345.6"]);
        let mut pages = PdfPages::new();
        pages.add_axes(&ax);
        let fig = Figure::new().with_axes([ax.clone()]);
        pages.add_with(&fig, SaveOpts::new().dpi(300.0));
        assert_eq!(media(&pages.to_bytes()), ["460.8 345.6", "460.8 345.6"]);
    }
}
