# Changelog

Notable changes to oxiplot and oxiplot-tex, by release. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.7.1]

### Changed

- **oxifit 0.4.1** behind the `fit` feature: its numerical `integrate` no
  longer hangs on a narrow peak or a `NaN` integrand.

## [0.7.0]

### Changed

- **oxifit 0.4.0** behind the `fit` feature (unbinned and extended
  likelihood fits, toy studies, simultaneous fits). No change to oxiplot's
  own API; a minor version because oxifit's types appear in it.

## [0.6.0]

### Added

- **Fit contours and profiles** (the `fit` feature): `Axes::fit_contour`
  draws a 2-D confidence contour from oxifit's `FitExt::contour` as a closed
  line with the best fit marked; `Axes::fit_profile` draws a profile scan
  from `FitExt::profile` with dashed reference lines at `Δ = 1` and `Δ = 4`
  (the 1σ and 2σ crossings). Both have `_with` variants taking `CurveOpts`.
- The `fit_scans` example: 68 % and 95 % contours of a resonance's mean and
  width, and the profile of its mean.

### Changed

- **oxifit 0.3.0** behind the `fit` feature (CMA-ES and Levenberg–Marquardt,
  sum models, Gaussian constraints, profiles and contours).

## [0.5.0]

### Changed

- **oxifit 0.2.0.** The `fit` feature now builds on oxifit 0.2.0, whose
  `FitResult` is `#[non_exhaustive]`: build one by hand with
  `FitResult::new(params, errors, chi2, ndf, valid)`. A breaking change for
  code that passes oxifit types to `Axes::fit_stats` and friends, hence the
  minor version.
- **The fit statistics box leaves out χ²/ndf and Prob for a fit without a
  χ²** (a robust-loss fit, whose `chi2` oxifit 0.2 reports as `NaN`) instead
  of printing `NaN`.

## [0.4.3]

### Fixed

- **The legend's `Loc::Best` could sit on a filled histogram.** It counted
  only a histogram's outline as data, so the inside of a filled histogram or
  a stack looked empty and attracted the legend. The area under a `Fill`
  histogram, and between a `Band`'s edges, now counts too.

### Changed

- The `hep_label` documentation uses a neutral `"Demo"` label in its example
  and notes that a collaboration's name belongs only on its own results.

### Added

- Examples: `data_mc_ratio` (stacked backgrounds, data and a ratio panel on
  a log scale), `efficiency` (a trigger turn-on with asymmetric Wilson
  errors), `occupancy_map` (a 15 360-cell detector map drawn as one image),
  `energy_resolution` (σ/E and its terms on log axes) and
  `control_plots_report` (a multi-page PDF).

## [0.4.2]

There is no 0.4.1: a `v0.4.1` tag was cut by mistake on the 0.4.0 commit
and has been deleted. 0.4.2 is the release with this fix.

### Fixed

- `Error::Io` reports its `std::io::Error` through `Error::source`, so an
  error chain (`anyhow`, `?` into another error type) keeps the cause.

## [0.4.0]

Output changes again (hence 0.4): SVGs are sized in points, large heatmaps
become one image, and narrow axes gain ticks. The additions (stacks,
category axes) are compatible; no public item was removed or renamed. The
`fit` feature uses oxifit 0.1.2.

### Added

- **Stacked histograms.** `Axes::hist_stack(&[&a, &b, …])` and
  `hist_stack_with(…, StackOpts)` draw layers that share a binning on top of
  each other (mplhep's `histplot(stack=True)`): filled by default, a label
  and colour per layer, the legend reading top to bottom. Layers of another
  binning are refused with the new `Error::BinningMismatch`.
- **Category axes.** `Hist1dData::x_labels` names the bins of a categorical
  histogram; plotting then labels each bin at its centre instead of drawing
  numeric ticks (and no minor ticks). `Binned1D::categories(&labels,
  values)` and `Binned1D::with_labels` build one from arrays.
- **Golden-image tests** (`tests/it/golden.rs`): five complete plots
  compared with reference PNGs in `tests/golden/`, with a tolerance for
  anti-aliasing differences between CPUs. Re-record after an intended change
  with `OXIPLOT_BLESS=1 cargo test -p oxiplot --test it golden`.

### Changed

- The legend's `Loc::Best` placement keeps off the fit-statistics box, as it
  does off the data; the box then stays in its corner.
- **SVGs are sized in points.** The `<svg>` element's `width`/`height` are
  now the figure's physical size (`460.8pt × 345.6pt` for 6.4 × 4.8 in, as
  matplotlib writes them), matching the PDF; the drawing keeps its pixel
  `viewBox`. They were the pixel size, so an SVG opened larger than the PDF.
- **Large heatmaps are one image.** A `hist2d` of 64 × 64 bins or more that
  is evenly spaced on screen (a regular binning on linear axes) is drawn as
  one image, a pixel per bin with no smoothing, instead of a rectangle per
  bin: a 200 × 200 map is an 86 kB SVG and a 210 kB PDF rather than 40 000
  shapes. Empty bins stay transparent. Needs the `png` feature (for the
  SVG's embedded PNG); without it, and for smaller or uneven grids, the
  per-bin rectangles remain.
- **A narrow axis gets at least two ticks.** The tick step could round so
  far up that one tick fell inside (a 150 px wide `[-3, 3]` axis showed only
  `0`); it now drops to the next smaller nice step until two do.

## [0.3.1]

### Fixed

- **Error bars pulled a histogram's y axis below zero.** 0.3.0 let negative
  bins extend the range below 0, but error bars reaching below an empty
  bin's count (data points at 0 with a ±1 bar, say) did so too, leaving a
  gap under the histogram. Only a negative bin content or point now takes
  the range below 0.

## [0.3.0]

Several defaults change how an existing plot looks (hence 0.3): the legend
moves off the data, figures widen their margins and gaps when labels need
room, PDF pages shrink to the figure's real size, and a histogram's y range
extends below 0 for negative bins. No public item was removed or renamed.

### Added

- `Axes::plot_with` (color, legend label, width, dash for a line through
  points) and `Axes::profile_with` (the `ErrorbarOpts` of `errorbar_with`),
  so plotted lines and profiles can appear in the legend.
- **Legend placement.** `Axes::legend_loc(Loc)` puts the legend at a corner,
  edge or the centre. `Axes::legend` now uses `Loc::Best`, matplotlib's
  default: the spot covering the fewest data points, upper right first, so a
  legend no longer hides a peak in the upper right.
- `PdfPages::add_axes` adds a single axes as a page.
- **Log axes.** `Axes::xscale(Scale::Log)` / `Axes::yscale(Scale::Log)`:
  decades evenly spaced, major ticks at the powers of ten labelled `10^k`
  (1·/2·/5· or integer multiples when the range spans under two decades),
  minor ticks at 2…9 per decade, and autoscaling to the positive data. A
  histogram on a log y axis starts below its smallest positive bin; values
  ≤ 0 are clipped off the bottom, as matplotlib's `nonpositive="clip"`.

### Changed

- **Rust 1.100 and edition 2024.** The minimum supported Rust version
  (`rust-version`) moves from 1.95 to 1.100, the current stable release that
  CI already builds with, and the crates move to the 2024 edition (migrated
  with `cargo fix --edition`; formatting follows the 2024 style).
- **Figures lay out their panels around their labels.** The grid's margins
  and gaps are now the least room left: they widen wherever a panel's tick
  labels, axis labels or title, or the suptitle, need more (matplotlib's
  constrained layout). Before, a 2×2 grid printed each top panel's x label
  over the panel below it and the suptitle over the titles. Touching panels
  (`hspace(0.0)`, a ratio plot) stay touching. A single `Axes::save` uses the
  same layout, so long tick labels no longer leave the figure.
- **More axes than grid cells add rows.** `Figure::with_axes` drew the
  extras over the last cell; they now get cells in new rows below.
- **A shared x axis keeps the histogram edges.** The shared range was the
  union of every panel's autoscaled range, so a ratio panel's padded points
  pulled the main histogram off the frame. It is now the union of the
  ranges the panels declare (`xlim`, a histogram's edges), when any does.
- The y-axis labels of a column line up (as `fig.align_ylabels()` does in
  matplotlib), so a ratio plot's two labels share one x.

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
- **PDF pages were 39% too large.** A page was sized in pixels as if they
  were points, so a 6.4 × 4.8 in figure became an 8.9 × 6.7 in page (and
  42 in wide at 300 dpi), with text past its point size. Pages are now the
  figure's size in points at any dpi.
- **A colorbar's label crossed its tick labels**, and wide tick labels
  (`80000`) ran past the figure: the label was anchored on the wrong side and
  the bar's room was a fixed 46 pt. The room now fits the labels.
- A y-axis label's descenders reached into the tick labels (it was anchored
  at its baseline, not its bottom).
- **A title collided with the experiment label** (`hep_label`/`hep_rhs`); it
  now sits above that row.
- **Tick labels overlapped at the seams of stacked shared-x panels**
  (`sharex` with `hspace(0.0)`): each panel below the first drops its top
  label, as the ratio panel of `ratio_subplots` already did.
- A fit-statistics box stacked under the legend only when they overlap, not
  whenever the legend exists.


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
