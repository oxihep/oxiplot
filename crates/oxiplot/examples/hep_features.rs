//! The pieces a HEP plot is made of, one figure each: Poisson error bars and
//! a hatched uncertainty band with a ratio panel, bin-width normalisation,
//! underflow and overflow, markers, shaded spans, a second y axis, and the
//! experiment styles. Run with:
//!
//! ```sh
//! cargo run -p oxiplot --example hep_features
//! ```
//!
//! The figures go to the system temporary directory, in `oxiplot-hep-features`.

use oxiplot::{
    Axes, Binned1D, Color, CurveOpts, ErrorbarOpts, FillOpts, Flow, GridSpec, Hatch, Hist1dData,
    HistOpts, HistType, Loc, Marker, Scale, Style, XyPoints, ratio_subplots_with,
    subplots_grid_with,
};

/// A deterministic generator, so the figures do not change between runs.
struct Lcg(u64);

impl Lcg {
    fn uniform(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 11) as f64 + 0.5) / (1_u64 << 53) as f64
    }

    fn gauss(&mut self) -> f64 {
        let (u, v) = (self.uniform(), self.uniform());
        (-2.0 * u.ln()).sqrt() * (2.0 * std::f64::consts::PI * v).cos()
    }

    /// A Poisson count of mean `mean` (Knuth's method; the means here are small).
    fn poisson(&mut self, mean: f64) -> f64 {
        let limit = (-mean).exp();
        let (mut k, mut p) = (0.0, self.uniform());
        while p > limit {
            k += 1.0;
            p *= self.uniform();
        }
        k
    }
}

/// A histogram that keeps what falls outside its range.
struct WithFlow {
    inner: Binned1D,
    under: f64,
    over: f64,
}

impl Hist1dData for WithFlow {
    fn edges(&self) -> Vec<f64> {
        self.inner.edges()
    }
    fn values(&self) -> Vec<f64> {
        self.inner.values()
    }
    fn error(&self, i: usize) -> f64 {
        self.inner.error(i)
    }
    fn underflow(&self) -> Option<(f64, f64)> {
        Some((self.under, self.under.sqrt()))
    }
    fn overflow(&self) -> Option<(f64, f64)> {
        Some((self.over, self.over.sqrt()))
    }
}

fn main() -> oxiplot::Result<()> {
    let out = std::env::temp_dir().join("oxiplot-hep-features");
    std::fs::create_dir_all(&out).expect("a writable temporary directory");
    let mut rng = Lcg(0x5eed_1234_abcd_ef01);

    // ---- 1. Data with Poisson errors over a prediction with its band ------
    // A prediction of a few events per bin in the tail, with a 10 %
    // uncertainty, and data drawn from it.
    let edges: Vec<f64> = (0..=20).map(|i| 100.0 + 10.0 * f64::from(i)).collect();
    let expected: Vec<f64> = edges
        .windows(2)
        .map(|w| 90.0 * (-(w[0] - 100.0) / 38.0).exp())
        .collect();
    let model = Binned1D::new(edges.clone(), expected.clone())?
        .with_errors(expected.iter().map(|v| 0.1 * v).collect())?;
    let counts: Vec<f64> = expected.iter().map(|&m| rng.poisson(m)).collect();
    let data = Binned1D::new(edges.clone(), counts.clone())?
        .with_errors(counts.iter().map(|c| c.sqrt()).collect())?;

    let (fig, mut main, mut lower) = ratio_subplots_with(Style::cms());
    main.hist_with(
        &model,
        HistOpts::new()
            .histtype(HistType::Fill)
            .color(Color::hex("#3f90da"))
            .fill_color(Color::hex("#3f90da").with_alpha(0.35))
            .label("Prediction"),
    );
    main.hist_with(
        &model,
        HistOpts::new()
            .histtype(HistType::Band)
            .hatch(Hatch::Diagonal)
            .color(Color::hex("#555555"))
            .label("Stat. unc."),
    );
    main.hist_with(
        &data,
        HistOpts::new()
            .histtype(HistType::Errorbar)
            .poisson_errors(true)
            .empty_bins(true)
            .color(Color::BLACK)
            .label("Data"),
    );
    main.yscale(Scale::Log)
        .ylim(0.2..400.0)
        .ylabel("Events / 10 GeV")
        .hep_label("Demo", "Simulation")
        .hep_rhs("$\\sqrt{s} = 13$ TeV")
        .legend_title("Signal region")
        .legend();
    lower.hist_with(
        &Binned1D::relative_uncertainty(&model),
        HistOpts::new()
            .histtype(HistType::Band)
            .hatch(Hatch::Diagonal)
            .color(Color::hex("#555555")),
    );
    lower.errorbar_with(
        &XyPoints::ratio(&data, &model)?,
        ErrorbarOpts::new().color(Color::BLACK),
    );
    lower.axhline_with(1.0, CurveOpts::new().dashed(vec![4.0, 3.0]));
    lower
        .ylim(0.0..2.5)
        .yticks(&[0.5, 1.0, 1.5, 2.0])
        .ylabel("Data / pred.")
        .xlabel("$m$ [GeV]");
    fig.ratio(main, lower)
        .save(out.join("1_poisson_band_ratio.png"))?;

    // ---- 2. Bins of different widths: per bin, per GeV, unit area ---------
    let wide: Vec<f64> = vec![0.0, 10.0, 20.0, 30.0, 50.0, 80.0, 130.0, 200.0];
    let samples: Vec<f64> = (0..20_000).map(|_| -45.0 * rng.uniform().ln()).collect();
    let spectrum = Binned1D::from_samples(wide, &samples);
    let (fig, mut axs) = subplots_grid_with(Style::cms(), GridSpec::new(1, 3));
    let titles = ["per bin", "per GeV", "unit area"];
    for (i, ax) in axs.iter_mut().enumerate() {
        let opts = HistOpts::new().color(Color::hex("#bd1f01"));
        ax.hist_with(
            &spectrum,
            match i {
                0 => opts,
                1 => opts.per_width(true),
                _ => opts.density(true),
            },
        );
        ax.title(titles[i]).xlabel("$p_T$ [GeV]");
    }
    axs[0].ylabel("Events");
    fig.with_axes(axs).save(out.join("2_normalisation.png"))?;

    // ---- 3. Underflow and overflow ----------------------------------------
    let values: Vec<f64> = (0..3_000).map(|_| 50.0 + 22.0 * rng.gauss()).collect();
    let range: Vec<f64> = (0..=10).map(|i| 20.0 + 6.0 * f64::from(i)).collect();
    let flows = WithFlow {
        inner: Binned1D::from_samples(range, &values),
        under: values.iter().filter(|&&v| v < 20.0).count() as f64,
        over: values.iter().filter(|&&v| v >= 80.0).count() as f64,
    };
    let (fig, mut axs) = subplots_grid_with(Style::cms(), GridSpec::new(1, 3));
    for (ax, (flow, title)) in axs.iter_mut().zip([
        (Flow::Hide, "hidden"),
        (Flow::Show, "shown"),
        (Flow::Sum, "summed"),
    ]) {
        ax.hist_with(
            &flows,
            HistOpts::new()
                .histtype(HistType::Fill)
                .color(Color::hex("#3f90da"))
                .fill_color(Color::hex("#3f90da").with_alpha(0.4))
                .flow(flow),
        );
        ax.title(title).xlabel("$x$");
    }
    axs[0].ylabel("Events");
    fig.sharey(true)
        .with_axes(axs)
        .save(out.join("3_flow.png"))?;

    // ---- 4. Markers, and a legend in two columns --------------------------
    let markers = [
        Marker::Circle,
        Marker::Square,
        Marker::TriangleUp,
        Marker::TriangleDown,
        Marker::Diamond,
        Marker::Star,
        Marker::Cross,
        Marker::Plus,
        Marker::OpenCircle,
        Marker::OpenSquare,
        Marker::OpenTriangleUp,
        Marker::OpenTriangleDown,
        Marker::OpenDiamond,
    ];
    let mut ax = Axes::with_style(Style::cms());
    for (i, &marker) in markers.iter().enumerate() {
        let xs: Vec<f64> = (0..6).map(f64::from).collect();
        let ys: Vec<f64> = xs.iter().map(|x| i as f64 + 0.25 * (x - 2.5)).collect();
        let points = XyPoints::new(xs, ys)?.with_y_errors(vec![0.2; 6])?;
        ax.errorbar_with(
            &points,
            ErrorbarOpts::new()
                .marker(marker)
                .marker_size(7.0)
                .label(format!("{marker:?}")),
        );
    }
    ax.xlim(-0.5..17.0)
        .xlabel("$x$")
        .ylabel("$y$")
        .legend_ncol(2)
        .legend_loc(Loc::CenterRight);
    ax.save(out.join("4_markers.png"))?;

    // ---- 5. Shaded spans: a signal region and a blinded window ------------
    let mass: Vec<f64> = (0..=60).map(|i| 100.0 + f64::from(i)).collect();
    let bkg: Vec<f64> = mass
        .windows(2)
        .map(|w| 400.0 * (-(w[0] - 100.0) / 30.0).exp())
        .collect();
    let sidebands: Vec<f64> = mass
        .windows(2)
        .zip(&bkg)
        .map(|(w, &b)| {
            if (120.0..130.0).contains(&w[0]) {
                0.0
            } else {
                rng.poisson(b.min(60.0)) * (b / b.min(60.0))
            }
        })
        .collect();
    let observed = Binned1D::new(mass, sidebands.clone())?
        .with_errors(sidebands.iter().map(|v| v.sqrt()).collect())?;
    let mut ax = Axes::with_style(Style::atlas());
    ax.hist_with(
        &observed,
        HistOpts::new()
            .histtype(HistType::Errorbar)
            .color(Color::BLACK)
            .label("Data, sidebands"),
    );
    ax.axvspan_with(
        120.0..130.0,
        FillOpts::new()
            .hollow(true)
            .color(Color::hex("#555555"))
            .hatch(Hatch::Diagonal)
            .label("Blinded"),
    );
    ax.axvspan_with(
        105.0..115.0,
        FillOpts::new()
            .color(Color::hex("#f89c20"))
            .alpha(0.25)
            .label("Control region"),
    );
    ax.xlabel("$m_{\\gamma\\gamma}$ [GeV]")
        .ylabel("Events / GeV")
        .hep_label("Demo", "Internal")
        .legend();
    ax.save(out.join("5_spans_atlas.png"))?;

    // ---- 6. A second y axis: a spectrum and its cumulative fraction -------
    let edges: Vec<f64> = (0..=30).map(|i| 4.0 * f64::from(i)).collect();
    let spectrum = Binned1D::from_samples(edges.clone(), &samples);
    let total: f64 = spectrum.values().iter().sum();
    let cumulative: Vec<f64> = spectrum
        .values()
        .iter()
        .scan(0.0, |acc, v| {
            *acc += v / total;
            Some(*acc)
        })
        .collect();
    let mut ax = Axes::with_style(Style::lhcb());
    ax.hist_with(
        &spectrum,
        HistOpts::new()
            .color(Color::hex("#3f90da"))
            .label("Candidates"),
    );
    ax.xlabel("$p_T$ [GeV]").ylabel("Candidates / 4 GeV");
    let right = ax.twinx();
    right.plot_with(
        &edges[1..],
        &cumulative,
        CurveOpts::new()
            .color(Color::hex("#bd1f01"))
            .label("Cumulative fraction"),
    )?;
    // Room above the curve for the label inside the frame.
    right.ylim(0.0..1.3).ylabel("Cumulative fraction");
    ax.hep_label("Demo", "simulation")
        .legend_loc(Loc::CenterRight);
    ax.save(out.join("6_twin_axis_lhcb.png"))?;

    // ---- 7. The four experiment styles -------------------------------------
    for (name, style) in [
        ("cms", Style::cms()),
        ("atlas", Style::atlas()),
        ("lhcb", Style::lhcb()),
        ("alice", Style::alice()),
    ] {
        let mut ax = Axes::with_style(style);
        ax.hist_with(
            &data,
            HistOpts::new()
                .histtype(HistType::Errorbar)
                .poisson_errors(true)
                .color(Color::BLACK)
                .label("Data"),
        );
        ax.hist_with(
            &model,
            HistOpts::new()
                .color(Color::hex("#3f90da"))
                .label("Prediction"),
        );
        ax.xlabel("$m$ [GeV]")
            .ylabel("Events / 10 GeV")
            .hep_label("Demo", "Preliminary")
            .hep_rhs("$\\sqrt{s} = 13$ TeV")
            .legend();
        ax.save(out.join(format!("7_style_{name}.png")))?;
    }

    println!("wrote 10 figures to {}", out.display());
    Ok(())
}
