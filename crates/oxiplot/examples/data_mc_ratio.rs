//! The classic HEP control plot: data over stacked simulation, with a ratio.
//!
//! Drell–Yan, tt̄ and diboson backgrounds are stacked (mplhep's
//! `histplot(stack=True)`), a signal sits on top, the simulation's
//! statistical uncertainty is a hatched-style band, and the data are points
//! with Poisson errors. The lower panel shows data / prediction. The y axis is
//! logarithmic, as for any falling spectrum. Run with:
//!
//! ```sh
//! cargo run -p oxiplot --example data_mc_ratio
//! ```

use oxiplot::{
    Binned1D, Color, CurveOpts, ErrorbarOpts, HistOpts, HistType, Scale, StackOpts, Style,
    XyPoints, ratio_subplots_with,
};

/// Bin edges: 25 bins of 20 GeV.
fn edges() -> Vec<f64> {
    (0..=25).map(|i| 20.0 * f64::from(i)).collect()
}

/// A falling shape integrated per bin: `norm · exp(−m / scale)`.
fn falling(norm: f64, scale: f64) -> Vec<f64> {
    edges()
        .windows(2)
        .map(|w| norm * scale * ((-w[0] / scale).exp() - (-w[1] / scale).exp()))
        .collect()
}

/// A Gaussian peak integrated per bin (midpoint rule is fine at this width).
fn peak(n: f64, mean: f64, sigma: f64) -> Vec<f64> {
    edges()
        .windows(2)
        .map(|w| {
            let x = 0.5 * (w[0] + w[1]);
            n * (w[1] - w[0]) / (sigma * (2.0 * std::f64::consts::PI).sqrt())
                * (-0.5 * ((x - mean) / sigma).powi(2)).exp()
        })
        .collect()
}

fn main() -> oxiplot::Result<()> {
    let e = edges();
    let centres: Vec<f64> = e.windows(2).map(|w| 0.5 * (w[0] + w[1])).collect();
    let drell_yan = falling(2.0e5, 35.0);
    let ttbar = falling(4.0e3, 80.0);
    let diboson = falling(600.0, 110.0);
    let signal = peak(20000.0, 300.0, 25.0);
    let total: Vec<f64> = (0..centres.len())
        .map(|i| drell_yan[i] + ttbar[i] + diboson[i] + signal[i])
        .collect();
    // "Data": the prediction with a deterministic ±1σ-ish wiggle, so the
    // example needs no random numbers.
    let data: Vec<f64> = total
        .iter()
        .enumerate()
        .map(|(i, t)| (t + t.sqrt() * (i as f64 * 1.7).sin()).round().max(0.0))
        .collect();

    let (fig, mut main, mut ratio) = ratio_subplots_with(Style::mplhep());
    let layers = [
        Binned1D::new(e.clone(), diboson.clone())?,
        Binned1D::new(e.clone(), ttbar.clone())?,
        Binned1D::new(e.clone(), drell_yan.clone())?,
        Binned1D::new(e.clone(), signal.clone())?,
    ];
    main.hist_stack_with(
        &[&layers[0], &layers[1], &layers[2], &layers[3]],
        StackOpts::new()
            .labels([
                "Diboson",
                "$t\\bar{t}$",
                "Drell–Yan",
                "Signal ($m = 300$ GeV)",
            ])
            .colors([
                Color::hex("#92dadd"),
                Color::hex("#e76300"),
                Color::hex("#3f90da"),
                Color::hex("#bd1f01"),
            ]),
    )?;
    main.hist_with(
        &Binned1D::new(e.clone(), total.clone())?,
        HistOpts::new()
            .histtype(HistType::Band)
            .color(Color::BLACK)
            .label("MC stat. unc."),
    );
    let data_pts = XyPoints::new(centres.clone(), data.clone())?
        .with_y_errors(data.iter().map(|d| d.sqrt()).collect())?;
    main.errorbar_with(
        &data_pts,
        ErrorbarOpts::new().color(Color::BLACK).label("Data"),
    );
    main.yscale(Scale::Log)
        // Headroom for the legend above the spectrum, as is usual on a log axis.
        .ylim(0.5..1e8)
        .ylabel("Events / 20 GeV")
        .hep_label("Demo", "Simulation")
        .hep_rhs("138 fb$^{-1}$ (13 TeV)")
        .legend();

    let r: Vec<f64> = data.iter().zip(&total).map(|(d, t)| d / t).collect();
    let r_err: Vec<f64> = data.iter().zip(&total).map(|(d, t)| d.sqrt() / t).collect();
    ratio.errorbar_with(
        &XyPoints::new(centres, r)?.with_y_errors(r_err)?,
        ErrorbarOpts::new().color(Color::BLACK),
    );
    ratio.function_with(
        |_| 1.0,
        0.0..500.0,
        CurveOpts::new()
            .color(Color::hex("#888888"))
            .dashed(vec![4.0, 2.0]),
    );
    ratio
        .ylim(0.5..1.5)
        .ylabel("Data / MC")
        .xlabel("$m_{\\ell\\ell}$ [GeV]");

    let fig = fig.ratio(main, ratio);
    let dir = std::env::temp_dir().join("oxiplot-examples");
    std::fs::create_dir_all(&dir)?;
    for ext in ["png", "pdf"] {
        let path = dir.join(format!("data_mc_ratio.{ext}"));
        fig.save(&path)?;
        println!("wrote {}", path.display());
    }
    Ok(())
}
