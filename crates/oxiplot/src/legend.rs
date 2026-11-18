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

/// The legend's bounding rectangle within `box_` (the frame), or `None` when
/// there is nothing to draw. Shared by [`draw_legend`] and the stat box, which
/// stacks beneath the legend when both sit top-right.
pub(crate) fn legend_rect(ax: &Axes, box_: Rect) -> Option<Rect> {
    let items = ax.legend_items();
    if items.is_empty() {
        return None;
    }
    let s = &ax.style;
    let fs = s.px(s.legend_size_pt);
    // Spacing in font-size units, matching matplotlib's legend rcParams:
    // borderpad 0.4, handlelength 2.0, handletextpad 0.8, labelspacing 0.5.
    let pad = 0.4 * fs;
    let gap = 0.8 * fs;
    let handle_w = 2.0 * fs;
    let row_h = fs * 1.5;

    let max_w = items
        .iter()
        // Measured as drawn: a `$…$` label is typeset, so its raw source
        // (backslashes, braces) says nothing about its width.
        .map(|it| crate::mathtext::label_extents(&s.fonts, &it.label, fs).0)
        .fold(0.0_f32, f32::max);

    let box_w = pad + handle_w + gap + max_w + pad;
    let box_h = pad * 2.0 + row_h * items.len() as f32;
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
            let stats = ax.stat_box_rect(box_);
            let covered = |r: &Rect| {
                let on_stats = stats.is_some_and(|s| {
                    r.x < s.right() && s.x < r.right() && r.y < s.bottom() && s.y < r.bottom()
                });
                pts.iter()
                    .filter(|&&(x, y)| x >= r.x && x <= r.right() && y >= r.y && y <= r.bottom())
                    .count()
                    + if on_stats { pts.len() + 1 } else { 0 }
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
    let Some(rect) = legend_rect(ax, box_) else {
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

    for (i, it) in items.iter().enumerate() {
        let cy = y0 + pad + row_h * i as f32 + row_h / 2.0;
        let hx0 = x0 + pad;
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
            } => {
                let h = fs * 0.8;
                g.push(DrawCommand::Rect {
                    rect: Rect::new(hx0, cy - h / 2.0, handle_w, h),
                    fill: *face,
                    stroke: edge.map(|c| Stroke::line(c, s.px(*width_pt))),
                });
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
        let result = oxifit::FitResult {
            params: vec![1.0, 0.0, 1.0],
            errors: vec![0.1, 0.1, 0.1],
            minos: None,
            covariance: None,
            chi2: 1.0,
            ndf: 1,
            valid: true,
        };
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
