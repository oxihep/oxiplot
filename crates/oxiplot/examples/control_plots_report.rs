//! A multi-page PDF of control plots, the way an analysis shares them.
//!
//! A first page with four kinematic distributions on a 2 × 2 grid, then one
//! page per distribution, all in one vector PDF (`PdfPages`, matplotlib's
//! equivalent). Each page is the figure's size in points. Run with:
//!
//! ```sh
//! cargo run -p oxiplot --example control_plots_report
//! ```

use oxiplot::{
    Axes, Binned1D, GridSpec, HistOpts, HistType, PdfPages, Scale, Style, subplots_grid_with,
};

/// `n` bins over `[lo, hi)` with contents `f(centre)`.
fn shape(n: usize, lo: f64, hi: f64, f: impl Fn(f64) -> f64) -> Binned1D {
    let w = (hi - lo) / n as f64;
    let edges: Vec<f64> = (0..=n).map(|i| lo + w * i as f64).collect();
    let values = (0..n)
        .map(|i| f(lo + w * (i as f64 + 0.5)).max(0.0))
        .collect();
    Binned1D::new(edges, values).expect("n + 1 edges")
}

fn main() -> oxiplot::Result<()> {
    let plots: [(&str, &str, Binned1D, bool); 4] = [
        (
            "lead_pt",
            "Leading lepton $p_T$ [GeV]",
            shape(40, 20.0, 220.0, |x| 1e5 * (-x / 30.0).exp()),
            true,
        ),
        (
            "lead_eta",
            "Leading lepton $\\eta$",
            shape(30, -2.5, 2.5, |x| 4000.0 * (1.0 - 0.1 * x * x)),
            false,
        ),
        (
            "mll",
            "$m_{\\ell\\ell}$ [GeV]",
            shape(40, 70.0, 110.0, |x| {
                2e4 / (1.0 + ((x - 91.19) / 1.25).powi(2)) + 200.0
            }),
            true,
        ),
        (
            "met",
            "$E_T^{miss}$ [GeV]",
            shape(30, 0.0, 150.0, |x| {
                2e4 * (-x / 15.0).exp() + 30.0 * (-x / 60.0).exp()
            }),
            true,
        ),
    ];
    let draw = |ax: &mut Axes, label: &str, h: &Binned1D, log: bool| {
        ax.hist_with(
            h,
            HistOpts::new().histtype(HistType::Fill).label("Simulation"),
        );
        ax.xlabel(label).ylabel("Events");
        if log {
            ax.yscale(Scale::Log);
        }
    };

    let mut pdf = PdfPages::new();
    // Page 1: all four together.
    let (fig, mut axs) = subplots_grid_with(Style::default(), GridSpec::new(2, 2));
    for (ax, (_, label, h, log)) in axs.iter_mut().zip(&plots) {
        draw(ax, label, h, *log);
    }
    pdf.add(&fig.suptitle("Control plots, signal region").with_axes(axs));
    // Then one page each.
    for (_, label, h, log) in &plots {
        let mut ax = Axes::with_style(Style::mplhep());
        draw(&mut ax, label, h, *log);
        ax.hep_label("Demo", "Simulation").legend();
        pdf.add_axes(&ax);
    }

    let dir = std::env::temp_dir().join("oxiplot-examples");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("control_plots.pdf");
    pdf.save(&path)?;
    println!("wrote {} ({} pages)", path.display(), pdf.len());
    Ok(())
}
