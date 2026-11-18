//! Golden-image tests: a few complete plots rendered to PNG and compared with
//! reference images in `tests/golden/`, so a change anywhere in the pipeline
//! (layout, ticks, text, colours, rasterisation) that alters a picture fails a
//! test instead of going unnoticed.
//!
//! The comparison allows small differences: anti-aliased edges may round
//! differently on another CPU. A pixel counts as different when a channel
//! moves by more than [`CHANNEL_TOLERANCE`]; a render may differ in up to
//! [`MAX_DIFFERENT`] of its pixels.
//!
//! After an intended change, re-record the references and review them:
//!
//! ```sh
//! OXIPLOT_BLESS=1 cargo test -p oxiplot --test it golden
//! ```
#![cfg(feature = "png")]

use std::path::PathBuf;

use oxiplot::{
    Axes, Binned1D, Binned2D, Color, CurveOpts, ErrorbarOpts, Figure, Hist1dData, Hist2dOpts,
    HistOpts, HistType, Points, SaveOpts, Scale, Style, ratio_subplots_with, subplots_grid,
};

/// A channel may move by this much (of 255) before a pixel counts as changed.
const CHANNEL_TOLERANCE: u8 = 48;
/// The share of pixels that may change: about 60 of a 640 × 480 render.
/// Changing one letter of an axis label moves several hundred, so any
/// visible change fails; anti-aliasing noise between CPUs stays well below.
const MAX_DIFFERENT: f64 = 0.0002;

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{name}.png"))
}

/// Compare `png` with the reference `name`, or record it with `OXIPLOT_BLESS=1`.
fn check(name: &str, png: &[u8]) {
    let path = golden_path(name);
    if std::env::var_os("OXIPLOT_BLESS").is_some() {
        std::fs::write(&path, png).unwrap();
        return;
    }
    let reference = std::fs::read(&path)
        .unwrap_or_else(|_| panic!("no reference image {path:?}: record it with OXIPLOT_BLESS=1"));
    let (got, want) = (
        tiny_skia::Pixmap::decode_png(png).unwrap(),
        tiny_skia::Pixmap::decode_png(&reference).unwrap(),
    );
    assert_eq!(
        (got.width(), got.height()),
        (want.width(), want.height()),
        "{name}: the image size changed"
    );
    let different = got
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(want.data().as_chunks::<4>().0.iter())
        .filter(|(a, b)| {
            a.iter()
                .zip(*b)
                .any(|(x, y)| x.abs_diff(*y) > CHANNEL_TOLERANCE)
        })
        .count();
    let share = different as f64 / f64::from(got.width() * got.height());
    if share > MAX_DIFFERENT {
        let out = std::env::temp_dir().join(format!("oxiplot-golden-{name}.png"));
        std::fs::write(&out, png).unwrap();
        panic!(
            "{name}: {different} pixels ({:.2} %) differ from {path:?}; the new render is {out:?}. \
             If the change is intended, re-record with OXIPLOT_BLESS=1.",
            100.0 * share
        );
    }
}

fn edges(n: usize, lo: f64, hi: f64) -> Vec<f64> {
    (0..=n)
        .map(|i| lo + (hi - lo) * i as f64 / n as f64)
        .collect()
}

fn centres(e: &[f64]) -> Vec<f64> {
    e.windows(2).map(|w| 0.5 * (w[0] + w[1])).collect()
}

fn gaussian(n: usize, lo: f64, hi: f64, mean: f64, sigma: f64, height: f64) -> Vec<f64> {
    centres(&edges(n, lo, hi))
        .iter()
        .map(|x| height * (-0.5 * ((x - mean) / sigma).powi(2)).exp())
        .collect()
}

fn png(fig: &Figure) -> Vec<u8> {
    fig.to_png_bytes(SaveOpts::new()).unwrap()
}

#[test]
fn golden_histogram_with_data_and_legend() {
    let e = edges(30, 60.0, 120.0);
    let mc = Binned1D::new(e.clone(), gaussian(30, 60.0, 120.0, 91.0, 6.0, 900.0)).unwrap();
    let data: Vec<f64> = mc.values().iter().map(|v| (v * 1.03).round()).collect();
    let mut ax = Axes::new();
    ax.hist_with(&mc, HistOpts::new().histtype(HistType::Fill).label("MC"));
    ax.errorbar_with(
        &Points::new(centres(&e), data.clone())
            .unwrap()
            .with_y_errors(data.iter().map(|d| d.sqrt()).collect())
            .unwrap(),
        ErrorbarOpts::new().color(Color::BLACK).label("data"),
    );
    ax.xlabel("$m_{\\mu\\mu}$ [GeV]")
        .ylabel("Events / 2 GeV")
        .title("$Z \\to \\mu\\mu$");
    ax.legend();
    check("histogram", &ax.to_png_bytes(SaveOpts::new()).unwrap());
}

#[test]
fn golden_log_ratio_plot() {
    let e = edges(25, 0.0, 500.0);
    let falling: Vec<f64> = centres(&e)
        .iter()
        .map(|x| 1e6 * (-x / 60.0).exp())
        .collect();
    let (fig, mut main, mut ratio) = ratio_subplots_with(Style::mplhep());
    main.hist_with(
        &Binned1D::new(e.clone(), falling).unwrap(),
        HistOpts::new().histtype(HistType::Fill).label("background"),
    );
    main.yscale(Scale::Log)
        .ylabel("Events")
        .hep_label("Demo", "Simulation")
        .hep_rhs("$\\sqrt{s} = 13$ TeV")
        .legend();
    ratio.function_with(|_| 1.0, 0.0..500.0, CurveOpts::new().dashed(vec![4.0, 2.0]));
    ratio.ylim(0.5..1.5).ylabel("Ratio").xlabel("$p_T$ [GeV]");
    check("log_ratio", &png(&fig.ratio(main, ratio)));
}

#[test]
fn golden_heatmaps() {
    let field = |n: usize| {
        let e = edges(n, -3.0, 3.0);
        let c = centres(&e);
        let grid = c
            .iter()
            .map(|x| {
                c.iter()
                    .map(|y| 100.0 * (-(x * x + y * y) / 2.0).exp())
                    .collect()
            })
            .collect();
        Binned2D::new(e.clone(), e, grid).unwrap()
    };
    // Small: a rectangle per bin. Large: one image.
    let (fig, mut axs) = subplots_grid(1, 2);
    axs[0].hist2d_with(&field(12), Hist2dOpts::new().label("entries"));
    axs[1].hist2d_with(&field(100), Hist2dOpts::new().log());
    axs[0].title("12 × 12");
    axs[1].title("100 × 100, log");
    check("heatmaps", &png(&fig.with_axes(axs)));
}

#[test]
fn golden_grid_with_titles() {
    let (fig, mut axs) = subplots_grid(2, 2);
    for (k, ax) in axs.iter_mut().enumerate() {
        let e = edges(10, 0.0, 1000.0);
        let v = gaussian(10, 0.0, 1000.0, 500.0, 200.0, 12345.0 * (k + 1) as f64);
        ax.hist(&Binned1D::new(e, v).unwrap());
        ax.title(format!("Panel {k}"))
            .xlabel("$p_T$ [GeV]")
            .ylabel("Events");
    }
    check("grid", &png(&fig.suptitle("Four panels").with_axes(axs)));
}

#[test]
fn golden_log_log_line() {
    let xs: Vec<f64> = (0..60)
        .map(|i| 10f64.powf(f64::from(i) / 10.0 - 1.0))
        .collect();
    let ys: Vec<f64> = xs.iter().map(|x| 3.0 / (x * x) + 0.01).collect();
    let mut ax = Axes::new();
    ax.plot_with(&xs, &ys, CurveOpts::new().label("$3/x^2 + 0.01$"))
        .unwrap();
    ax.xscale(Scale::Log)
        .yscale(Scale::Log)
        .grid_minor()
        .legend()
        .xlabel("$x$")
        .ylabel("$y$");
    check("log_log", &ax.to_png_bytes(SaveOpts::new()).unwrap());
}
