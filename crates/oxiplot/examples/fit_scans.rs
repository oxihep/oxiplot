//! Beyond the parabolic errors: 2-D confidence contours and a profile
//! likelihood, from an oxifit fit of a resonance.
//!
//! A toy Z → ee mass spectrum (a Gaussian peak on a falling background) is
//! fitted with a binned Poisson likelihood, as the sum `sig + bkg`. The left
//! panel shows the 68 % and 95 % joint regions of the peak's mean and width
//! (`Δ = 2.30` and `5.99`, the background refitted at every point); the right
//! panel the profile of the mean, whose crossings of the dashed `Δ = 1` and
//! `Δ = 4` lines are its 1σ and 2σ intervals. Needs the `fit` feature:
//!
//! ```sh
//! cargo run -p oxiplot --features fit --example fit_scans
//! ```

use oxifit::{FitExt, FitMethod, FitOptions, Model, Point, Points};
use oxiplot::{CurveOpts, GridSpec, Style, subplots_grid_with};

/// A small reproducible generator: uniform, Gaussian and exponential draws.
struct Toy(u64);

impl Toy {
    fn uniform(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }
    fn gauss(&mut self, mean: f64, sigma: f64) -> f64 {
        let (u1, u2) = (self.uniform(), self.uniform());
        mean + sigma * (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }
    fn falling(&mut self, scale: f64, lo: f64, hi: f64) -> f64 {
        let (a, b) = ((-lo / scale).exp(), (-hi / scale).exp());
        -scale * (a - self.uniform() * (a - b)).ln()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 3000 Z events (91.19 GeV, 2.5 GeV resolution) on 6000 background
    // events, binned in 60 bins over [70, 110) GeV.
    let (lo, hi, nbins) = (70.0, 110.0, 60);
    let width = (hi - lo) / nbins as f64;
    let mut counts = vec![0.0; nbins];
    let mut toy = Toy(11);
    let mut fill = |m: f64| {
        if (lo..hi).contains(&m) {
            counts[((m - lo) / width) as usize] += 1.0;
        }
    };
    for _ in 0..3000 {
        fill(toy.gauss(91.19, 2.5));
    }
    for _ in 0..6000 {
        fill(toy.falling(30.0, lo, hi));
    }
    let data = Points::from_points(
        counts
            .iter()
            .enumerate()
            .map(|(i, &n)| Point::new(lo + (i as f64 + 0.5) * width, n, n.sqrt()))
            .collect(),
    );

    let sig = Model::gaussian("sig")
        .with_params(vec![150.0, 90.0, 3.0])
        .lower_limit("sigma", 0.1);
    let bkg = Model::exponential("bkg").with_params(vec![6.0, -0.03]);
    let model = sig + bkg;
    let opts = FitOptions::new().method(FitMethod::Likelihood);
    let fit = data.fit_opts(&model, &opts);
    println!(
        "mean = {:.3} ± {:.3} GeV, width = {:.3} ± {:.3} GeV",
        fit.params[1], fit.errors[1], fit.params[2], fit.errors[2]
    );

    let (fig, mut axs) = subplots_grid_with(Style::mplhep(), GridSpec::new(1, 2));
    let (mut ylo, mut yhi) = (f64::INFINITY, f64::NEG_INFINITY);
    for (delta, label) in [(2.30, "68 % CL"), (5.99, "95 % CL")] {
        let c = data.contour(&model, &opts, "sig.mean", "sig.sigma", delta, 60)?;
        for &(_, y) in &c.points {
            (ylo, yhi) = (ylo.min(y), yhi.max(y));
        }
        axs[0].fit_contour_with(&c, CurveOpts::new().label(label));
    }
    // Headroom above the outer contour for the label and the legend.
    let pad = 0.1 * (yhi - ylo);
    axs[0]
        .ylim(ylo - pad..yhi + 5.0 * pad)
        .xlabel("Peak mean [GeV]")
        .ylabel("Peak width [GeV]")
        .hep_label("Demo", "Simulation")
        .legend();

    let (m, e) = (fit.params[1], fit.errors[1]);
    let profile = data.profile(&model, &opts, "sig.mean", (m - 3.0 * e, m + 3.0 * e), 41)?;
    let (a, b) = profile.interval(1.0).ok_or("scan range too narrow")?;
    println!("profile 1σ interval: [{a:.3}, {b:.3}] GeV");
    axs[1].fit_profile(&profile);
    axs[1]
        .xlabel("Peak mean [GeV]")
        .ylabel("$-2\\Delta\\ln L$")
        .hep_label("Demo", "Simulation");

    let dir = std::env::temp_dir().join("oxiplot-examples");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("fit_scans.png");
    fig.with_axes(axs).save(&path)?;
    println!("wrote {}", path.display());
    Ok(())
}
