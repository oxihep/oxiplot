//! Data-space bounds and the data→pixel transform.

use crate::draw::{Pt, Rect};

/// A 2-D data-coordinate bounding box, with the smallest strictly positive x
/// and y it holds (what a log axis autoscales to).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    /// Minimum x.
    pub xmin: f64,
    /// Maximum x.
    pub xmax: f64,
    /// Minimum y.
    pub ymin: f64,
    /// Maximum y.
    pub ymax: f64,
    /// Smallest x > 0 (∞ when there is none).
    pub xpos: f64,
    /// Smallest y > 0 (∞ when there is none).
    pub ypos: f64,
}

/// The smallest of `a` and `b` that is > 0, or ∞.
fn least_positive(a: f64, b: f64) -> f64 {
    [a, b]
        .into_iter()
        .filter(|&v| v > 0.0)
        .fold(f64::INFINITY, f64::min)
}

impl Bounds {
    /// A bounds from explicit extents.
    #[must_use]
    pub fn new(xmin: f64, xmax: f64, ymin: f64, ymax: f64) -> Self {
        Bounds {
            xmin,
            xmax,
            ymin,
            ymax,
            xpos: least_positive(xmin, xmax),
            ypos: least_positive(ymin, ymax),
        }
    }

    /// The union of two bounds.
    #[must_use]
    pub fn union(self, other: Bounds) -> Bounds {
        Bounds {
            xmin: self.xmin.min(other.xmin),
            xmax: self.xmax.max(other.xmax),
            ymin: self.ymin.min(other.ymin),
            ymax: self.ymax.max(other.ymax),
            xpos: self.xpos.min(other.xpos),
            ypos: self.ypos.min(other.ypos),
        }
    }
}

/// How an axis maps data onto its length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Scale {
    /// Evenly spaced values.
    #[default]
    Linear,
    /// Base-10 logarithmic: each decade takes the same length. Values ≤ 0
    /// cannot be placed; they are drawn far beyond the low edge and clipped
    /// (matplotlib's `nonpositive="clip"`), so a histogram's bars and error
    /// bars run off the bottom instead of vanishing.
    Log,
}

/// Where `v` sits along `[lo, hi]` as a fraction (0 at `lo`, 1 at `hi`).
fn fraction(v: f64, lo: f64, hi: f64, scale: Scale) -> f64 {
    let (v, lo, hi) = match scale {
        Scale::Linear => (v, lo, hi),
        Scale::Log => {
            if v <= 0.0 || lo <= 0.0 || hi <= 0.0 {
                // Far below the axis: off the frame, so clipped.
                return -10.0;
            }
            (v.log10(), lo.log10(), hi.log10())
        }
    };
    let span = hi - lo;
    if span.abs() < f64::EPSILON {
        0.5
    } else {
        (v - lo) / span
    }
}

/// Maps data coordinates to pixel coordinates within an axes box. The y axis is
/// inverted (larger data y → smaller pixel y), matching screen conventions.
#[derive(Debug, Clone, Copy)]
pub struct Transform {
    box_: Rect,
    xmin: f64,
    xmax: f64,
    ymin: f64,
    ymax: f64,
    xscale: Scale,
    yscale: Scale,
}

impl Transform {
    /// Build a linear transform mapping `[xmin,xmax]×[ymin,ymax]` onto `box_`.
    #[must_use]
    pub fn new(box_: Rect, xmin: f64, xmax: f64, ymin: f64, ymax: f64) -> Self {
        Transform {
            box_,
            xmin,
            xmax,
            ymin,
            ymax,
            xscale: Scale::Linear,
            yscale: Scale::Linear,
        }
    }

    /// The same transform with the given axis scales.
    #[must_use]
    pub fn with_scales(mut self, xscale: Scale, yscale: Scale) -> Self {
        self.xscale = xscale;
        self.yscale = yscale;
        self
    }

    /// Map a data x to a pixel x.
    #[must_use]
    pub fn x(&self, dx: f64) -> f32 {
        let t = fraction(dx, self.xmin, self.xmax, self.xscale);
        self.box_.x + (t as f32) * self.box_.w
    }

    /// Map a data y to a pixel y (inverted).
    #[must_use]
    pub fn y(&self, dy: f64) -> f32 {
        let t = fraction(dy, self.ymin, self.ymax, self.yscale);
        self.box_.bottom() - (t as f32) * self.box_.h
    }

    /// The pixel rectangle the transform maps onto.
    #[must_use]
    pub fn frame(&self) -> Rect {
        self.box_
    }

    /// Map a data point to a pixel point.
    #[must_use]
    pub fn pt(&self, p: (f64, f64)) -> Pt {
        (self.x(p.0), self.y(p.1))
    }
}
