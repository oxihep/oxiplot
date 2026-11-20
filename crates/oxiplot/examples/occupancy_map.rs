//! A detector occupancy map: a fine 2-D histogram on a log colour scale.
//!
//! 120 × 128 bins in (η, φ) — 15 360 cells — so the heatmap is drawn as one
//! image rather than a rectangle per cell, which keeps the SVG and PDF
//! small. Hits fall off with |η|, a module is dead (white: empty bins show the
//! background) and one channel is noisy. Run with:
//!
//! ```sh
//! cargo run -p oxiplot --example occupancy_map
//! ```

use oxiplot::{Axes, Binned2D, Colormap, Hist2dOpts};

fn main() -> oxiplot::Result<()> {
    let pi = std::f64::consts::PI;
    let (nx, ny) = (120, 128);
    let x_edges: Vec<f64> = (0..=nx)
        .map(|i| -3.0 + 6.0 * f64::from(i) / f64::from(nx))
        .collect();
    let y_edges: Vec<f64> = (0..=ny)
        .map(|i| -pi + 2.0 * pi * f64::from(i) / f64::from(ny))
        .collect();
    let grid: Vec<Vec<f64>> = (0..nx)
        .map(|ix| {
            let eta = -3.0 + 6.0 * (f64::from(ix) + 0.5) / f64::from(nx);
            (0..ny)
                .map(|iy| {
                    let phi = -pi + 2.0 * pi * (f64::from(iy) + 0.5) / f64::from(ny);
                    if (0.5..1.0).contains(&eta) && (1.0..1.6).contains(&phi) {
                        return 0.0; // a dead module
                    }
                    let noisy = if ix == 30 && iy == 20 { 50.0 } else { 1.0 };
                    // Occupancy falls towards the forward region, with a
                    // little φ modulation from the module overlaps.
                    noisy * 1000.0 * (-0.6 * eta.abs()).exp() * (1.0 + 0.15 * (16.0 * phi).cos())
                })
                .collect()
        })
        .collect();
    let map = Binned2D::new(x_edges, y_edges, grid)?;

    let mut ax = Axes::new();
    ax.hist2d_with(
        &map,
        Hist2dOpts::new()
            .cmap(Colormap::Viridis)
            .log(true)
            .label("hits"),
    );
    ax.xlabel("$\\eta$")
        .ylabel("$\\phi$")
        .title("Pixel layer 1 occupancy");

    let dir = std::env::temp_dir().join("oxiplot-examples");
    std::fs::create_dir_all(&dir)?;
    for ext in ["png", "svg", "pdf"] {
        let path = dir.join(format!("occupancy_map.{ext}"));
        ax.save(&path)?;
        println!(
            "wrote {} ({} kB)",
            path.display(),
            std::fs::metadata(&path)?.len() / 1024
        );
    }
    Ok(())
}
