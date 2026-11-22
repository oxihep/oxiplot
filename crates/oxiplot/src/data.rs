//! The data the plotting methods draw, as traits: [`Hist1dData`] for 1-D binned
//! values ([`Axes::hist`](crate::Axes::hist),
//! [`Axes::profile`](crate::Axes::profile)), [`Hist2dData`] for a 2-D grid
//! ([`Axes::hist2d`](crate::Axes::hist2d)), and [`PointData`] for points with
//! optional error bars ([`Axes::errorbar`](crate::Axes::errorbar)).
//!
//! Any data can implement them: [oxiroot](https://github.com/oxihep/oxiroot)'s
//! histograms and graphs do (its `plot` feature). For data you already hold as
//! plain arrays, [`Binned1D`], [`Binned2D`] and [`XyPoints`] implement them for
//! you, the way mplhep's `histplot(values, bins)` takes arrays.

use crate::error::{Error, Result};

/// A 1-D binned distribution: `n` in-range bins.
pub trait Hist1dData {
    /// The `n + 1` bin edges, ascending.
    fn edges(&self) -> Vec<f64>;
    /// The `n` in-range bin contents.
    fn values(&self) -> Vec<f64>;
    /// The uncertainty of in-range bin `i` (`0..n`).
    fn error(&self, i: usize) -> f64;
    /// The x axis's ROOT time format, when its values are times
    /// (`fTimeFormat` on a time axis). Plotting picks it up, so a histogram
    /// with a time axis draws dates and clock times instead of numbers.
    fn x_time_format(&self) -> Option<String> {
        None
    }
    /// One label per in-range bin, when the bins are categories (a string or
    /// integer category axis) rather than numeric intervals. Plotting then
    /// labels each bin at its centre instead of drawing numeric ticks.
    fn x_labels(&self) -> Option<Vec<String>> {
        None
    }
    /// The content and uncertainty of the underflow bin (everything below
    /// the first edge), when the data keeps one. [`Flow`](crate::Flow)
    /// decides whether a plot shows it.
    fn underflow(&self) -> Option<(f64, f64)> {
        None
    }
    /// The content and uncertainty of the overflow bin (everything at or
    /// above the last edge), when the data keeps one.
    fn overflow(&self) -> Option<(f64, f64)> {
        None
    }
}

/// A 2-D binned distribution: `nx × ny` in-range bins.
pub trait Hist2dData {
    /// The `nx + 1` x edges, ascending.
    fn x_edges(&self) -> Vec<f64>;
    /// The `ny + 1` y edges, ascending.
    fn y_edges(&self) -> Vec<f64>;
    /// The in-range contents, indexed `[x][y]`.
    fn grid(&self) -> Vec<Vec<f64>>;
}

/// Points with optional error bars.
pub trait PointData {
    /// The x coordinates.
    fn xs(&self) -> Vec<f64>;
    /// The y coordinates, one per x.
    fn ys(&self) -> Vec<f64>;
    /// The `(low, high)` x errors, one pair of values per point, if any.
    fn x_errors(&self) -> Option<(Vec<f64>, Vec<f64>)> {
        None
    }
    /// The `(low, high)` y errors, one pair of values per point, if any.
    fn y_errors(&self) -> Option<(Vec<f64>, Vec<f64>)> {
        None
    }

    /// The x axis's ROOT time format, when its values are times (`fTimeFormat`
    /// on a time axis). Plotting picks it up, so a time axis draws as times.
    fn x_time_format(&self) -> Option<String> {
        None
    }
}

/// 1-D binned data held as plain arrays: `n + 1` edges, `n` values, and
/// optional per-bin errors.
///
/// Without explicit errors a bin's error is `√|value|`, the Poisson error of a
/// count (mplhep's default).
///
/// ```
/// use oxiplot::{Axes, Binned1D};
///
/// let h = Binned1D::new(vec![0.0, 1.0, 2.0, 3.0], vec![4.0, 9.0, 1.0])?;
/// let mut ax = Axes::new();
/// ax.hist(&h);
/// # Ok::<(), oxiplot::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Binned1D {
    edges: Vec<f64>,
    values: Vec<f64>,
    errors: Option<Vec<f64>>,
    labels: Option<Vec<String>>,
}

impl Binned1D {
    /// Bins with the given `edges` (`n + 1`, ascending) and `values` (`n`).
    ///
    /// # Errors
    ///
    /// [`Error::LengthMismatch`] if `edges` is not one longer than `values`.
    pub fn new(edges: Vec<f64>, values: Vec<f64>) -> Result<Self> {
        check_len("Binned1D edges", values.len() + 1, edges.len())?;
        Ok(Binned1D {
            edges,
            values,
            errors: None,
            labels: None,
        })
    }

    /// Count `samples` into bins with the given `edges` (`n + 1`, ascending).
    /// A bin holds `[lo, hi)`, except the last, which also holds its upper
    /// edge; samples outside the edges, and NaN, are dropped.
    pub fn from_samples(edges: Vec<f64>, samples: &[f64]) -> Self {
        let n = edges.len().saturating_sub(1);
        let mut values = vec![0.0; n];
        for &x in samples {
            if n == 0 || !(edges[0]..=edges[n]).contains(&x) {
                continue;
            }
            // The first edge above x, minus one; x == the last edge goes in the last bin.
            let i = edges
                .partition_point(|&e| e <= x)
                .saturating_sub(1)
                .min(n - 1);
            values[i] += 1.0;
        }
        Binned1D {
            edges,
            values,
            errors: None,
            labels: None,
        }
    }

    /// Categories: `n` values, one per label, on unit-width bins `[i, i + 1)`.
    /// Plotting labels each bin with its category instead of numeric ticks.
    ///
    /// ```
    /// use oxiplot::{Axes, Binned1D};
    /// let h = Binned1D::categories(&["ee", "μμ", "eμ"], vec![120.0, 135.0, 18.0])?;
    /// let mut ax = Axes::new();
    /// ax.hist(&h);
    /// # Ok::<(), oxiplot::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`Error::LengthMismatch`] if there is not one value per label.
    pub fn categories<S: AsRef<str>>(labels: &[S], values: Vec<f64>) -> Result<Self> {
        check_len("Binned1D values", labels.len(), values.len())?;
        let edges = (0..=labels.len()).map(|i| i as f64).collect();
        Binned1D::new(edges, values)?.with_labels(labels)
    }

    /// Label the bins as categories, one label per bin.
    ///
    /// # Errors
    ///
    /// [`Error::LengthMismatch`] if there is not one label per value.
    pub fn with_labels<S: AsRef<str>>(mut self, labels: &[S]) -> Result<Self> {
        check_len("Binned1D labels", self.values.len(), labels.len())?;
        self.labels = Some(labels.iter().map(|l| l.as_ref().to_owned()).collect());
        Ok(self)
    }

    /// Set explicit per-bin errors (`n`, one per value).
    ///
    /// # Errors
    ///
    /// [`Error::LengthMismatch`] if there is not one error per value.
    pub fn with_errors(mut self, errors: Vec<f64>) -> Result<Self> {
        check_len("Binned1D errors", self.values.len(), errors.len())?;
        self.errors = Some(errors);
        Ok(self)
    }
}

impl Hist1dData for Binned1D {
    fn edges(&self) -> Vec<f64> {
        self.edges.clone()
    }
    fn values(&self) -> Vec<f64> {
        self.values.clone()
    }
    fn error(&self, i: usize) -> f64 {
        match &self.errors {
            Some(e) => e[i],
            None => self.values[i].abs().sqrt(),
        }
    }
    fn x_labels(&self) -> Option<Vec<String>> {
        self.labels.clone()
    }
}

/// 2-D binned data held as plain arrays: `nx + 1` x edges, `ny + 1` y edges,
/// and an `nx × ny` grid of values indexed `[x][y]`.
///
/// ```
/// use oxiplot::{Axes, Binned2D};
///
/// let h = Binned2D::new(
///     vec![0.0, 1.0, 2.0],
///     vec![0.0, 1.0, 2.0],
///     vec![vec![1.0, 2.0], vec![3.0, 0.0]],
/// )?;
/// let mut ax = Axes::new();
/// ax.hist2d(&h);
/// # Ok::<(), oxiplot::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Binned2D {
    x_edges: Vec<f64>,
    y_edges: Vec<f64>,
    grid: Vec<Vec<f64>>,
}

impl Binned2D {
    /// Bins with the given edges and a grid of values indexed `[x][y]`.
    ///
    /// # Errors
    ///
    /// [`Error::LengthMismatch`] if the grid is not `nx` columns of `ny`
    /// values for the given edges.
    pub fn new(x_edges: Vec<f64>, y_edges: Vec<f64>, grid: Vec<Vec<f64>>) -> Result<Self> {
        let nx = x_edges.len().saturating_sub(1);
        let ny = y_edges.len().saturating_sub(1);
        check_len("Binned2D grid columns", nx, grid.len())?;
        for col in &grid {
            check_len("Binned2D grid column", ny, col.len())?;
        }
        Ok(Binned2D {
            x_edges,
            y_edges,
            grid,
        })
    }
}

impl Hist2dData for Binned2D {
    fn x_edges(&self) -> Vec<f64> {
        self.x_edges.clone()
    }
    fn y_edges(&self) -> Vec<f64> {
        self.y_edges.clone()
    }
    fn grid(&self) -> Vec<Vec<f64>> {
        self.grid.clone()
    }
}

/// x–y points held as plain arrays, with optional symmetric or asymmetric error
/// bars.
///
/// ```
/// use oxiplot::{Axes, XyPoints};
///
/// let pts = XyPoints::new(vec![1.0, 2.0, 3.0], vec![2.0, 4.0, 3.0])?
///     .with_y_errors(vec![0.3, 0.5, 0.4])?;
/// let mut ax = Axes::new();
/// ax.errorbar(&pts);
/// # Ok::<(), oxiplot::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct XyPoints {
    xs: Vec<f64>,
    ys: Vec<f64>,
    x_errors: Option<(Vec<f64>, Vec<f64>)>,
    y_errors: Option<(Vec<f64>, Vec<f64>)>,
}

impl XyPoints {
    /// Points at `(xs[i], ys[i])`, with no error bars.
    ///
    /// # Errors
    ///
    /// [`Error::LengthMismatch`] if `ys` and `xs` differ in length.
    pub fn new(xs: Vec<f64>, ys: Vec<f64>) -> Result<Self> {
        check_len("XyPoints ys", xs.len(), ys.len())?;
        Ok(XyPoints {
            xs,
            ys,
            x_errors: None,
            y_errors: None,
        })
    }

    /// Symmetric x error bars, one per point.
    ///
    /// # Errors
    ///
    /// [`Error::LengthMismatch`] if there is not one error per point.
    pub fn with_x_errors(self, err: Vec<f64>) -> Result<Self> {
        self.with_x_errors_asym(err.clone(), err)
    }

    /// Asymmetric x error bars: the distances below and above each point.
    ///
    /// # Errors
    ///
    /// [`Error::LengthMismatch`] if there is not one pair per point.
    pub fn with_x_errors_asym(mut self, low: Vec<f64>, high: Vec<f64>) -> Result<Self> {
        check_len("XyPoints x errors (low)", self.xs.len(), low.len())?;
        check_len("XyPoints x errors (high)", self.xs.len(), high.len())?;
        self.x_errors = Some((low, high));
        Ok(self)
    }

    /// Symmetric y error bars, one per point.
    ///
    /// # Errors
    ///
    /// [`Error::LengthMismatch`] if there is not one error per point.
    pub fn with_y_errors(self, err: Vec<f64>) -> Result<Self> {
        self.with_y_errors_asym(err.clone(), err)
    }

    /// Asymmetric y error bars: the distances below and above each point.
    ///
    /// # Errors
    ///
    /// [`Error::LengthMismatch`] if there is not one pair per point.
    pub fn with_y_errors_asym(mut self, low: Vec<f64>, high: Vec<f64>) -> Result<Self> {
        check_len("XyPoints y errors (low)", self.xs.len(), low.len())?;
        check_len("XyPoints y errors (high)", self.xs.len(), high.len())?;
        self.y_errors = Some((low, high));
        Ok(self)
    }
}

/// The edges and contents of two histograms compared bin by bin, with the
/// uncertainties of each.
struct Pair {
    edges: Vec<f64>,
    a: Vec<f64>,
    a_err: Vec<f64>,
    b: Vec<f64>,
    b_err: Vec<f64>,
}

/// Read `a` and `b` for a bin-by-bin comparison.
///
/// # Errors
///
/// [`Error::BinningMismatch`] if they do not share their edges.
fn paired(a: &impl Hist1dData, b: &impl Hist1dData, what: &str) -> Result<Pair> {
    let (edges, other) = (a.edges(), b.edges());
    let same = edges.len() == other.len()
        && edges
            .iter()
            .zip(&other)
            .all(|(x, y)| (x - y).abs() <= 1e-9 * x.abs().max(y.abs()).max(1.0));
    if !same {
        return Err(Error::BinningMismatch(format!(
            "{what}: the two histograms have {} and {} bin edges, or edges that differ",
            edges.len(),
            other.len()
        )));
    }
    let (av, bv) = (a.values(), b.values());
    Ok(Pair {
        a_err: (0..av.len()).map(|i| a.error(i)).collect(),
        b_err: (0..bv.len()).map(|i| b.error(i)).collect(),
        a: av,
        b: bv,
        edges,
    })
}

impl XyPoints {
    /// The ratio of two histograms of one binning, as points at the bin
    /// centres: what the lower panel of a data-over-simulation plot shows.
    ///
    /// Each point is `numerator / denominator` with the numerator's
    /// uncertainty over the denominator. The denominator's own uncertainty
    /// is not folded in: it is drawn as a band around one, which
    /// [`Binned1D::relative_uncertainty`] gives. A bin where the denominator
    /// is zero, or either content is not finite, has no point.
    ///
    /// ```
    /// use oxiplot::{Binned1D, PointData, XyPoints};
    /// let data = Binned1D::new(vec![0.0, 1.0, 2.0], vec![90.0, 0.0])?;
    /// let model = Binned1D::new(vec![0.0, 1.0, 2.0], vec![100.0, 0.0])?;
    /// let ratio = XyPoints::ratio(&data, &model)?;
    /// assert_eq!((ratio.xs(), ratio.ys()), (vec![0.5], vec![0.9]));
    /// # Ok::<(), oxiplot::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`Error::BinningMismatch`] if the two do not share their bin edges.
    pub fn ratio(numerator: &impl Hist1dData, denominator: &impl Hist1dData) -> Result<Self> {
        let p = paired(numerator, denominator, "ratio")?;
        Self::from_bins(&p, |a, a_err, b, _| {
            (b != 0.0).then(|| (a / b, a_err / b.abs()))
        })
    }

    /// The pull of `data` from `model` in each bin, `(data − model) / σ`,
    /// with `σ` the two uncertainties in quadrature: the lower panel of a
    /// fit. A bin with no uncertainty has no point.
    ///
    /// # Errors
    ///
    /// [`Error::BinningMismatch`] if the two do not share their bin edges.
    pub fn pull(data: &impl Hist1dData, model: &impl Hist1dData) -> Result<Self> {
        let p = paired(data, model, "pull")?;
        let pulls = Self::from_bins(&p, |a, a_err, b, b_err| {
            let sigma = a_err.hypot(b_err);
            (sigma > 0.0).then(|| ((a - b) / sigma, 0.0))
        })?;
        // A pull is in units of its own uncertainty: it carries no error bar.
        Ok(XyPoints {
            y_errors: None,
            ..pulls
        })
    }

    /// The difference `a − b` in each bin, with the two uncertainties in
    /// quadrature: data minus background.
    ///
    /// # Errors
    ///
    /// [`Error::BinningMismatch`] if the two do not share their bin edges.
    pub fn difference(a: &impl Hist1dData, b: &impl Hist1dData) -> Result<Self> {
        let p = paired(a, b, "difference")?;
        Self::from_bins(&p, |a, a_err, b, b_err| Some((a - b, a_err.hypot(b_err))))
    }

    /// One point per bin of `p` where `f(a, a_err, b, b_err)` gives a value
    /// and its uncertainty, at the bin centre.
    fn from_bins(p: &Pair, f: impl Fn(f64, f64, f64, f64) -> Option<(f64, f64)>) -> Result<Self> {
        let (mut xs, mut ys, mut errs) = (Vec::new(), Vec::new(), Vec::new());
        for i in 0..p.a.len().min(p.b.len()) {
            let (a, b) = (p.a[i], p.b[i]);
            if !a.is_finite() || !b.is_finite() {
                continue;
            }
            if let Some((y, err)) = f(a, p.a_err[i], b, p.b_err[i])
                && y.is_finite()
            {
                xs.push(0.5 * (p.edges[i] + p.edges[i + 1]));
                ys.push(y);
                errs.push(if err.is_finite() { err } else { 0.0 });
            }
        }
        XyPoints::new(xs, ys)?.with_y_errors(errs)
    }
}

impl Binned1D {
    /// The relative uncertainty of `h` as a histogram of ones: each bin holds
    /// `1 ± error / |content|`. Drawn as a band
    /// ([`HistType::Band`](crate::HistType::Band)) in a ratio panel, it shows
    /// the denominator's uncertainty around the line at one. An empty bin
    /// gets no uncertainty.
    #[must_use]
    pub fn relative_uncertainty(h: &impl Hist1dData) -> Self {
        let values = h.values();
        let errors = values
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                let e = h.error(i) / v.abs();
                if e.is_finite() { e } else { 0.0 }
            })
            .collect();
        Binned1D {
            edges: h.edges(),
            values: vec![1.0; values.len()],
            errors: Some(errors),
            labels: None,
        }
    }
}

impl PointData for XyPoints {
    fn xs(&self) -> Vec<f64> {
        self.xs.clone()
    }
    fn ys(&self) -> Vec<f64> {
        self.ys.clone()
    }
    fn x_errors(&self) -> Option<(Vec<f64>, Vec<f64>)> {
        self.x_errors.clone()
    }
    fn y_errors(&self) -> Option<(Vec<f64>, Vec<f64>)> {
        self.y_errors.clone()
    }
}

fn check_len(what: &'static str, expected: usize, found: usize) -> Result<()> {
    if expected == found {
        Ok(())
    } else {
        Err(Error::LengthMismatch {
            what,
            expected,
            found,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_samples_bins_half_open_with_closed_last_bin() {
        let h = Binned1D::from_samples(
            vec![0.0, 1.0, 2.0],
            &[0.0, 0.5, 1.0, 2.0, 2.5, -1.0, f64::NAN],
        );
        assert_eq!(h.values(), vec![2.0, 2.0]);
        assert_eq!(h.error(0), 2f64.sqrt());
    }

    #[test]
    fn lengths_are_checked() {
        assert!(Binned1D::new(vec![0.0, 1.0], vec![1.0, 2.0]).is_err());
        assert!(
            Binned1D::new(vec![0.0, 1.0], vec![1.0])
                .unwrap()
                .with_errors(vec![])
                .is_err()
        );
        assert!(Binned2D::new(vec![0.0, 1.0], vec![0.0, 1.0, 2.0], vec![vec![1.0]]).is_err());
        assert!(XyPoints::new(vec![1.0], vec![]).is_err());
        let p = XyPoints::new(vec![1.0, 2.0], vec![3.0, 4.0]).unwrap();
        assert!(p.clone().with_y_errors(vec![0.1]).is_err());
        let p = p
            .with_y_errors_asym(vec![0.1, 0.2], vec![0.3, 0.4])
            .unwrap();
        assert_eq!(p.y_errors(), Some((vec![0.1, 0.2], vec![0.3, 0.4])));
        assert_eq!(p.x_errors(), None);
    }
}
