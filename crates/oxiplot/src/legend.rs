//! Legend rendering — a framed box of sample swatches and labels, placed at a
//! [`Loc`] inside the axes: by default the spot that covers the least data
//! (matplotlib's `loc="best"`).

use crate::artists::{LegendHandle, Marker, draw_marker};
use crate::axes::Axes;
use crate::color::Color;
use crate::draw::{DrawCommand, DrawGroup, Path, Rect, Seg, Stroke};
use crate::text::{HAlign, VAlign};

/// Where a legend goes inside the axes (matplotlib's `loc`).
///
/// ```
/// use oxiplot::{Axes, CurveOpts, Loc};
/// let mut ax = Axes::new();
/// ax.plot_with(&[0.0, 1.0], &[0.0, 1.0], CurveOpts::new().label("rising"))?;
/// ax.legend_loc(Loc::UpperLeft);
/// # Ok::<(), oxiplot::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Loc {
    /// The candidate spot that covers the fewest data points, trying them in
    /// matplotlib's order (upper right first, so an empty corner wins there).
    #[default]
    Best,
    /// Top-right corner.
    UpperRight,
    /// Top-left corner.
    UpperLeft,
    /// Bottom-left corner.
    LowerLeft,
    /// Bottom-right corner.
    LowerRight,
    /// Middle of the right edge.
    CenterRight,
    /// Middle of the left edge.
    CenterLeft,
    /// Middle of the bottom edge.
    LowerCenter,
    /// Middle of the top edge.
    UpperCenter,
    /// The centre of the axes.
    Center,
}

impl Loc {
    /// The fixed spots `Best` chooses from, in matplotlib's order.
    const CANDIDATES: [Loc; 9] = [
        Loc::UpperRight,
        Loc::UpperLeft,
        Loc::LowerLeft,
        Loc::LowerRight,
        Loc::CenterRight,
        Loc::CenterLeft,
        Loc::LowerCenter,
        Loc::UpperCenter,
        Loc::Center,
    ];

    /// The top-left corner of a `w × h` box at this spot in `frame`, `margin`
    /// in from its edges. `Best` is placed by [`legend_rect`], not here.
    fn origin(self, frame: Rect, w: f32, h: f32, margin: f32) -> (f32, f32) {
        let left = frame.x + margin;
        let right = frame.right() - margin - w;
        let hmid = frame.x + (frame.w - w) / 2.0;
        let top = frame.y + margin;
        let bottom = frame.bottom() - margin - h;
        let vmid = frame.y + (frame.h - h) / 2.0;
        match self {
            Loc::Best | Loc::UpperRight => (right, top),
            Loc::UpperLeft => (left, top),
            Loc::LowerLeft => (left, bottom),
            Loc::LowerRight => (right, bottom),
            Loc::CenterRight => (right, vmid),
            Loc::CenterLeft => (left, vmid),
            Loc::LowerCenter => (hmid, bottom),
            Loc::UpperCenter => (hmid, top),
            Loc::Center => (hmid, vmid),
        }
    }
}

/// A rounded rectangle as a path (matplotlib `fancybox` legend frame). `r` is the
/// corner radius in pixels, clamped so it never exceeds half the shorter side.
fn rounded_rect(rect: Rect, r: f32) -> Path {
    let (x, y, w, h) = (rect.x, rect.y, rect.w, rect.h);
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    Path {
        segs: vec![
            Seg::Move(x + r, y),
            Seg::Line(x + w - r, y),
            Seg::Quad(x + w, y, x + w, y + r),
            Seg::Line(x + w, y + h - r),
            Seg::Quad(x + w, y + h, x + w - r, y + h),
            Seg::Line(x + r, y + h),
            Seg::Quad(x, y + h, x, y + h - r),
            Seg::Line(x, y + r),
            Seg::Quad(x, y, x + r, y),
            Seg::Close,
        ],
    }
}

/// How a legend's entries are laid out: in columns filled top to bottom,
/// under an optional title.
struct Layout {
    /// Entries per column.
    rows: usize,
    /// The width of each column: handle, gap and its widest label.
    col_w: Vec<f32>,
    /// The height of the title row (0 with no title).
    title_h: f32,
    box_w: f32,
    box_h: f32,
}

/// Lay the legend of `ax` out, or `None` when it has no entries.
fn layout(ax: &Axes) -> Option<Layout> {
    let items = ax.legend_items();
    if items.is_empty() {
        return None;
    }
    let s = &ax.style;
    let fs = s.px(s.legend_size_pt);
    // Spacing in font-size units, matching matplotlib's legend rcParams:
    // borderpad 0.4, handlelength 2.0, handletextpad 0.8, labelspacing 0.5,
    // columnspacing 2.0.
    let pad = 0.4 * fs;
    let gap = 0.8 * fs;
    let handle_w = 2.0 * fs;
    let row_h = fs * 1.5;
    let col_gap = 2.0 * fs;

    let cols = ax.legend_columns().clamp(1, items.len());
    let rows = items.len().div_ceil(cols);
    let col_w: Vec<f32> = items
        .chunks(rows)
        .map(|column| {
            let widest = column
                .iter()
                // Measured as drawn: a `$…$` label is typeset, so its raw
                // source (backslashes, braces) says nothing about its width.
                .map(|it| crate::mathtext::label_extents(&s.fonts, &it.label, fs).0)
                .fold(0.0_f32, f32::max);
            handle_w + gap + widest
        })
        .collect();
    let (title_w, title_h) = match ax.legend_heading() {
        Some(title) => (crate::mathtext::label_extents(&s.fonts, title, fs).0, row_h),
        None => (0.0, 0.0),
    };
    let entries_w = col_w.iter().sum::<f32>() + col_gap * (col_w.len() - 1) as f32;
    Some(Layout {
        rows,
        col_w,
        title_h,
        box_w: pad * 2.0 + entries_w.max(title_w),
        box_h: pad * 2.0 + title_h + row_h * rows as f32,
    })
}

/// How many of `pts` (data sample points, in pixels) the legend of `ax`
/// would cover at the best of its candidate spots on a frame at `box_`.
/// `None` when there is no legend to place.
pub(crate) fn least_covered(ax: &Axes, box_: Rect, pts: &[(f32, f32)]) -> Option<usize> {
    let l = layout(ax)?;
    let s = &ax.style;
    let margin = s.px(s.axes_linewidth_pt) + 0.5 * s.px(s.legend_size_pt);
    Loc::CANDIDATES
        .iter()
        .map(|&loc| {
            let (x, y) = loc.origin(box_, l.box_w, l.box_h, margin);
            pts.iter()
                .filter(|&&(px, py)| px >= x && px <= x + l.box_w && py >= y && py <= y + l.box_h)
                .count()
        })
        .min()
}

/// The legend's bounding rectangle within `box_` (the frame), or `None` when
/// there is nothing to draw. Shared by [`draw_legend`] and the stat box, which
/// stacks beneath the legend when both sit top-right.
pub(crate) fn legend_rect(ax: &Axes, box_: Rect) -> Option<Rect> {
    let l = layout(ax)?;
    let s = &ax.style;
    let fs = s.px(s.legend_size_pt);
    let (box_w, box_h) = (l.box_w, l.box_h);
    let margin = s.px(s.axes_linewidth_pt) + 0.5 * fs; // borderaxespad ~0.5
    let at = |loc: Loc| {
        let (x, y) = loc.origin(box_, box_w, box_h, margin);
        Rect::new(x, y, box_w, box_h)
    };
    Some(match ax.legend_location() {
        Loc::Best => {
            // matplotlib's "best": the spot covering the fewest data points;
            // ties go to the earlier candidate.
            let pts = ax.data_points_px(box_);
            // The fit-statistics box counts as more data than any plot holds:
            // a spot that would cover it is taken only if every spot does.
            // So does each text annotation: a legend over a label hides it.
            let mut keep_clear: Vec<Rect> = ax.stat_box_rect(box_).into_iter().collect();
            keep_clear.extend(ax.annotation_rects_px(box_));
            keep_clear.extend(ax.hep_label_inside_px(box_));
            let overlaps = |r: &Rect, s: &Rect| {
                r.x < s.right() && s.x < r.right() && r.y < s.bottom() && s.y < r.bottom()
            };
            let covered = |r: &Rect| {
                let hidden = keep_clear.iter().filter(|s| overlaps(r, s)).count();
                pts.iter()
                    .filter(|&&(x, y)| x >= r.x && x <= r.right() && y >= r.y && y <= r.bottom())
                    .count()
                    + hidden * (pts.len() + 1)
            };
            Loc::CANDIDATES
                .iter()
                .map(|&loc| at(loc))
                .min_by_key(covered)
                .expect("there are candidates")
        }
        loc => at(loc),
    })
}

/// Draw the legend into `g` (the unclipped axis group).
pub(crate) fn draw_legend(g: &mut DrawGroup, ax: &Axes, box_: Rect) {
    let items = ax.legend_items();
    let (Some(rect), Some(l)) = (legend_rect(ax, box_), layout(ax)) else {
        return;
    };
    let (x0, y0, box_w, box_h) = (rect.x, rect.y, rect.w, rect.h);
    let s = &ax.style;
    let fs = s.px(s.legend_size_pt);
    let pad = 0.4 * fs;
    let gap = 0.8 * fs;
    let handle_w = 2.0 * fs;
    let row_h = fs * 1.5;

    if s.legend_frame {
        // matplotlib `fancybox`: a rounded white box (framealpha 0.8) with a light
        // grey edge (legend.edgecolor "0.8") at the axes line width.
        g.push(DrawCommand::Path {
            path: rounded_rect(Rect::new(x0, y0, box_w, box_h), 0.2 * fs),
            fill: Some(Color::WHITE.with_alpha(0.8)),
            stroke: Some(Stroke::line(
                Color::hex("#cccccc"),
                s.px(s.axes_linewidth_pt),
            )),
        });
    }

    if let Some(title) = ax.legend_heading() {
        crate::mathtext::layout_label(
            g,
            &s.fonts,
            title,
            x0 + box_w / 2.0,
            y0 + pad + row_h / 2.0,
            fs,
            s.fg_color,
            HAlign::Center,
            VAlign::Middle,
            0.0,
        );
    }
    // The left edge of each column.
    let col_gap = 2.0 * fs;
    let col_x: Vec<f32> = l
        .col_w
        .iter()
        .scan(x0 + pad, |x, w| {
            let left = *x;
            *x += w + col_gap;
            Some(left)
        })
        .collect();
    for (i, it) in items.iter().enumerate() {
        let (col, row) = (i / l.rows, i % l.rows);
        let cy = y0 + pad + l.title_h + row_h * row as f32 + row_h / 2.0;
        let hx0 = col_x[col];
        let hx1 = hx0 + handle_w;
        let hcx = (hx0 + hx1) / 2.0;
        match &it.handle {
            LegendHandle::Line {
                color,
                width_pt,
                dash,
                marker,
                marker_size_pt,
            } => {
                if *width_pt > 0.0 {
                    let mut st = Stroke::new(*color, s.px(*width_pt));
                    st.dash = dash.as_ref().map(|d| d.iter().map(|v| s.px(*v)).collect());
                    g.push(DrawCommand::Line {
                        p0: (hx0, cy),
                        p1: (hx1, cy),
                        stroke: st,
                    });
                }
                if *marker != Marker::None {
                    draw_marker(
                        g,
                        *marker,
                        (hcx, cy),
                        s.px(*marker_size_pt),
                        Some(*color),
                        None,
                    );
                }
            }
            LegendHandle::Patch {
                face,
                edge,
                width_pt,
                hatch,
            } => {
                let h = fs * 0.8;
                let rect = Rect::new(hx0, cy - h / 2.0, handle_w, h);
                g.push(DrawCommand::Rect {
                    rect,
                    fill: *face,
                    stroke: edge.map(|c| Stroke::line(c, s.px(*width_pt))),
                });
                if let Some((hatch, color)) = hatch {
                    let stroke = Stroke::new(*color, s.px(0.6));
                    crate::artists::hatch_rect(g, rect, *hatch, s.px(4.5), &stroke);
                }
            }
        }
        // Labels go through the math layout so `$…$` renders like axis labels.
        crate::mathtext::layout_label(
            g,
            &s.fonts,
            &it.label,
            hx1 + gap,
            cy,
            fs,
            s.fg_color,
            HAlign::Left,
            VAlign::Middle,
            0.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CurveOpts, FontSet};

    fn legend_width(label: &str) -> f32 {
        let mut ax = Axes::new();
        ax.fonts(FontSet::stix());
        ax.plot_with(&[0.0, 1.0], &[0.0, 1.0], CurveOpts::new().label(label))
            .unwrap();
        legend_rect(&ax, Rect::new(0.0, 0.0, 600.0, 400.0))
            .unwrap()
            .w
    }

    #[test]
    fn a_math_label_is_measured_as_typeset() {
        // The TeX source is far longer than what it typesets to; the frame
        // must fit the typeset label, not the source.
        let math = legend_width(r"$\sqrt{s} = 13\,\mathrm{TeV}$");
        let source_as_text = legend_width(r"\sqrt{s} = 13\,\mathrm{TeV}");
        assert!(math < 0.7 * source_as_text, "{math} vs {source_as_text}");
    }
}

#[cfg(test)]
mod placement_tests {
    use super::*;
    use crate::CurveOpts;

    const FRAME: Rect = Rect {
        x: 80.0,
        y: 40.0,
        w: 500.0,
        h: 380.0,
    };

    fn with_line(xs: &[f64], ys: &[f64]) -> Axes {
        let mut ax = Axes::new();
        ax.plot_with(xs, ys, CurveOpts::new().label("data"))
            .unwrap();
        ax
    }

    fn is_upper_right(r: Rect) -> bool {
        r.right() > FRAME.x + 0.9 * FRAME.w && r.y < FRAME.y + 0.1 * FRAME.h
    }

    #[test]
    fn best_keeps_off_the_data() {
        // A line rising to the top-right corner: the legend goes elsewhere.
        let ax = with_line(&[0.0, 1.0, 2.0], &[0.0, 0.2, 1.0]);
        let r = legend_rect(&ax, FRAME).unwrap();
        assert!(!is_upper_right(r), "{r:?}");
        let pts = ax.data_points_px(FRAME);
        let inside = pts
            .iter()
            .filter(|&&(x, y)| x >= r.x && x <= r.right() && y >= r.y && y <= r.bottom())
            .count();
        assert_eq!(inside, 0);
    }

    #[test]
    fn a_filled_histogram_counts_its_inside() {
        // A legend over the fill hides data as much as one over the outline:
        // the samples `Best` avoids must cover the area under a filled
        // histogram, not just its top edge (they used to, so the empty-looking
        // inside of a stack attracted the legend).
        let h = crate::Binned1D::new((0..=8).map(f64::from).collect(), vec![10.0; 8]).unwrap();
        let near_bottom = |histtype| {
            let mut ax = Axes::new();
            ax.hist_with(&h, crate::HistOpts::new().histtype(histtype));
            ax.data_points_px(FRAME)
                .iter()
                .filter(|&&(_, y)| y > FRAME.y + 0.6 * FRAME.h && y < FRAME.bottom() - 1.0)
                .count()
        };
        assert!(
            near_bottom(crate::HistType::Fill) > 100,
            "the fill's inside is sampled"
        );
        assert_eq!(
            near_bottom(crate::HistType::Step),
            0,
            "an outline has no inside"
        );
    }

    #[test]
    fn best_prefers_the_upper_right_when_it_is_free() {
        let ax = with_line(&[0.0, 1.0, 2.0], &[1.0, 0.2, 0.0]);
        assert!(is_upper_right(legend_rect(&ax, FRAME).unwrap()));
    }

    #[test]
    #[cfg(feature = "fit")]
    fn best_keeps_off_the_fit_statistics_box() {
        // Data falling to the right leaves the upper right free, but the stat
        // box sits there: the legend goes elsewhere, and the box keeps its
        // corner instead of being pushed down.
        let model = oxifit::Model::gaussian("g").with_params(vec![1.0, 0.0, 1.0]);
        let result = oxifit::FitResult::new(vec![1.0, 0.0, 1.0], vec![0.1; 3], 1.0, 1, true);
        let mut ax = with_line(&[0.0, 1.0, 2.0], &[1.0, 0.2, 0.0]);
        ax.fit_stats(&model, &result);
        let stats = ax.stat_box_rect(FRAME).unwrap();
        let legend = legend_rect(&ax, FRAME).unwrap();
        let overlap = legend.x < stats.right()
            && stats.x < legend.right()
            && legend.y < stats.bottom()
            && stats.y < legend.bottom();
        assert!(!overlap, "legend {legend:?} over the stat box {stats:?}");
    }

    #[test]
    fn best_keeps_off_a_text_annotation() {
        // The upper right is free of data, but a label sits there: the legend
        // goes elsewhere.
        let mut ax = with_line(&[0.0, 1.0, 2.0], &[1.0, 0.2, 0.0]);
        ax.text_with(
            0.97,
            0.95,
            "a label in the corner",
            crate::TextOpts::new()
                .axes_coords(true)
                .align(crate::HAlign::Right, crate::VAlign::Top),
        );
        let label = ax.annotation_rects_px(FRAME)[0];
        let legend = legend_rect(&ax, FRAME).unwrap();
        let overlap = legend.x < label.right()
            && label.x < legend.right()
            && legend.y < label.bottom()
            && label.y < legend.bottom();
        assert!(!overlap, "legend {legend:?} over the label {label:?}");
    }

    #[test]
    fn an_explicit_location_is_kept() {
        let mut ax = with_line(&[0.0, 1.0, 2.0], &[1.0, 0.2, 0.0]);
        ax.legend_loc(Loc::UpperLeft);
        let r = legend_rect(&ax, FRAME).unwrap();
        assert!(r.x < FRAME.x + 0.1 * FRAME.w && r.y < FRAME.y + 0.1 * FRAME.h);
        ax.legend_loc(Loc::LowerCenter);
        let r = legend_rect(&ax, FRAME).unwrap();
        assert!((r.x + r.w / 2.0 - (FRAME.x + FRAME.w / 2.0)).abs() < 1.0);
        assert!(r.bottom() > FRAME.y + 0.9 * FRAME.h);
    }
}
