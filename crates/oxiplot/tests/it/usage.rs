//! End-to-end usage: the plots an analysis makes, built only through the
//! public API, checked for a well-formed output whatever the data holds.

use oxiplot::{
    Axes, Binned1D, Binned2D, Color, CurveOpts, ErrorbarOpts, Figure, GridSpec, Hist2dOpts,
    HistOpts, HistType, Norm, Points, Style, ratio_subplots_with, subplots_grid_with,
};

fn edges(n: usize, lo: f64, hi: f64) -> Vec<f64> {
    (0..=n)
        .map(|i| lo + (hi - lo) * i as f64 / n as f64)
        .collect()
}

/// The SVG is complete and every number in it is a number.
fn assert_valid_svg(svg: &str) {
    assert!(svg.starts_with("<svg") && svg.trim_end().ends_with("</svg>"));
    for bad in ["NaN", "inf"] {
        assert!(!svg.contains(bad), "the SVG holds `{bad}`");
    }
}

fn assert_valid_pdf(pdf: &[u8]) {
    assert!(pdf.starts_with(b"%PDF-") && pdf.ends_with(b"%%EOF\n"));
    let text = String::from_utf8_lossy(pdf);
    assert!(!text.contains("NaN") && !text.contains("inf"));
}

#[test]
fn data_over_stacked_backgrounds_with_a_ratio_panel() {
    let e = edges(20, 0.0, 200.0);
    let bkg: Vec<f64> = (0..20).map(|i| 400.0 * (-(i as f64) / 6.0).exp()).collect();
    let sig: Vec<f64> = (0..20)
        .map(|i| 60.0 * (-((i as f64 - 10.0) / 2.0).powi(2)).exp())
        .collect();
    let total: Vec<f64> = bkg.iter().zip(&sig).map(|(b, s)| b + s).collect();
    let data: Vec<f64> = total.iter().map(|t| t.round()).collect();
    let centers: Vec<f64> = e.windows(2).map(|w| 0.5 * (w[0] + w[1])).collect();

    let (fig, mut main, mut ratio) = ratio_subplots_with(Style::mplhep());
    main.hep_label("Demo", "Simulation")
        .hep_rhs(r"$\sqrt{s} = 13$ TeV");
    // A stack is drawn as cumulative fills, the tallest first.
    main.hist_with(
        &Binned1D::new(e.clone(), total.clone()).unwrap(),
        HistOpts::new().histtype(HistType::Fill).label("signal"),
    );
    main.hist_with(
        &Binned1D::new(e.clone(), bkg.clone()).unwrap(),
        HistOpts::new().histtype(HistType::Fill).label("background"),
    );
    main.hist_with(
        &Binned1D::new(e.clone(), total.clone()).unwrap(),
        HistOpts::new()
            .histtype(HistType::Band)
            .color(Color::BLACK)
            .label("stat. unc."),
    );
    let pts = Points::new(centers.clone(), data.clone())
        .unwrap()
        .with_y_errors(data.iter().map(|d| d.sqrt()).collect())
        .unwrap();
    main.errorbar_with(&pts, ErrorbarOpts::new().color(Color::BLACK).label("data"));
    main.ylabel("Events / 10 GeV").legend();

    let r: Vec<f64> = data.iter().zip(&total).map(|(d, t)| d / t).collect();
    let rerr: Vec<f64> = data.iter().zip(&total).map(|(d, t)| d.sqrt() / t).collect();
    ratio.errorbar_with(
        &Points::new(centers, r)
            .unwrap()
            .with_y_errors(rerr)
            .unwrap(),
        ErrorbarOpts::new().color(Color::BLACK),
    );
    ratio.function_with(|_| 1.0, 0.0..200.0, CurveOpts::new().dashed(vec![4.0, 2.0]));
    ratio
        .ylim(0.5..1.5)
        .ylabel("data / pred.")
        .xlabel("$m$ [GeV]");

    let fig = fig.ratio(main, ratio);
    if let Some(dir) = std::env::var_os("OXIPLOT_USAGE_OUT") {
        let dir = std::path::PathBuf::from(dir);
        fig.save(dir.join("usage_ratio.png")).unwrap();
    }
    assert_valid_svg(&fig.to_svg_string());
    assert_valid_pdf(&fig.to_pdf_bytes());
}

#[test]
fn empty_and_missing_bins_render() {
    // Empty data, a NaN bin, an infinite bin, NaN errors and NaN points: the
    // output stays valid and the finite data still shows.
    let mut ax = Axes::new();
    ax.hist(&Binned1D::new(vec![0.0], vec![]).unwrap());
    ax.hist_with(
        &Binned1D::new(vec![0.0, 1.0, 2.0, 3.0], vec![f64::NAN, 3.0, f64::INFINITY])
            .unwrap()
            .with_errors(vec![f64::NAN, 1.0, 1.0])
            .unwrap(),
        HistOpts::new().yerr(),
    );
    for histtype in [HistType::Fill, HistType::Band, HistType::Errorbar] {
        ax.hist_with(
            &Binned1D::new(vec![0.0, 1.0, 2.0], vec![f64::NAN, 2.0]).unwrap(),
            HistOpts::new().histtype(histtype),
        );
    }
    ax.errorbar_with(
        &Points::new(vec![0.5, f64::NAN, 1.5], vec![1.0, 2.0, f64::NEG_INFINITY]).unwrap(),
        ErrorbarOpts::new().line(),
    );
    ax.function(|x| 1.0 / x, -1.0..1.0);
    ax.legend();
    assert_valid_svg(&ax.to_svg_string());
    assert_valid_pdf(&ax.to_pdf_bytes());
}

#[test]
fn degenerate_limits_render() {
    let mut ax = Axes::new();
    ax.plot(&[1.0, 1.0], &[2.0, 2.0]).unwrap();
    assert_valid_svg(&ax.to_svg_string());

    let mut ax = Axes::new();
    ax.plot(&[0.0, 1.0], &[0.0, 1.0]).unwrap();
    ax.xlim(3.0..3.0).ylim(f64::NAN..f64::INFINITY);
    let svg = ax.to_svg_string();
    assert_valid_svg(&svg);
    // A NaN range once produced a megabyte of NaN tick paths.
    assert!(svg.len() < 200_000, "{} bytes", svg.len());
}

#[test]
fn heatmaps_with_holes_and_extremes() {
    let e = edges(4, 0.0, 4.0);
    let grid = vec![
        vec![0.0, 1.0, 10.0, 100.0],
        vec![f64::NAN, -3.0, 1e6, 0.5],
        vec![f64::INFINITY, 2.0, 3.0, 4.0],
        vec![5.0; 4],
    ];
    let h = Binned2D::new(e.clone(), e, grid).unwrap();
    for opts in [
        Hist2dOpts::new(),
        Hist2dOpts::new().log().label("entries"),
        Hist2dOpts::new().norm(Norm::SymLog { linthresh: 1.0 }),
        Hist2dOpts::new().vrange(10.0..1.0),
    ] {
        let mut ax = Axes::new();
        ax.hist2d_with(&h, opts);
        assert_valid_svg(&ax.to_svg_string());
    }
}

#[test]
fn a_shared_grid_of_panels() {
    let style = Style::default();
    let grid = GridSpec::new(2, 2).wspace(0.05).hspace(0.05);
    let (fig, mut axs) = subplots_grid_with(style, grid);
    for (k, ax) in axs.iter_mut().enumerate() {
        let k = k as f64;
        ax.plot_with(
            &[0.0, 1.0, 2.0],
            &[k, k + 1.0, k * k],
            CurveOpts::new().label(format!("panel {k}")),
        )
        .unwrap();
        ax.legend().grid();
    }
    let fig: Figure = fig.sharex().sharey().suptitle("Four panels").with_axes(axs);
    assert_valid_svg(&fig.to_svg_string());
}

#[test]
fn large_and_tiny_values_label_cleanly() {
    let mut ax = Axes::new();
    ax.plot(&[100000.0, 100001.0], &[1.0e-5, 1.1e-5]).unwrap();
    assert_valid_svg(&ax.to_svg_string());
}
