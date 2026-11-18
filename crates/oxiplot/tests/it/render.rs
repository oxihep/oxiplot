//! High-level rendering tests that drive the public `oxiplot` API end to end
//! and assert real properties of the output: the format chosen by extension, the
//! in-memory render bytes, DPI scaling, determinism, SVG structure, and that every
//! artist / layout actually renders. No pixel comparison — only structural and
//! invariant checks that won't flake.

#[cfg(feature = "png")]
use oxiplot::Norm;
#[cfg(feature = "png")]
use oxiplot::SaveOpts;
use oxiplot::{
    Axes, Binned1D, Binned2D, Color, CurveOpts, Error, ErrorbarOpts, FontSet, Hist2dOpts, HistOpts,
    HistType, PdfPages, Points, Style, ratio_subplots, subplots, subplots_grid,
};

// --- deterministic fixtures (a tiny LCG → reproducible bytes, no rng dep) ---

/// `n` uniform bin edges over `[lo, hi]`.
fn edges(n: usize, lo: f64, hi: f64) -> Vec<f64> {
    (0..=n)
        .map(|i| lo + (hi - lo) * i as f64 / n as f64)
        .collect()
}

/// A 2-D histogram with uniform bins, filled by `(ix, iy) -> weight`.
fn binned2d(
    nx: usize,
    (xlo, xhi): (f64, f64),
    ny: usize,
    (ylo, yhi): (f64, f64),
    w: impl Fn(usize, usize) -> f64,
) -> Binned2D {
    let grid = (0..nx)
        .map(|ix| (0..ny).map(|iy| w(ix, iy)).collect())
        .collect();
    Binned2D::new(edges(nx, xlo, xhi), edges(ny, ylo, yhi), grid).unwrap()
}

fn gauss_hist() -> Binned1D {
    let mut s = 0x1234_5678_9abc_def0u64;
    let mut next = move || {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (s >> 33) as f64 / (1u64 << 31) as f64
    };
    let mut gauss = move || (0..12).map(|_| next()).sum::<f64>() - 6.0;
    let samples: Vec<f64> = (0..5000).map(|_| 90.0 + 8.0 * gauss()).collect();
    Binned1D::from_samples(edges(40, 50.0, 130.0), &samples)
}

fn graph() -> Points {
    let x: Vec<f64> = (0..8).map(|i| 55.0 + 10.0 * i as f64).collect();
    let y: Vec<f64> = x
        .iter()
        .map(|v| 1000.0 * (-0.5 * ((v - 90.0) / 9.0).powi(2)).exp())
        .collect();
    let e: Vec<f64> = y.iter().map(|v| v.sqrt().max(5.0)).collect();
    Points::new(x.clone(), y)
        .unwrap()
        .with_x_errors(vec![5.0; x.len()])
        .unwrap()
        .with_y_errors(e)
        .unwrap()
}

fn th2() -> Binned2D {
    binned2d(20, (-3.0, 3.0), 20, (-3.0, 3.0), |ix, iy| {
        let x = -3.0 + (ix as f64 + 0.5) * 0.3;
        let y = -3.0 + (iy as f64 + 0.5) * 0.3;
        (-(x * x + y * y) / 2.0).exp() * 100.0
    })
}

/// Per-bin means of `y = 1.5 x + {0, 1, 2}` and their standard errors, as a
/// profile holds them.
#[cfg(feature = "png")]
fn profile() -> Binned1D {
    let means: Vec<f64> = (0..10).map(|i| 1.5 * (i as f64 + 0.5) + 1.0).collect();
    Binned1D::new(edges(10, 0.0, 10.0), means)
        .unwrap()
        .with_errors(vec![(2.0f64 / 3.0).sqrt() / 40f64.sqrt(); 10])
        .unwrap()
}

/// Decode just the width/height from a PNG's IHDR (big-endian at bytes 16..24).
#[cfg(feature = "png")]
fn png_dims(b: &[u8]) -> (u32, u32) {
    assert!(b.starts_with(b"\x89PNG\r\n\x1a\n"), "not a PNG");
    let w = u32::from_be_bytes([b[16], b[17], b[18], b[19]]);
    let h = u32::from_be_bytes([b[20], b[21], b[22], b[23]]);
    (w, h)
}

fn occurrences(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

// --- reading geometry back out of an SVG ---------------------------------------

/// The value of attribute `name` in an SVG element's text.
fn attr(el: &str, name: &str) -> Option<String> {
    let key = format!(" {name}=\"");
    let start = el.find(&key)? + key.len();
    Some(el[start..].split('"').next()?.to_string())
}

fn num_attr(el: &str, name: &str) -> f32 {
    attr(el, name)
        .and_then(|v| v.parse().ok())
        .unwrap_or(f32::NAN)
}

/// Every `<tag …/>` element in `svg`, as its text.
fn elements<'a>(svg: &'a str, tag: &str) -> Vec<&'a str> {
    let open = format!("<{tag} ");
    svg.match_indices(&open)
        .map(|(i, _)| {
            let end = svg[i..].find("/>").map_or(svg.len(), |e| i + e);
            &svg[i..end]
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Frame {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl Frame {
    fn right(&self) -> f32 {
        self.x + self.w
    }
    fn bottom(&self) -> f32 {
        self.y + self.h
    }
    fn contains(&self, (x, y): (f32, f32)) -> bool {
        x >= self.x - 0.5
            && x <= self.right() + 0.5
            && y >= self.y - 0.5
            && y <= self.bottom() + 0.5
    }
}

/// The panels' frames: the clip rectangles, once per panel (a panel clips
/// its grid and its data to the same frame).
fn frames(svg: &str) -> Vec<Frame> {
    let mut out: Vec<Frame> = Vec::new();
    for clip in svg.split("<clipPath ").skip(1) {
        let rect = elements(clip, "rect")[0];
        let f = Frame {
            x: num_attr(rect, "x"),
            y: num_attr(rect, "y"),
            w: num_attr(rect, "width"),
            h: num_attr(rect, "height"),
        };
        if out.last() != Some(&f) {
            out.push(f);
        }
    }
    out
}

/// The vertices of every `<tag points="…">` element (polyline, polygon).
fn point_lists(svg: &str, tag: &str) -> Vec<Vec<(f32, f32)>> {
    elements(svg, tag)
        .iter()
        .map(|el| {
            attr(el, "points")
                .unwrap_or_default()
                .split_whitespace()
                .map(|p| {
                    let (x, y) = p.split_once(',').unwrap();
                    (x.parse().unwrap(), y.parse().unwrap())
                })
                .collect()
        })
        .collect()
}

/// The centres of every `<circle>` (markers).
fn circles(svg: &str) -> Vec<(f32, f32)> {
    elements(svg, "circle")
        .iter()
        .map(|c| (num_attr(c, "cx"), num_attr(c, "cy")))
        .collect()
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.6
}

/// A unique scratch dir for a test that writes files.
fn scratch(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("oxiplot_test_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// --- output format selection ---------------------------------------------------

#[test]
#[cfg(feature = "png")] // renders PNG too
fn save_picks_format_from_extension() {
    let dir = scratch("formats");
    let mut ax = Axes::new();
    ax.hist(&gauss_hist());
    ax.xlabel("$m$ [GeV]");

    ax.save(dir.join("p.png")).unwrap();
    ax.save(dir.join("p.svg")).unwrap();
    ax.save(dir.join("p.pdf")).unwrap();

    let png = std::fs::read(dir.join("p.png")).unwrap();
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"), "PNG magic");

    let svg = std::fs::read_to_string(dir.join("p.svg")).unwrap();
    assert!(svg.starts_with("<svg") && svg.trim_end().ends_with("</svg>"));

    let pdf = std::fs::read(dir.join("p.pdf")).unwrap();
    assert!(pdf.starts_with(b"%PDF-1.4"), "PDF header");
    assert!(pdf.ends_with(b"%%EOF\n"), "PDF trailer");
    assert!(pdf.windows(4).any(|w| w == b"xref"), "PDF xref table");
}

#[test]
fn unknown_extension_is_an_error_that_mentions_the_formats() {
    let dir = scratch("badext");
    let mut ax = Axes::new();
    ax.plot(&[0.0, 1.0], &[0.0, 1.0]).unwrap();
    let err = ax.save(dir.join("p.jpg")).unwrap_err();
    assert!(matches!(err, Error::UnknownFormat(_)));
    let msg = err.to_string();
    assert!(msg.contains("jpg") && msg.contains("pdf"), "got: {msg}");
}

// --- in-memory rendering -------------------------------------------------------

#[test]
#[cfg(feature = "png")] // renders PNG too
fn in_memory_render_produces_each_format() {
    let mut ax = Axes::new();
    ax.hist(&gauss_hist());

    let png = ax.to_png_bytes(SaveOpts::new()).unwrap();
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));

    let svg = ax.to_svg_string();
    assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));

    let pdf = ax.to_pdf_bytes();
    assert!(pdf.starts_with(b"%PDF-1.4") && pdf.ends_with(b"%%EOF\n"));
}

#[test]
#[cfg(feature = "png")] // renders PNG too
fn file_and_in_memory_bytes_agree() {
    let dir = scratch("agree");
    let mut ax = Axes::new();
    ax.hist(&gauss_hist());
    ax.xlabel("$m$");

    ax.save(dir.join("a.png")).unwrap();
    ax.save(dir.join("a.svg")).unwrap();
    assert_eq!(
        std::fs::read(dir.join("a.png")).unwrap(),
        ax.to_png_bytes(SaveOpts::new()).unwrap(),
        "save(.png) must equal to_png_bytes"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("a.svg")).unwrap(),
        ax.to_svg_string(),
        "save(.svg) must equal to_svg_string"
    );
}

#[test]
fn hep_label_adds_glyphs_above_the_frame() {
    use oxiplot::Style;
    // The bold experiment label + italic status + right-hand lumi/energy each add
    // glyph paths to the SVG, so a labelled axes renders more `<path` than a bare
    // one with the same data.
    let mut bare = Axes::with_style(Style::mplhep());
    bare.hist(&gauss_hist());
    let n_bare = occurrences(&bare.to_svg_string(), "<path");

    let mut labelled = Axes::with_style(Style::mplhep());
    labelled.hist(&gauss_hist());
    labelled
        .hep_label("CMS", "Preliminary")
        .hep_rhs("138 fb$^{-1}$ (13 TeV)");
    let n_labelled = occurrences(&labelled.to_svg_string(), "<path");

    assert!(
        n_labelled > n_bare + 15,
        "expected the experiment label to add many glyph paths: {n_bare} -> {n_labelled}"
    );
}

#[test]
#[cfg(feature = "png")] // renders PNG too
fn rendering_is_deterministic() {
    // Two independently built identical plots must produce identical bytes.
    let render = || {
        let mut ax = Axes::new();
        ax.hist_with(
            &gauss_hist(),
            HistOpts::new().histtype(HistType::Fill).label("MC"),
        );
        ax.errorbar_with(
            &graph(),
            ErrorbarOpts::new().color(Color::BLACK).label("data"),
        );
        ax.xlabel("$m_{\\mu\\mu}$ [GeV]");
        ax.legend();
        ax
    };
    let a = render();
    let b = render();
    assert_eq!(a.to_svg_string(), b.to_svg_string());
    assert_eq!(
        a.to_png_bytes(SaveOpts::new()).unwrap(),
        b.to_png_bytes(SaveOpts::new()).unwrap()
    );
    assert_eq!(a.to_pdf_bytes(), b.to_pdf_bytes());
}

// --- save options --------------------------------------------------------------

#[test]
#[cfg(feature = "png")] // renders PNG too
fn dpi_scales_the_png_raster() {
    let mut ax = Axes::new();
    ax.hist(&gauss_hist());
    let base = png_dims(&ax.to_png_bytes(SaveOpts::new()).unwrap());
    let hi = png_dims(&ax.to_png_bytes(SaveOpts::new().dpi(200.0)).unwrap());
    // The default style is 100 dpi, so 200 dpi doubles both dimensions exactly.
    assert_eq!(hi.0, base.0 * 2, "width doubles at 2x dpi");
    assert_eq!(hi.1, base.1 * 2, "height doubles at 2x dpi");
    // Vector formats are resolution-independent: the SVG viewBox is unchanged.
    assert!(
        ax.to_svg_string()
            .contains(&format!("viewBox=\"0 0 {} {}\"", base.0, base.1))
    );
}

#[test]
#[cfg(feature = "png")] // renders PNG too
fn transparency_changes_the_raster() {
    let mut ax = Axes::new();
    ax.hist(&gauss_hist());
    let opaque = ax.to_png_bytes(SaveOpts::new()).unwrap();
    let transparent = ax.to_png_bytes(SaveOpts::new().transparent()).unwrap();
    assert_ne!(
        opaque, transparent,
        "the transparent flag must affect the output"
    );
    // Same geometry, different pixels.
    assert_eq!(png_dims(&opaque), png_dims(&transparent));
}

// --- SVG structure & content ---------------------------------------------------

#[test]
fn svg_is_well_formed_and_sized() {
    let mut ax = Axes::new();
    ax.hist(&gauss_hist());
    let svg = ax.to_svg_string();
    assert!(svg.starts_with("<svg"));
    assert!(svg.ends_with("</svg>"));
    // 6.4 × 4.8 in, stated in points; the drawing is in 640 × 480 pixels.
    assert!(
        svg.contains("width=\"460.8pt\" height=\"345.6pt\""),
        "default figure size"
    );
    assert!(svg.contains("viewBox=\"0 0 640 480\""));
    // There is always an opaque background rect and a frame.
    assert!(svg.contains("<rect"));
}

#[test]
fn math_and_text_labels_emit_glyph_paths() {
    let mut plain = Axes::new();
    plain.hist(&gauss_hist());
    let n_plain = occurrences(&plain.to_svg_string(), "<path");

    let mut labelled = Axes::new();
    labelled.hist(&gauss_hist());
    labelled.xlabel("$\\chi^2 / \\mathrm{ndf}$");
    labelled.ylabel("Events");
    labelled.title("$Z \\to \\mu\\mu$");
    let svg = labelled.to_svg_string();
    let n_labelled = occurrences(&svg, "<path");

    // Tick numerals already produce glyph paths; the labels add many more.
    assert!(n_plain > 5, "tick labels are glyph paths, got {n_plain}");
    assert!(
        n_labelled > n_plain + 10,
        "axis labels + math should add glyphs ({n_plain} -> {n_labelled})"
    );
}

#[test]
fn legend_adds_a_frame_and_label_glyphs() {
    let h = gauss_hist();
    let mut without = Axes::new();
    without.hist_with(&h, HistOpts::new().label("MC"));
    let n_without = occurrences(&without.to_svg_string(), "<path");

    let mut with = Axes::new();
    with.hist_with(&h, HistOpts::new().label("MC"));
    with.legend();
    let svg = with.to_svg_string();

    assert!(
        occurrences(&svg, "<path") > n_without,
        "legend label adds glyphs"
    );
    // The default style draws a translucent white legend frame.
    assert!(svg.contains("fill-opacity=\"0.8\""), "legend frame box");
}

// --- every artist type renders -------------------------------------------------

#[test]
fn all_histtypes_render() {
    let h = gauss_hist(); // 40 bins over [50, 130)
    let n = 40;
    let svg_of = |t: HistType| {
        let mut ax = Axes::new();
        ax.hist_with(&h, HistOpts::new().histtype(t).yerr());
        ax.to_svg_string()
    };

    // A step outline: 2n + 2 vertices, from the frame's bottom-left corner
    // (the edges are sticky, the bars stand on 0) to its bottom-right.
    let svg = svg_of(HistType::Step);
    let f = frames(&svg)[0];
    let outline = point_lists(&svg, "polyline")
        .into_iter()
        .find(|p| p.len() == 2 * n + 2)
        .expect("a step outline");
    assert!(close(outline[0].0, f.x) && close(outline[0].1, f.bottom()));
    let last = outline[outline.len() - 1];
    assert!(close(last.0, f.right()) && close(last.1, f.bottom()));
    assert!(
        outline.iter().all(|&p| f.contains(p)),
        "the outline leaves the frame"
    );

    // A fill: the same outline, closed as a polygon.
    let svg = svg_of(HistType::Fill);
    assert!(
        point_lists(&svg, "polygon")
            .iter()
            .any(|p| p.len() == 2 * n + 2)
    );

    // Errorbar: one marker per bin, at the bin centres, inside the frame.
    let svg = svg_of(HistType::Errorbar);
    let f = frames(&svg)[0];
    let markers = circles(&svg);
    assert_eq!(markers.len(), n);
    assert!(markers.iter().all(|&c| f.contains(c)));
    let dx = markers[1].0 - markers[0].0;
    assert!(
        close(markers[0].0 - f.x, dx / 2.0),
        "the first marker is off its bin centre"
    );

    // Band: upper and lower staircases (2n vertices each), translucent.
    let svg = svg_of(HistType::Band);
    let band = elements(&svg, "polygon")
        .into_iter()
        .find(|el| attr(el, "fill-opacity").is_some_and(|o| o.starts_with("0.3")))
        .expect("a translucent band");
    assert_eq!(
        attr(band, "points").unwrap().split_whitespace().count(),
        4 * n
    );
}

#[test]
#[cfg(feature = "png")] // renders PNG too
fn graph_profile_plot_and_function_render() {
    // symmetric + asymmetric error graphs
    let sym = graph();
    let asym = Points::new(vec![1.0, 2.0, 3.0], vec![3.0, 4.0, 3.5])
        .unwrap()
        .with_x_errors_asym(vec![0.1, 0.1, 0.1], vec![0.2, 0.2, 0.2])
        .unwrap()
        .with_y_errors_asym(vec![0.3, 0.1, 0.2], vec![0.1, 0.3, 0.2])
        .unwrap();
    for (g, n) in [(&sym, 8), (&asym, 3)] {
        let mut ax = Axes::new();
        ax.errorbar(g);
        let svg = ax.to_svg_string();
        let f = frames(&svg)[0];
        let markers = circles(&svg);
        assert_eq!(markers.len(), n);
        assert!(markers.iter().all(|&c| f.contains(c)));
        // An x and a y error bar per point.
        let bars = elements(&svg, "line")
            .iter()
            .filter(|l| f.contains((num_attr(l, "x1"), num_attr(l, "y1"))))
            .filter(|l| {
                let (x1, x2) = (num_attr(l, "x1"), num_attr(l, "x2"));
                let (y1, y2) = (num_attr(l, "y1"), num_attr(l, "y2"));
                markers.iter().any(|&(cx, cy)| {
                    (close(x1, cx) && close(x2, cx)) || (close(y1, cy) && close(y2, cy))
                })
            })
            .count();
        assert!(bars >= 2 * n, "{bars} error bars for {n} points");
    }

    let mut ax = Axes::new();
    ax.profile(&profile());
    // One point per bin.
    assert_eq!(circles(&ax.to_svg_string()).len(), 10);
    assert!(
        ax.to_png_bytes(SaveOpts::new())
            .unwrap()
            .starts_with(b"\x89PNG")
    );

    let xs: Vec<f64> = (0..50).map(|i| i as f64 * 0.2).collect();
    let ys: Vec<f64> = xs.iter().map(|x| x.sin()).collect();
    let mut ax = Axes::new();
    ax.plot(&xs, &ys).unwrap();
    ax.function(|x| (x / 2.0).cos(), 0.0..10.0);
    let svg = ax.to_svg_string();
    assert!(
        occurrences(&svg, "<polyline") >= 2,
        "a line plot + a function curve"
    );
}

#[test]
fn function_with_options_styles_the_curve() {
    let mut ax = Axes::new();
    ax.function_with(
        |x| (x - 5.0_f64).powi(2),
        0.0..10.0,
        CurveOpts::new()
            .color(Color::hex("#d62728"))
            .linewidth(3.0)
            .dashed(vec![6.0, 3.0])
            .samples(64)
            .label("parabola"),
    );
    ax.legend();
    let svg = ax.to_svg_string();
    assert!(svg.contains("stroke=\"#d62728\""), "custom curve color");
    assert!(svg.contains("stroke-dasharray="), "dashed curve");
}

#[test]
fn hist2d_draws_a_mesh_and_colorbar() {
    let mut ax = Axes::new();
    ax.hist2d_with(&th2(), Hist2dOpts::new().label("entries"));
    let svg = ax.to_svg_string();
    let f = frames(&svg)[0];
    let rects: Vec<(f32, f32)> = elements(&svg, "rect")
        .iter()
        .filter(|r| attr(r, "fill").is_some_and(|c| c != "none"))
        .map(|r| (num_attr(r, "x"), num_attr(r, "width")))
        .collect();
    // Every one of the 20 × 20 cells is filled, inside the frame…
    let cells = rects
        .iter()
        .filter(|&&(x, w)| x >= f.x - 1.0 && x + w <= f.right() + 1.0 && w < f.w / 10.0)
        .count();
    assert_eq!(cells, 400);
    // …and the colorbar's gradient sits right of the frame, on the figure.
    let bar = rects.iter().filter(|&&(x, _)| x > f.right()).count();
    assert!(bar >= 100, "{bar} colorbar strips");
    assert!(rects.iter().all(|&(x, w)| x + w <= 640.0));
}

#[test]
fn hist2d_leaves_empty_bins_as_background() {
    // A fully-filled mesh draws every cell; a mostly-empty one draws far fewer,
    // because empty bins (no data) are not painted — they show the page
    // background instead of the colormap's value-0 color.
    let rects = |h: &Binned2D| {
        let mut ax = Axes::new();
        ax.hist2d(h);
        occurrences(&ax.to_svg_string(), "<rect")
    };

    let full = binned2d(10, (0.0, 1.0), 10, (0.0, 1.0), |_, _| 1.0);
    // Only two filled cells.
    let sparse = binned2d(10, (0.0, 1.0), 10, (0.0, 1.0), |ix, iy| match (ix, iy) {
        (0, 0) => 5.0,
        (5, 5) => 3.0,
        _ => 0.0,
    });

    let (nf, ns) = (rects(&full), rects(&sparse));
    // Background, frame, and colorbar rects are identical between the two; the
    // ~98-cell difference is exactly the skipped empty bins.
    assert!(
        nf >= ns + 90,
        "empty bins must be skipped: full={nf} rects, sparse={ns} rects"
    );
}

// --- figures and layouts -------------------------------------------------------

#[test]
fn subplots_grid_returns_one_axes_per_cell() {
    let (_fig, axs2) = subplots_grid(2, 2);
    assert_eq!(axs2.len(), 4);
    let (_fig, axs6) = subplots_grid(2, 3);
    assert_eq!(axs6.len(), 6);
}

#[test]
#[cfg(feature = "png")] // renders PNG too
fn figure_grid_renders_all_panels() {
    let h = gauss_hist();
    let (fig, mut axs) = subplots_grid(2, 2);
    axs[0].hist(&h);
    axs[1].errorbar(&graph());
    axs[2].hist2d(&th2());
    axs[3].plot(&[0.0, 1.0, 2.0], &[0.0, 1.0, 0.5]).unwrap();
    let fig = fig.with_axes(axs);
    let png = fig.to_png_bytes(SaveOpts::new()).unwrap();
    assert_eq!(png_dims(&png), (640, 480));
    // Four frames in a 2 × 2 arrangement, none overlapping.
    let f = frames(&fig.to_svg_string());
    assert_eq!(f.len(), 4);
    assert!(close(f[0].x, f[2].x) && close(f[1].x, f[3].x) && f[0].right() < f[1].x);
    assert!(close(f[0].y, f[1].y) && close(f[2].y, f[3].y) && f[0].bottom() < f[2].y);
}

#[test]
fn shared_axes_and_suptitle_add_content() {
    let h = gauss_hist();
    let build = |suptitle: bool| {
        let (mut fig, mut axs) = subplots_grid(1, 2);
        axs[0].hist(&h);
        axs[1].hist(&h);
        fig = fig.sharex().sharey();
        if suptitle {
            fig = fig.suptitle("$Z \\to \\mu\\mu$");
        }
        fig.with_axes(axs).to_svg_string()
    };
    let plain = build(false);
    let titled = build(true);
    // Side by side, the same height; a suptitle that fits the top margin
    // leaves them where they are.
    let (p, t) = (frames(&plain), frames(&titled));
    assert_eq!((p.len(), t.len()), (2, 2));
    assert!(close(p[0].y, p[1].y) && close(p[0].h, p[1].h) && p[0].right() < p[1].x);
    assert_eq!(t, p);
    // The suptitle is typeset as extra glyph paths.
    assert!(
        occurrences(&titled, "<path") > occurrences(&plain, "<path"),
        "suptitle should add glyphs"
    );
}

#[test]
fn ratio_subplots_renders() {
    let (fig, mut main, mut ratio) = ratio_subplots();
    main.hist_with(
        &gauss_hist(),
        HistOpts::new().histtype(HistType::Fill).label("MC"),
    );
    main.ylabel("Events");
    main.legend();
    ratio.errorbar_with(&graph(), ErrorbarOpts::new().color(Color::BLACK));
    ratio.ylim(0.5..1.5);
    ratio.ylabel("data/MC");
    ratio.xlabel("$m$ [GeV]");
    ratio.grid();
    let fig = fig.ratio(main, ratio);
    let pdf = fig.to_pdf_bytes();
    assert!(pdf.starts_with(b"%PDF-1.4") && pdf.ends_with(b"%%EOF\n"));
    // The main panel over the ratio panel: touching, aligned, 3 : 1.
    let f = frames(&fig.to_svg_string());
    assert_eq!(f.len(), 2);
    assert!(close(f[0].bottom(), f[1].y), "the panels do not touch");
    assert!(close(f[0].x, f[1].x) && close(f[0].w, f[1].w));
    assert!(
        (f[0].h / f[1].h - 3.0).abs() < 0.05,
        "{} : {}",
        f[0].h,
        f[1].h
    );
}

#[test]
#[cfg(feature = "png")] // renders PNG too
fn figure_in_memory_render_all_formats() {
    let (fig, mut ax) = subplots();
    ax.hist(&gauss_hist());
    let fig = fig.with_axes([ax]);
    assert!(
        fig.to_png_bytes(SaveOpts::new())
            .unwrap()
            .starts_with(b"\x89PNG")
    );
    assert!(fig.to_svg_string().starts_with("<svg"));
    assert!(fig.to_pdf_bytes().starts_with(b"%PDF-1.4"));
}

// --- edge cases ----------------------------------------------------------------

#[test]
#[cfg(feature = "png")] // renders PNG too
fn empty_axes_renders_without_panicking() {
    let ax = Axes::new();
    let svg = ax.to_svg_string();
    assert_eq!(frames(&svg).len(), 1);
    assert!(circles(&svg).is_empty() && point_lists(&svg, "polyline").is_empty());
    assert!(
        ax.to_png_bytes(SaveOpts::new())
            .unwrap()
            .starts_with(b"\x89PNG")
    );
    assert!(ax.to_pdf_bytes().starts_with(b"%PDF-1.4"));
}

#[test]
fn mplhep_style_renders() {
    let mut ax = Axes::with_style(Style::mplhep());
    ax.hist_with(&gauss_hist(), HistOpts::new().yerr().label("MC"));
    ax.grid_minor();
    ax.legend();
    assert!(ax.to_svg_string().contains("<svg"));
}

// --- fonts ---------------------------------------------------------------------

#[test]
fn builtin_font_sets_render() {
    let h = gauss_hist();
    for fonts in [FontSet::stix(), FontSet::dejavu(), FontSet::default()] {
        let mut ax = Axes::new();
        ax.fonts(fonts);
        ax.hist(&h);
        ax.xlabel("$\\Sigma$ [GeV]");
        let svg = ax.to_svg_string();
        assert!(svg.starts_with("<svg") && svg.contains("<path"));
    }
}

#[test]
fn font_choice_changes_the_glyphs() {
    // Switching the font set must actually change the rendered glyph paths.
    let render = |fonts: FontSet| {
        let mut ax = Axes::new();
        ax.fonts(fonts);
        ax.xlabel("Events / 2 GeV");
        ax.hist(&gauss_hist());
        ax.to_svg_string()
    };
    assert_ne!(
        render(FontSet::stix()),
        render(FontSet::dejavu()),
        "STIX and DejaVu should produce different glyph outlines"
    );
}

#[test]
fn custom_text_font_from_bytes_renders() {
    // A real font, embedded from the crate's own assets (so the test is
    // environment-independent), drives the custom-font path.
    static DEJAVU: &[u8] = include_bytes!("../../assets/DejaVuSans.ttf");
    let fonts = FontSet::from_font(DEJAVU).expect("DejaVu is a valid font");
    let mut ax = Axes::new();
    ax.fonts(fonts);
    ax.hist(&gauss_hist());
    ax.xlabel("custom font");
    assert!(ax.to_svg_string().contains("<path"));
}

#[test]
fn custom_text_and_math_fonts_render() {
    static DEJAVU: &[u8] = include_bytes!("../../assets/DejaVuSans.ttf");
    static MATH: &[u8] = include_bytes!("../../assets/STIXTwoMath-Regular.otf");
    let fonts = FontSet::from_fonts(DEJAVU, MATH).expect("valid text + math fonts");
    let mut ax = Axes::new();
    ax.fonts(fonts);
    ax.xlabel("$\\frac{1}{\\sqrt{2}}$"); // exercises the custom math font
    ax.hist(&gauss_hist());
    assert!(ax.to_svg_string().contains("<path"));
}

#[test]
fn custom_font_rejects_garbage() {
    assert!(FontSet::from_font(b"definitely not a font").is_err());
    static DEJAVU: &[u8] = include_bytes!("../../assets/DejaVuSans.ttf");
    assert!(
        FontSet::from_fonts(DEJAVU, b"not a math font").is_err(),
        "a non-font math argument must be rejected"
    );
}

/// The fit stat box (`Axes::fit_stats`) draws a framed panel of extra glyphs, and
/// its options change what is listed — all rendered deterministically.
#[cfg(feature = "fit")]
#[test]
fn fit_stats_box_renders() {
    use oxifit::{FitExt, Model, Points};
    use oxiplot::StatBox;

    // A clean Gaussian sampled as (x, y, sigma) points, fitted through Points
    // (which implements FitData).
    let xs: Vec<f64> = (0..40).map(|i| 50.0 + 2.0 * i as f64).collect();
    let ys: Vec<f64> = xs
        .iter()
        .map(|&x| 1000.0 * (-0.5 * ((x - 90.0) / 8.0).powi(2)).exp())
        .collect();
    let sig: Vec<f64> = ys.iter().map(|&y| y.sqrt().max(1.0)).collect();
    let model = Model::gaussian("gaus").with_params(vec![1000.0, 90.0, 8.0]);
    let r = Points::new(&xs, &ys, &sig).unwrap().fit(&model);
    let fitted = model.with_params(r.params.clone());

    let render = |stats: Option<StatBox>| {
        let mut ax = Axes::new();
        ax.hist(&gauss_hist());
        ax.model(&fitted, 50.0..130.0);
        if let Some(opts) = stats {
            ax.fit_stats_with(&fitted, &r, opts);
        }
        ax.to_svg_string()
    };

    let without = render(None);
    let with = render(Some(StatBox::new()));
    // The box adds its frame plus a header, a χ²/ndf line, and three parameter
    // rows — many extra glyph paths.
    assert!(
        occurrences(&with, "<path") > occurrences(&without, "<path") + 15,
        "stat box should add many glyph paths: {} -> {}",
        occurrences(&without, "<path"),
        occurrences(&with, "<path"),
    );
    // Enabling the probability line adds one more row of glyphs than the default.
    let with_prob = render(Some(StatBox::new().prob(true)));
    assert!(
        occurrences(&with_prob, "<path") > occurrences(&with, "<path"),
        "the Prob line should add glyphs"
    );
    // Deterministic.
    assert_eq!(with, render(Some(StatBox::new())));
}

// --- multi-page PDF ------------------------------------------------------------

#[test]
fn pdf_pages_writes_one_page_per_figure() {
    // Three distinct figures collected into a single multi-page vector PDF.
    let mut pdf = PdfPages::new();
    for _ in 0..3 {
        let (fig, mut ax) = subplots();
        ax.hist(&gauss_hist());
        pdf.add(&fig.with_axes([ax]));
    }
    assert_eq!(pdf.len(), 3);
    assert!(!pdf.is_empty());

    let bytes = pdf.to_bytes();
    assert!(bytes.starts_with(b"%PDF-1.4") && bytes.ends_with(b"%%EOF\n"));

    let s = String::from_utf8_lossy(&bytes);
    // The page tree advertises three kids...
    assert!(s.contains("/Count 3"), "page tree should hold 3 kids");
    // ...and there are exactly three Page objects (the Pages tree node is
    // "/Type /Pages", so "/Type /Page /Parent" matches only real pages).
    assert_eq!(
        s.matches("/Type /Page /Parent").count(),
        3,
        "one Page object per figure"
    );
}

#[test]
fn pdf_pages_saves_and_matches_single_page_document() {
    let dir = std::env::temp_dir().join("oxiplot_pdfpages_test");
    std::fs::create_dir_all(&dir).unwrap();

    let (fig, mut ax) = subplots();
    ax.hist(&gauss_hist());
    let fig = fig.with_axes([ax]);

    // A one-figure PdfPages document is a valid, single-page PDF on disk.
    let mut pdf = PdfPages::new();
    pdf.add(&fig);
    let path = dir.join("one.pdf");
    pdf.save(&path).unwrap();
    let disk = std::fs::read(&path).unwrap();
    assert!(disk.starts_with(b"%PDF-1.4") && disk.ends_with(b"%%EOF\n"));
    assert_eq!(pdf.len(), 1);
    assert_eq!(
        String::from_utf8_lossy(&disk)
            .matches("/Type /Page /Parent")
            .count(),
        1,
    );
}

// --- log / symlog color norm ---------------------------------------------------

#[test]
#[cfg(feature = "png")] // renders PNG too
fn log_norm_changes_heatmap_and_is_valid() {
    let h = th2();
    let linear = {
        let mut ax = Axes::new();
        ax.hist2d_with(&h, Hist2dOpts::new().label("entries"));
        ax.to_png_bytes(SaveOpts::new()).unwrap()
    };
    let log = {
        let mut ax = Axes::new();
        ax.hist2d_with(&h, Hist2dOpts::new().label("entries").log());
        ax.to_png_bytes(SaveOpts::new()).unwrap()
    };
    // Both render to real PNGs...
    assert!(linear.starts_with(b"\x89PNG") && log.starts_with(b"\x89PNG"));
    // ...but the log norm must actually change the rendered colors.
    assert_ne!(linear, log, "Norm::Log should change the heatmap");
}

#[test]
fn log_norm_colorbar_shows_decade_ticks() {
    // A wide dynamic range so the log colorbar spans multiple decades.
    let w = [1.0, 10.0, 100.0, 1000.0, 10000.0, 100000.0];
    let h = binned2d(6, (0.0, 6.0), 1, (0.0, 1.0), |ix, _| w[ix]);
    let mut ax = Axes::new();
    ax.hist2d_with(&h, Hist2dOpts::new().label("weight").log());
    let svg = ax.to_svg_string();
    assert!(svg.starts_with("<svg"));
    // The colorbar tick labels are drawn as glyph paths, so we can't grep the
    // text out of the SVG directly — but the log render must differ from the
    // linear one, which exercises the decade-tick path end to end.
    let mut ax_lin = Axes::new();
    ax_lin.hist2d_with(&h, Hist2dOpts::new().label("weight"));
    assert_ne!(svg, ax_lin.to_svg_string());
}

#[test]
#[cfg(feature = "png")] // renders PNG too
fn symlog_norm_handles_data_straddling_zero() {
    // A row of cells running from strongly negative to strongly positive — the
    // regime where a plain log norm masks everything and only SymLog works.
    let w = [-1000.0, -10.0, 0.0, 10.0, 1000.0];
    let h = binned2d(5, (0.0, 5.0), 1, (0.0, 1.0), |ix, _| w[ix]);
    let mut ax = Axes::new();
    ax.hist2d_with(
        &h,
        Hist2dOpts::new()
            .label("signed")
            .norm(Norm::SymLog { linthresh: 1.0 }),
    );
    let png = ax.to_png_bytes(SaveOpts::new()).unwrap();
    assert!(png.starts_with(b"\x89PNG"));
}

#[test]
fn plot_rejects_xs_and_ys_of_different_lengths() {
    let mut ax = Axes::new();
    let before = ax.to_svg_string();
    let Err(err) = ax.plot(&[0.0, 1.0, 2.0], &[0.0, 1.0]) else {
        panic!("mismatched xs and ys must be rejected");
    };
    assert!(
        matches!(
            err,
            Error::LengthMismatch {
                what: "plot ys",
                expected: 3,
                found: 2
            }
        ),
        "{err:?}"
    );
    assert_eq!(
        err.to_string(),
        "length mismatch: plot ys has length 2, expected 3"
    );
    // Nothing was drawn, and the color cycle did not advance.
    assert_eq!(ax.to_svg_string(), before);
    ax.plot(&[0.0, 1.0], &[0.0, 1.0]).unwrap();
    assert_ne!(ax.to_svg_string(), before);
}
