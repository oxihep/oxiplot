# Changelog

Notable changes to oxiplot and oxiplot-tex, by release. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `Axes::plot_with` (color, legend label, width, dash for a line through
  points) and `Axes::profile_with` (the `ErrorbarOpts` of `errorbar_with`),
  so plotted lines and profiles can appear in the legend.

### Fixed

- **Limits set before plotting were overwritten.** `xlim` then `hist`
  widened the range to the histogram's edges, and `hist2d` replaced both
  limits outright. A histogram's edges are now the default range (its
  "sticky edges") and the caller's `xlim`/`ylim` always win.
- **Negative bins were cut off.** A histogram's y range started at 0 even
  when bins went below it (weighted or background-subtracted histograms); an
  all-negative histogram showed nothing. The range now stands on 0 and
  extends below it when the data does.
- **NaN or infinite values broke the output.** They reached the SVG as
  `NaN` coordinates (an invalid document), and an infinite bin or a NaN
  limit made the axis range non-finite (one plot produced 1.2 MB of `NaN`
  tick paths). Non-finite bins and points are now skipped (a bin draws
  empty), and limits always resolve to a finite, nonzero range.
- **`HistType::Band` drew nothing without `.yerr()`.** Band and errorbar
  histograms now carry their errors on their own, as in mplhep.
- **A `$…$` legend label made the legend too wide.** The frame was sized to
  the label's TeX source rather than its typeset width (2.4× too wide for
  `$\sqrt{s} = 13\,\mathrm{TeV}$`).
- **Tick labels could all read the same.** Scientific labels kept one
  decimal, so ticks from 100000 to 100001 all read `1e5`. Labels now keep
  enough digits to differ, falling back to plain decimals when scientific
  notation would need a long mantissa; tick positions no longer gather
  rounding error (`100000.6`, not `100000.59999999999`).
- `Axes::profile` now picks up a time axis from its data, as `hist` and
  `errorbar` do.

### Changed

- **Rust 1.100 and edition 2024.** The minimum supported Rust version
  (`rust-version`) moves from 1.95 to 1.100, the current stable release that
  CI already builds with, and the crates move to the 2024 edition (migrated
  with `cargo fix --edition`; formatting follows the 2024 style).

## [0.2.1]

### Fixed

- **Subscripts sat too low.** oxiplot-tex (from upstream ReX) took the
  default subscript drop from the font's `subscriptTopMax` instead of
  `subscriptShiftDown`, so a subscript was pushed down further than the font
  asks wherever that value decided its position. It now reads
  `subscriptShiftDown`, as TeX and MathML do. Only vertical positions of
  subscripts change; 16 of ReX's 154 recorded test renders were re-recorded
  (`OXIPLOT_TEX_BLESS=1 cargo test -p oxiplot-tex --test history`).

## [0.2.0]

### Changed

- **Fonts are read with skrifa (Google Fonts' fontations) instead of
  ttf-parser and ab_glyph.** ttf-parser is unmaintained (RUSTSEC-2026-0192),
  and ab_glyph depends on it. Text, tick labels and `$…$` math draw the same:
  PNG output is pixel-identical on the bundled STIX Two and DejaVu fonts, and
  SVG paths lose only a redundant zero-length segment before each `Z`.
  `FontSet` is unchanged.
- **Breaking (oxiplot-tex):** the math font backend is
  `oxiplot_tex::font::backend::skrifa::SkrifaMathFont` (built from font
  bytes or a `skrifa::FontRef`); `font::backend::ttf_parser::TtfMathFont`
  is gone. oxiplot-tex is internal to oxiplot and has no stability promise,
  but its version moves with oxiplot's.

### Fixed

- oxiplot-tex reads an `ssty` single substitution (GSUB format 2) through
  its coverage index; upstream ReX searched the substitute array instead,
  which could return the wrong glyph for a script-size form.

## [0.1.1]

### Changed

- Depends on oxifit 0.1.1 (formula-parser limits). CI pins its toolchain and
  checks advisories with cargo-deny.

## [0.1.0]

First release under the oxiplot names. The code is `oxiroot-plot` and
`oxiroot-rex` 0.3.0 from the [oxiroot](https://github.com/oxihep/oxiroot)
workspace, moved to their own repository with their history.

### Added

- `Binned1D`, `Binned2D` and `Points`: plain-array data that implements
  `Hist1dData`, `Hist2dData` and `PointData`, so anything can be plotted
  without a histogram library. `Binned1D::from_samples` counts samples into
  bins.

### Changed

- Renamed `oxiroot-plot` to `oxiplot` and `oxiroot-rex` to `oxiplot-tex`
  (oxiroot users keep `oxiroot::plot`, which re-exports `oxiplot`).
- **oxiplot no longer depends on oxiroot.** The `hist` feature is gone: oxiroot
  now implements the data traits for its own types (its `plot` feature), and the
  `From<oxiplot::Error> for oxiroot::Error` conversion moved to oxiroot too.
- The `fit` feature depends on `oxifit` (formerly `oxiroot-fit`).
