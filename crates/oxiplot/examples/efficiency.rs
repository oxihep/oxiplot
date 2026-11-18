//! A trigger efficiency with asymmetric (Wilson) binomial errors.
//!
//! Efficiencies near 0 or 1 have lopsided uncertainties: a bin where all 40
//! probes passed is not "100 % ± 0". The Wilson score interval gives the
//! right asymmetric errors; the points carry them as low/high y errors, and
//! the generating turn-on curve is overlaid. Run with:
//!
//! ```sh
//! cargo run -p oxiplot --example efficiency
//! ```

use oxiplot::{Axes, Color, CurveOpts, ErrorbarOpts, Points, Style};

/// The 68 % Wilson score interval for `k` passes out of `n`.
fn wilson(k: f64, n: f64) -> (f64, f64) {
    let z = 1.0;
    let p = k / n;
    let denom = 1.0 + z * z / n;
    let centre = (p + z * z / (2.0 * n)) / denom;
    let half = z * (p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt() / denom;
    (centre - half, centre + half)
}

/// The true turn-on: a logistic step at 25 GeV.
fn truth(pt: f64) -> f64 {
    0.97 / (1.0 + (-(pt - 25.0) / 1.8).exp())
}

fn main() -> oxiplot::Result<()> {
    let mut x = Vec::new();
    let (mut eff, mut lo, mut hi) = (Vec::new(), Vec::new(), Vec::new());
    let mut xerr = Vec::new();
    for i in 0..20 {
        let pt = 11.0 + 2.5 * f64::from(i);
        // Few probes per bin, as at low pT in a real tag-and-probe sample:
        // the expected number passing, rounded.
        let n = 40.0;
        let k = (n * truth(pt)).round();
        let (l, h) = wilson(k, n);
        x.push(pt);
        eff.push(k / n);
        lo.push(k / n - l);
        hi.push(h - k / n);
        xerr.push(1.25);
    }
    let points = Points::new(x, eff)?
        .with_y_errors_asym(lo, hi)?
        .with_x_errors(xerr)?;

    let mut ax = Axes::with_style(Style::mplhep());
    ax.errorbar_with(
        &points,
        ErrorbarOpts::new().color(Color::BLACK).label("Tag & probe"),
    );
    ax.function_with(
        truth,
        10.0..60.0,
        CurveOpts::new()
            .color(Color::hex("#bd1f01"))
            .label("True turn-on"),
    );
    ax.function_with(
        |_| 0.97,
        10.0..60.0,
        CurveOpts::new()
            .color(Color::hex("#888888"))
            .dashed(vec![3.0, 3.0])
            .label("Plateau"),
    );
    ax.ylim(0.0..1.15)
        .xlabel("Offline muon $p_T$ [GeV]")
        .ylabel("Trigger efficiency")
        .hep_label("Demo", "Simulation")
        .legend();

    let dir = std::env::temp_dir().join("oxiplot-examples");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("efficiency.png");
    ax.save(&path)?;
    println!("wrote {}", path.display());
    Ok(())
}
