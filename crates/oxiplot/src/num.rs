//! The float → integer and f64 → f32 conversions plotting needs, each
//! justified once here rather than at every use.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "each function states the range that makes its cast exact or saturating"
)]

/// A position or size in pixels: the rasterizer works in `f32`, and figure
/// coordinates are far inside its range (data far outside the frame lands
/// off-canvas either way).
pub(crate) fn to_px(v: f64) -> f32 {
    v as f32
}

/// The decimal exponent `floor(v)` of a finite `log10`: within ±330, so it
/// fits an `i32` (a non-finite value saturates).
pub(crate) fn floor_i32(v: f64) -> i32 {
    v.floor() as i32
}

/// As [`floor_i32`], rounding up.
pub(crate) fn ceil_i32(v: f64) -> i32 {
    v.ceil() as i32
}

/// As [`floor_i32`], rounding to the nearest.
pub(crate) fn round_i32(v: f64) -> i32 {
    v.round() as i32
}

/// An image dimension in pixels: rounded, at least 0 (saturating).
pub(crate) fn round_u32(v: f64) -> u32 {
    v.round().max(0.0) as u32
}

/// A color channel from a value in `0..=255` (clamped first).
pub(crate) fn channel(v: f64) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

/// A count or an index from a non-negative value (at least 0, saturating).
pub(crate) fn to_index(v: f64) -> usize {
    v.max(0.0) as usize
}

/// A whole number of seconds or points from a finite value (saturating; a
/// non-finite one gives 0).
pub(crate) fn round_i64(v: f64) -> i64 {
    v.round() as i64
}
