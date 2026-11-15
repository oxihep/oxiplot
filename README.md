# oxiplot

[![CI](https://github.com/oxihep/oxiplot/actions/workflows/ci.yml/badge.svg)](https://github.com/oxihep/oxiplot/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust 1.95+](https://img.shields.io/badge/rust-1.95%2B-orange.svg)](https://www.rust-lang.org)

Pure-Rust plotting for high-energy physics: histograms, graphs and 2-D maps to
**SVG, PNG and PDF**, with a matplotlib-like API and an mplhep-style look. No
Python, no matplotlib, no system fonts; `$…$` labels are typeset as TeX math.

Part of the [oxihep](https://github.com/oxihep) family of Rust crates for
high-energy physics. [oxiroot](https://github.com/oxihep/oxiroot)'s histograms
and graphs plot directly (its `plot` feature), and
[oxifit](https://github.com/oxihep/oxifit) results can be overlaid with a stat
box (the `fit` feature here).

| Crate | What it is |
| --- | --- |
| [`oxiplot`](crates/oxiplot) | The plotting library |
| [`oxiplot-tex`](crates/oxiplot-tex) | A trimmed, vendored copy of the [ReX](https://github.com/KenyC/ReX) TeX math layout engine. Internal: no stable API. |

```toml
[dependencies]
oxiplot = { git = "https://github.com/oxihep/oxiplot" }
```

```rust
use oxiplot::{Axes, Binned1D, Color, ErrorbarOpts, HistOpts, HistType, Points};

let edges: Vec<f64> = (0..=50).map(|i| 2.0 * i as f64).collect();
let mc = Binned1D::from_samples(edges, &[40.0, 48.0, 50.0, 52.0, 60.0]);
let data = Points::new(vec![50.0], vec![3.0])?.with_y_errors(vec![1.7])?;

let mut ax = Axes::new();
ax.hist_with(&mc, HistOpts::new().histtype(HistType::Fill).label("MC"));
ax.errorbar_with(&data, ErrorbarOpts::new().color(Color::BLACK).label("data"));
ax.xlabel("$p_T$ [GeV]");
ax.ylabel("Events");
ax.legend();
ax.save("pt.png")?; // or .svg / .pdf
```

## Plotting your own data

Plotting methods take anything that implements one of three small traits:
`Hist1dData` (1-D bins), `Hist2dData` (a 2-D grid) and `PointData` (points with
optional error bars). `Binned1D`, `Binned2D` and `Points` implement them for
plain arrays; implement them yourself for your own types, as oxiroot does for
`Hist1D`, `Hist2D`, `Profile1D` and `Graph`. Like
[UHI](https://uhi.readthedocs.io) in Python, this keeps the plotting library
independent of any one histogram library.

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

## History

These crates were `oxiroot-plot` and `oxiroot-rex` inside the
[oxiroot](https://github.com/oxihep/oxiroot) workspace; their history was
carried over when they moved here.

## License

MIT — see [LICENSE](LICENSE). Bundled fonts and the vendored ReX code carry
their own licenses: see `crates/oxiplot/assets/` and
[`crates/oxiplot-tex/LICENSE-3rdparty`](crates/oxiplot-tex/LICENSE-3rdparty).
