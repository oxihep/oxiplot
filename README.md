# oxiplot

[![CI](https://github.com/oxihep/oxiplot/actions/workflows/ci.yml/badge.svg)](https://github.com/oxihep/oxiplot/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust 1.95+](https://img.shields.io/badge/rust-1.95%2B-orange.svg)](https://www.rust-lang.org)

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
oxiplot = { git = "https://github.com/oxihep/oxiplot", tag = "v0.2.1" }
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
histogram styles, ratio plots, grids, colormaps, log scales, fonts and output
options (it uses the `oxiroot::plot` re-export, which is this crate).

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
