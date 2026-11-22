//! Tests of the pieces a HEP plot is made of: histogram normalisation, Poisson
//! errors, flow bins, hatching, spans, tick positions, the legend's layout and
//! the room made for it, a second y axis, comparisons of two histograms, and
//! the experiment styles.

use crate::artists::Artist;
use crate::draw::Rect;
use crate::{
    Axes, Binned1D, Color, ErrorbarOpts, FillOpts, Flow, Hatch, HepLabelLoc, Hist1dData, HistOpts,
    HistType, Marker, PointData, Style, XyPoints,
};

const FRAME: Rect = Rect {
    x: 80.0,
    y: 40.0,
    w: 500.0,
    h: 380.0,
};

fn hist(edges: &[f64], values: &[f64]) -> Binned1D {
    let errors = values.iter().map(|v| v.abs().sqrt()).collect();
    Binned1D::new(edges.to_vec(), values.to_vec())
        .unwrap()
        .with_errors(errors)
        .unwrap()
}

/// The one histogram artist of `ax`.
fn step(ax: &Axes) -> &crate::artists::StepArtist {
    match &ax.artists[0] {
        Artist::Step(a) => a,
        other => panic!("not a histogram: {other:?}"),
    }
}

fn count(svg: &str, tag: &str) -> usize {
    svg.matches(&format!("<{tag} ")).count()
}

/// A histogram that keeps what falls outside its range.
struct WithFlow(Binned1D);

impl Hist1dData for WithFlow {
    fn edges(&self) -> Vec<f64> {
        self.0.edges()
    }
    fn values(&self) -> Vec<f64> {
        self.0.values()
    }
    fn error(&self, i: usize) -> f64 {
        self.0.error(i)
    }
    fn underflow(&self) -> Option<(f64, f64)> {
        Some((5.0, 3.0))
    }
    fn overflow(&self) -> Option<(f64, f64)> {
        Some((7.0, 4.0))
    }
}

#[test]
fn per_width_and_density_scale_values_and_errors_alike() {
    // Bins 1, 3 and 4 wide holding 10, 30 and 60.
    let h = hist(&[0.0, 1.0, 4.0, 8.0], &[10.0, 30.0, 60.0]);

    let mut ax = Axes::new();
    ax.hist_with(&h, HistOpts::new().per_width(true).yerr(true));
    let a = step(&ax);
    assert_eq!(a.values, [10.0, 10.0, 15.0]);
    let errs = a.errs.as_ref().unwrap();
    assert!((errs[1].0 - 30.0_f64.sqrt() / 3.0).abs() < 1e-12);
    assert_eq!(errs[1].0, errs[1].1);

    let mut ax = Axes::new();
    ax.hist_with(&h, HistOpts::new().density(true));
    let a = step(&ax);
    // Unit area: the bins times their widths sum to one.
    let area: f64 = a
        .values
        .iter()
        .zip(a.edges.windows(2))
        .map(|(v, e)| v * (e[1] - e[0]))
        .sum();
    assert!((area - 1.0).abs() < 1e-12, "{area}");
    assert!((a.values[0] - 0.1).abs() < 1e-12);

    // A density is far below one: the axis still follows it.
    let (_, top) = ax.resolved_ylim();
    assert!(top < 0.2, "the y axis runs to {top}");
}

#[test]
fn poisson_errors_are_the_exact_interval_of_a_count() {
    let h = hist(&[0.0, 1.0, 2.0, 3.0], &[0.0, 4.0, 2.5]);
    let mut ax = Axes::new();
    ax.hist_with(
        &h,
        HistOpts::new()
            .histtype(HistType::Errorbar)
            .poisson_errors(true),
    );
    let errs = step(&ax).errs.clone().unwrap();
    // No events: nothing below, the upper limit above.
    assert_eq!(errs[0].0, 0.0);
    assert!((errs[0].1 - 1.841).abs() < 2e-3);
    // Four events: 2.086 to 7.163, an interval longer above than below.
    assert!((4.0 - errs[1].0 - 2.086).abs() < 2e-3, "{:?}", errs[1]);
    assert!((4.0 + errs[1].1 - 7.163).abs() < 2e-3, "{:?}", errs[1]);
    assert!(errs[1].1 > errs[1].0);
    // A weighted bin is not a count: it keeps its own error.
    assert_eq!(errs[2], (2.5_f64.sqrt(), 2.5_f64.sqrt()));
}

#[test]
fn an_errorbar_histogram_leaves_its_empty_bins_out_unless_asked() {
    let h = hist(&[0.0, 1.0, 2.0, 3.0, 4.0], &[3.0, 0.0, 5.0, 0.0]);
    let draw = |opts: HistOpts| {
        let mut ax = Axes::new();
        ax.hist_with(&h, opts.histtype(HistType::Errorbar));
        count(&ax.to_svg_string(), "circle")
    };
    assert_eq!(draw(HistOpts::new()), 2);
    assert_eq!(draw(HistOpts::new().empty_bins(true)), 4);
}

#[test]
fn flow_bins_are_hidden_summed_or_drawn_beside_the_axis() {
    let h = WithFlow(hist(&[0.0, 1.0, 2.0, 4.0], &[10.0, 20.0, 30.0]));
    let drawn = |flow: Flow| {
        let mut ax = Axes::new();
        ax.hist_with(&h, HistOpts::new().flow(flow).yerr(true));
        let a = step(&ax);
        (a.edges.clone(), a.values.clone(), a.errs.clone().unwrap())
    };

    let (edges, values, _) = drawn(Flow::Hide);
    assert_eq!((edges.len(), values), (4, vec![10.0, 20.0, 30.0]));

    let (edges, values, errs) = drawn(Flow::Sum);
    assert_eq!((edges.len(), &values), (4, &vec![15.0, 20.0, 37.0]));
    // Uncertainties add in quadrature.
    assert!((errs[0].0 - 10.0_f64.sqrt().hypot(3.0)).abs() < 1e-12);

    // Shown: one more bin at each end, as wide as its neighbour.
    let (edges, values, errs) = drawn(Flow::Show);
    assert_eq!(edges, [-1.0, 0.0, 1.0, 2.0, 4.0, 6.0]);
    assert_eq!(values, [5.0, 10.0, 20.0, 30.0, 7.0]);
    assert_eq!((errs[0].0, errs[4].0), (3.0, 4.0));

    // Data that keeps no flow bins is drawn the same either way.
    let plain = hist(&[0.0, 1.0, 2.0], &[1.0, 2.0]);
    let mut ax = Axes::new();
    ax.hist_with(&plain, HistOpts::new().flow(Flow::Show));
    assert_eq!(step(&ax).values, [1.0, 2.0]);
}

#[test]
fn a_hatched_band_is_lines_with_no_fill_behind_them() {
    let h = hist(&[0.0, 1.0, 2.0, 3.0], &[100.0, 80.0, 60.0]);
    let svg = |opts: HistOpts| {
        let mut ax = Axes::new();
        ax.hist_with(&h, opts.histtype(HistType::Band));
        ax.to_svg_string()
    };
    let plain = svg(HistOpts::new());
    let hatched = svg(HistOpts::new().hatch(Hatch::Diagonal));
    let crossed = svg(HistOpts::new().hatch(Hatch::Cross));
    assert!(plain.contains("fill-opacity=\"0.3"));
    assert!(
        !hatched.contains("fill-opacity=\"0.3"),
        "a hatched band is not filled"
    );
    let lines = |s: &str| count(s, "line");
    assert!(lines(&hatched) > lines(&plain) + 10, "no hatch lines");
    // Crossed: about twice the lines.
    assert!(lines(&crossed) - lines(&plain) > 3 * (lines(&hatched) - lines(&plain)) / 2);
}

#[test]
fn a_span_shades_the_frame_without_moving_the_limits() {
    let mut ax = Axes::new();
    ax.plot(&[100.0, 160.0], &[9.0, 2.0]).unwrap();
    let before = (ax.resolved_xlim(), ax.resolved_ylim());
    ax.axvspan(120.0, 130.0);
    ax.axhspan_with(
        -50.0..50.0,
        FillOpts::new()
            .hollow(true)
            .hatch(Hatch::BackDiagonal)
            .label("band"),
    );
    ax.axvspan_with(500.0..600.0, FillOpts::new().color(Color::BLACK));
    assert_eq!((ax.resolved_xlim(), ax.resolved_ylim()), before);
    // Only the labelled span is in the legend.
    let labels: Vec<String> = ax.legend_items().into_iter().map(|i| i.label).collect();
    assert_eq!(labels, ["band"]);
    assert!(ax.to_svg_string().contains("<rect"));
}

#[test]
fn ticks_go_where_the_caller_puts_them() {
    let mut ax = Axes::new();
    ax.plot(&[0.0, 10.0], &[0.0, 2.5]).unwrap();
    ax.xticks(&[2.0, 8.0, 5.0, 99.0]).yticks(&[0.5, 1.0]);
    let ticks = ax.tick_layout(FRAME);
    // In order, and only those on the axis.
    assert_eq!(ticks.xticks, [2.0, 5.0, 8.0]);
    assert_eq!(ticks.xlabels, ["2", "5", "8"]);
    assert_eq!(ticks.yticks, [0.5, 1.0]);
    assert_eq!(ticks.ylabels, ["0.5", "1.0"]);
}

#[test]
fn a_legend_lays_out_in_columns_under_a_title() {
    let legend = |ncol: usize, title: bool| {
        let mut ax = Axes::new();
        for i in 0..6 {
            ax.errorbar_with(
                &XyPoints::new(vec![0.0, 1.0], vec![f64::from(i), f64::from(i)]).unwrap(),
                ErrorbarOpts::new().label(format!("series {i}")),
            );
        }
        ax.legend_ncol(ncol);
        if title {
            ax.legend_title("Six of them");
        }
        ax.legend();
        crate::legend::legend_rect(&ax, FRAME).unwrap()
    };
    let (one, three) = (legend(1, false), legend(3, false));
    // Three columns of two rows: wider, and a third of the rows' height.
    assert!(three.w > 2.0 * one.w);
    assert!(three.h < 0.6 * one.h);
    // A title is one more row.
    let titled = legend(3, true);
    assert!(titled.h > three.h && titled.w >= three.w);
    // More columns than entries: one row.
    assert!(legend(40, false).h <= three.h);
}

#[test]
fn room_is_made_above_data_that_leaves_the_legend_none() {
    // A flat filled histogram covers the frame from side to side and from
    // the axis up: no spot is clear.
    let edges: Vec<f64> = (0..=20).map(f64::from).collect();
    let flat = hist(&edges, &[100.0; 20]);
    let top = |legend: bool, fixed: bool| {
        let mut ax = Axes::new();
        // Four entries: a legend a few lines tall.
        for i in 0..4 {
            ax.hist_with(
                &flat,
                HistOpts::new()
                    .histtype(HistType::Fill)
                    .label(format!("flat {i}")),
            );
        }
        if legend {
            ax.legend();
        }
        if fixed {
            ax.ylim(0.0..105.0);
        }
        ax.resolved_ylim().1
    };
    let plain = top(false, false);
    assert!(top(true, false) > 1.1 * plain, "no room was made");
    // A range the caller set is kept.
    assert_eq!(top(true, true), 105.0);

    // A peak leaves the corners free: nothing changes.
    let peaked: Vec<f64> = (0..20)
        .map(|i| 100.0 * (-0.5 * ((f64::from(i) - 9.5) / 2.0).powi(2)).exp())
        .collect();
    let peak = hist(&edges, &peaked);
    let range = |legend: bool| {
        let mut ax = Axes::new();
        ax.hist_with(&peak, HistOpts::new().label("peak"));
        if legend {
            ax.legend();
        }
        ax.resolved_ylim()
    };
    assert_eq!(range(true), range(false));
}

#[test]
fn a_second_y_axis_shares_x_and_takes_room_on_the_right() {
    let mut ax = Axes::new();
    ax.plot_with(
        &[0.0, 1.0, 2.0],
        &[10.0, 40.0, 20.0],
        crate::CurveOpts::new().label("left"),
    )
    .unwrap();
    let alone = ax.insets(FRAME).right;
    let (xlo, xhi) = ax.resolved_xlim();

    let right = ax.twinx();
    right
        .plot_with(
            &[0.0, 5.0],
            &[0.0, 1.0],
            crate::CurveOpts::new().label("right"),
        )
        .unwrap();
    right.ylabel("fraction").ylim(0.0..1.0);

    // Its tick labels and label take room right of the frame.
    assert!(ax.insets(FRAME).right > alone + 20.0);
    // Its data widens the shared x range, and leaves the left y range alone.
    let (lo, hi) = ax.resolved_xlim();
    assert!(lo <= xlo && hi > xhi + 2.0, "x range {lo}..{hi}");
    assert!(ax.resolved_ylim().1 < 50.0);
    // One legend holds both, and its colours carry on from this axes' cycle.
    let labels: Vec<String> = ax.legend_items().into_iter().map(|i| i.label).collect();
    assert_eq!(labels, ["left", "right"]);
    // Calling it again returns the same axis.
    assert_eq!(ax.twinx().artists.len(), 1);
    assert!(ax.to_svg_string().len() > 1000);
}

#[test]
fn two_histograms_compare_bin_by_bin() {
    let edges = [0.0, 1.0, 2.0, 3.0];
    let data = hist(&edges, &[90.0, 0.0, 16.0]);
    let model = hist(&edges, &[100.0, 0.0, 25.0]);

    // A ratio has no point where the denominator is empty.
    let ratio = XyPoints::ratio(&data, &model).unwrap();
    assert_eq!(ratio.xs(), [0.5, 2.5]);
    assert_eq!(ratio.ys(), [0.9, 0.64]);
    let (lo, hi) = ratio.y_errors().unwrap();
    assert!((lo[0] - 90.0_f64.sqrt() / 100.0).abs() < 1e-12);
    assert_eq!(lo, hi);

    // A pull is in units of the combined uncertainty, with no error bar.
    let pull = XyPoints::pull(&data, &model).unwrap();
    assert_eq!(pull.xs(), [0.5, 2.5]);
    assert!((pull.ys()[0] + 10.0 / 190.0_f64.sqrt()).abs() < 1e-12);
    assert!(pull.y_errors().is_none());

    let difference = XyPoints::difference(&data, &model).unwrap();
    assert_eq!(difference.ys(), [-10.0, 0.0, -9.0]);
    assert!((difference.y_errors().unwrap().0[2] - 41.0_f64.sqrt()).abs() < 1e-12);

    // The denominator's own uncertainty, as a band around one.
    let band = Binned1D::relative_uncertainty(&model);
    assert_eq!(band.values(), [1.0, 1.0, 1.0]);
    assert_eq!(
        (band.error(0), band.error(1), band.error(2)),
        (0.1, 0.0, 0.2)
    );

    // Another binning is refused.
    let other = hist(&[0.0, 1.0, 2.0, 3.5], &[1.0, 1.0, 1.0]);
    assert!(matches!(
        XyPoints::ratio(&data, &other),
        Err(crate::Error::BinningMismatch(_))
    ));
}

#[test]
fn each_experiment_style_puts_its_label_in_its_place() {
    let rect = |style: Style| {
        let mut ax = Axes::with_style(style);
        ax.hep_label("Demo", "Internal");
        ax.hep_label_rect(FRAME).unwrap()
    };
    let (cms, atlas, lhcb) = (
        rect(Style::cms()),
        rect(Style::atlas()),
        rect(Style::lhcb()),
    );
    assert_eq!(Style::cms().hep_label_loc, HepLabelLoc::Above);
    assert!(cms.bottom() < FRAME.y, "CMS writes above the frame");
    assert!(
        atlas.y > FRAME.y && atlas.x > FRAME.x,
        "ATLAS inside, at the left"
    );
    assert!(lhcb.y > FRAME.y && lhcb.right() < FRAME.right());
    assert!(
        lhcb.x > FRAME.x + FRAME.w / 2.0,
        "LHCb inside, at the right"
    );
    assert_eq!(Style::alice().hep_label_loc, HepLabelLoc::InsideLeft);

    // A label inside the frame takes no row above it, and is kept clear of
    // the data: the axis grows until nothing is under it.
    let top = |style: Style| {
        let mut ax = Axes::with_style(style);
        let edges: Vec<f64> = (0..=20).map(f64::from).collect();
        ax.hist(&hist(&edges, &[100.0; 20]));
        ax.hep_label("Demo", "Internal");
        (ax.insets(FRAME).top, ax.resolved_ylim().1)
    };
    let ((cms_row, cms_top), (atlas_row, atlas_top)) = (top(Style::cms()), top(Style::atlas()));
    assert!(cms_row > 0.0 && atlas_row == 0.0);
    assert!(atlas_top > 1.05 * cms_top);
}

#[test]
fn every_marker_draws_and_an_open_one_is_an_outline() {
    let svg = |marker: Marker| {
        let mut ax = Axes::new();
        ax.errorbar_with(
            &XyPoints::new(vec![0.0, 1.0, 2.0], vec![1.0, 2.0, 3.0]).unwrap(),
            ErrorbarOpts::new()
                .marker(marker)
                .color(Color::rgb(200, 30, 30)),
        );
        ax.to_svg_string()
    };
    let none = svg(Marker::None).len();
    for marker in [
        Marker::Circle,
        Marker::Square,
        Marker::TriangleUp,
        Marker::TriangleDown,
        Marker::Diamond,
        Marker::Cross,
        Marker::Plus,
        Marker::Star,
        Marker::OpenCircle,
        Marker::OpenSquare,
        Marker::OpenTriangleUp,
        Marker::OpenTriangleDown,
        Marker::OpenDiamond,
    ] {
        assert!(svg(marker).len() > none, "{marker:?} drew nothing");
    }
    // Solid: three filled circles. Open: three circles with no fill.
    let filled = |s: &str| {
        s.split("<circle ")
            .skip(1)
            .filter(|c| !c.split('>').next().unwrap().contains("fill=\"none\""))
            .count()
    };
    assert_eq!(filled(&svg(Marker::Circle)), 3);
    assert_eq!(filled(&svg(Marker::OpenCircle)), 0);
    assert_eq!(count(&svg(Marker::OpenCircle), "circle"), 3);
}
