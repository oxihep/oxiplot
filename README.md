# oxiplot

[![CI](https://github.com/oxihep/oxiplot/actions/workflows/ci.yml/badge.svg)](https://github.com/oxihep/oxiplot/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust 1.100+](https://img.shields.io/badge/rust-1.100%2B-orange.svg)](https://www.rust-lang.org)
[![API docs](https://img.shields.io/badge/docs-API-blue.svg)](https://oxihep.github.io/oxiplot/oxiplot/)

Pure-Rust plotting for high-energy physics: histograms, graphs and 2-D maps to
**SVG, PNG and PDF**, with a matplotlib-like API and an mplhep-style look. No
Python, no matplotlib, no system fonts; `$…$` labels are typeset as TeX math.
Fonts (STIX Two by default, bundled) are read in pure Rust with
[skrifa](https://crates.io/crates/skrifa).

| Crate | What it is |
| --- | --- |
| [`oxiplot`](crates/oxiplot) | The plotting library |
| [`oxiplot-tex`](crates/oxiplot-tex) | A trimmed, vendored copy of the [ReX](https://github.com/KenyC/ReX) TeX math layout engine. Internal: no stable API. |

```toml
[dependencies]
oxiplot = { git = "https://github.com/oxihep/oxiplot", tag = "v0.4.1" }
```

```rust
use oxiplot::{Axes, Binned1D, Color, ErrorbarOpts, HistOpts, HistType, Points};

let edges: Vec<f64> = (0..=50).map(|i| 2.0 * i as f64).collect();
let mc = Binned1D::from_samples(edges, &[40.0, 48.0, 50.0, 52.0, 60.0]);
let data = Points::new(vec![50.0], vec![3.0]).unwrap().with_y_errors(vec![1.7]).unwrap();

let mut ax = Axes::new();
ax.hist_with(&mc, HistOpts::new().histtype(HistType::Fill).label("MC"));
ax.errorbar_with(&data, ErrorbarOpts::new().color(Color::BLACK).label("data"));
ax.xlabel("$p_T$ [GeV]");
ax.ylabel("Events");
ax.legend();
ax.save("pt.png").unwrap(); // or .svg / .pdf
```

## What it draws

- **Histograms** in the mplhep styles (step, fill, error bars, uncertainty
  band), stacked (`hist_stack`) or over category bins, **graphs** and **profiles** with error bars, **2-D histograms** as
  a colour mesh with a colorbar (linear, log or symlog colour scale), and
  analytic **curves**.
- **Linear or log axes** (`ax.yscale(Scale::Log)`), with decade ticks
  labelled 10ᵏ, and **time axes**.
- A **legend** placed where it hides the least data, as matplotlib's
  `loc="best"` does, or at a chosen corner (`ax.legend_loc(Loc::UpperLeft)`).
- **Multi-panel figures**: grids (`subplots_grid`, `GridSpec`) and the HEP
  ratio plot (`ratio_subplots`). The figure makes room for each panel's tick
  labels, axis labels and title, and the suptitle, so nothing overlaps, and
  lines up the y-axis labels of a column.
- **Output** to PNG, SVG and PDF, in memory or to a file, and multi-page PDFs
  (`PdfPages`). PDF pages are the figure's size in points at any DPI.

```rust
use oxiplot::{Binned1D, HistOpts, HistType, Scale, Style, ratio_subplots_with};

let edges: Vec<f64> = (0..=25).map(|i| 20.0 * i as f64).collect();
let falling: Vec<f64> = (0..25).map(|i| 1e6 * (-(i as f64) / 3.0).exp()).collect();
let h = Binned1D::new(edges, falling).unwrap();

let (fig, mut main, mut ratio) = ratio_subplots_with(Style::mplhep());
main.hist_with(&h, HistOpts::new().histtype(HistType::Fill).label("background"));
main.yscale(Scale::Log).ylabel("Events").hep_label("Demo", "Simulation").legend();
ratio.function(|_| 1.0, 0.0..500.0);
ratio.ylim(0.5..1.5).ylabel("Ratio").xlabel("$p_T$ [GeV]");
fig.ratio(main, ratio).save("spectrum.pdf").unwrap();
```

## Plotting your own data

Plotting methods take anything that implements one of three small traits:
`Hist1dData` (1-D bins), `Hist2dData` (a 2-D grid) and `PointData` (points with
optional error bars). `Binned1D`, `Binned2D` and `Points` implement them for
plain arrays; implement them yourself for your own types, as oxiroot does for
`Hist1D`, `Hist2D`, `Profile1D` and `Graph`. Like
[UHI](https://uhi.readthedocs.io) in Python, this keeps the plotting library
independent of any one histogram library: [oxihist](https://github.com/oxihep/oxihist)
histograms plot with its `plot` feature, oxiroot's ROOT histograms with oxiroot's.

The [plotting guide](https://oxihep.github.io/oxiroot/guide/plotting/) covers
histogram styles, ratio plots, grids, legends, colormaps, log axes, fonts
and output options (it uses the `oxiroot::plot` re-export, which is this crate).

## Features

| Feature | Default | Adds |
| --- | --- | --- |
| `png` | yes | PNG output, through the tiny-skia rasterizer |
| `math` | yes | `$…$` labels typeset as TeX math (oxiplot-tex); without it they are plain text |
| `fit` | no | `Axes::model` and `Axes::fit_stats` to overlay an `oxifit::Model` and its fit results |

SVG and PDF output need no feature.

## The oxihep family

Rust crates for high-energy physics, split the way Python's
[Scikit-HEP](https://scikit-hep.org) is. Each works on its own; together they
cover reading data, histogramming, fitting and plotting.

| Crate | What it does | Python counterpart |
| --- | --- | --- |
| [oxiroot](https://github.com/oxihep/oxiroot) | Read and write ROOT files: `TTree`, RNTuple, ROOT histograms and graphs | uproot |
| [oxihist](https://github.com/oxihep/oxihist) | Histograms: typed axes, weighted and profile storages | boost-histogram, hist |
| **oxiplot** (this crate) | Plotting to SVG/PNG/PDF with an mplhep look | matplotlib, mplhep |
| [oxifit](https://github.com/oxihep/oxifit) | Curve fitting with Minuit2, and a `TFormula` engine | iminuit |
| [oxistats](https://github.com/oxihep/oxistats) | Statistics, HEP lineshapes and intervals | scipy.stats, hepstats |
| [oxiparticle](https://github.com/oxihep/oxiparticle) | PDG particle data and ID decoding | particle |

The crates are not on crates.io yet: depend on them from GitHub, pinned to a
release tag, as shown above.

## History

These crates were `oxiroot-plot` and `oxiroot-rex` inside the
[oxiroot](https://github.com/oxihep/oxiroot) workspace; their history was
carried over when they moved here.

## License

MIT — see [LICENSE](LICENSE). Bundled fonts and the vendored ReX code carry
their own licenses: see `crates/oxiplot/assets/` and
[`crates/oxiplot-tex/LICENSE-3rdparty`](crates/oxiplot-tex/LICENSE-3rdparty).
