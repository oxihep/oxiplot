//! Calorimeter energy resolution: σ/E versus E on a log axis, with its
//! stochastic, noise and constant terms.
//!
//! A calorimeter's relative resolution is
//!
//! ```text
//! σ/E = a/√E ⊕ b/E ⊕ c
//! ```
//!
//! (added in quadrature): sampling fluctuations (`a`), electronic noise
//! (`b`) and calibration non-uniformity (`c`). Each term dominates in its own
//! energy range, which a log x axis shows at a glance. Run with:
//!
//! ```sh
//! cargo run -p oxiplot --example energy_resolution
//! ```

use oxiplot::{Axes, Color, CurveOpts, ErrorbarOpts, Points, Scale, Style};

const A: f64 = 0.10; // 10 %/√E
const B: f64 = 0.25; // 250 MeV noise
const C: f64 = 0.007; // 0.7 % constant term

fn total(e: f64) -> f64 {
    ((A * A) / e + (B / e).powi(2) + C * C).sqrt()
}

fn main() -> oxiplot::Result<()> {
    // Test-beam points: the model with a small deterministic scatter.
    let energies = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0];
    let res: Vec<f64> = energies
        .iter()
        .enumerate()
        .map(|(i, &e)| total(e) * (1.0 + 0.04 * (i as f64 * 2.3).sin()))
        .collect();
    let errs: Vec<f64> = res.iter().map(|r| 0.04 * r).collect();
    let points = Points::new(energies.to_vec(), res)?.with_y_errors(errs)?;

    let mut ax = Axes::with_style(Style::mplhep());
    ax.errorbar_with(
        &points,
        ErrorbarOpts::new().color(Color::BLACK).label("Test beam"),
    );
    ax.function_with(
        total,
        0.8..800.0,
        CurveOpts::new()
            .color(Color::hex("#bd1f01"))
            .samples(400)
            .label("Total"),
    );
    let dashed = |c: &str, label: &str| {
        CurveOpts::new()
            .color(Color::hex(c))
            .dashed(vec![5.0, 3.0])
            .samples(400)
            .label(label)
    };
    ax.function_with(
        |e| A / e.sqrt(),
        0.8..800.0,
        dashed("#3f90da", "Stochastic $a/\\sqrt{E}$"),
    );
    ax.function_with(|e| B / e, 0.8..800.0, dashed("#e76300", "Noise $b/E$"));
    ax.function_with(|_| C, 0.8..800.0, dashed("#717581", "Constant $c$"));
    ax.xscale(Scale::Log)
        .yscale(Scale::Log)
        .ylim(0.003..0.5)
        .xlabel("Beam energy $E$ [GeV]")
        .ylabel("$\\sigma_E / E$")
        .hep_label("Demo", "Test beam")
        .grid()
        .legend();

    let dir = std::env::temp_dir().join("oxiplot-examples");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("energy_resolution.png");
    ax.save(&path)?;
    println!("wrote {}", path.display());
    Ok(())
}
