//! The data the plotting methods draw, as traits: [`Hist1dData`] for 1-D binned
//! values ([`Axes::hist`](crate::Axes::hist),
//! [`Axes::profile`](crate::Axes::profile)), [`Hist2dData`] for a 2-D grid
//! ([`Axes::hist2d`](crate::Axes::hist2d)), and [`PointData`] for points with
//! optional error bars ([`Axes::errorbar`](crate::Axes::errorbar)).
//!
//! Any data can implement them: [oxiroot](https://github.com/oxihep/oxiroot)'s
//! histograms and graphs do (its `plot` feature). For data you already hold as
//! plain arrays, [`Binned1D`], [`Binned2D`] and [`Points`] implement them for
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
        }
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

/// Points held as plain arrays, with optional symmetric or asymmetric error
/// bars.
///
/// ```
/// use oxiplot::{Axes, Points};
///
/// let pts = Points::new(vec![1.0, 2.0, 3.0], vec![2.0, 4.0, 3.0])?
///     .with_y_errors(vec![0.3, 0.5, 0.4])?;
/// let mut ax = Axes::new();
/// ax.errorbar(&pts);
/// # Ok::<(), oxiplot::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Points {
    xs: Vec<f64>,
    ys: Vec<f64>,
    x_errors: Option<(Vec<f64>, Vec<f64>)>,
    y_errors: Option<(Vec<f64>, Vec<f64>)>,
}

impl Points {
    /// Points at `(xs[i], ys[i])`, with no error bars.
    ///
    /// # Errors
    ///
    /// [`Error::LengthMismatch`] if `ys` and `xs` differ in length.
    pub fn new(xs: Vec<f64>, ys: Vec<f64>) -> Result<Self> {
        check_len("Points ys", xs.len(), ys.len())?;
        Ok(Points {
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
        check_len("Points x errors (low)", self.xs.len(), low.len())?;
        check_len("Points x errors (high)", self.xs.len(), high.len())?;
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
        check_len("Points y errors (low)", self.xs.len(), low.len())?;
        check_len("Points y errors (high)", self.xs.len(), high.len())?;
        self.y_errors = Some((low, high));
        Ok(self)
    }
}

impl PointData for Points {
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
        assert!(Binned1D::new(vec![0.0, 1.0], vec![1.0])
            .unwrap()
            .with_errors(vec![])
            .is_err());
        assert!(Binned2D::new(vec![0.0, 1.0], vec![0.0, 1.0, 2.0], vec![vec![1.0]]).is_err());
        assert!(Points::new(vec![1.0], vec![]).is_err());
        let p = Points::new(vec![1.0, 2.0], vec![3.0, 4.0]).unwrap();
        assert!(p.clone().with_y_errors(vec![0.1]).is_err());
        let p = p
            .with_y_errors_asym(vec![0.1, 0.2], vec![0.3, 0.4])
            .unwrap();
        assert_eq!(p.y_errors(), Some((vec![0.1, 0.2], vec![0.3, 0.4])));
        assert_eq!(p.x_errors(), None);
    }
}
