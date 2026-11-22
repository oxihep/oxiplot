//! Tick location and formatting — a "nice numbers" locator (like matplotlib's
//! `MaxNLocator`) and a plain decimal/scientific formatter (`ScalarFormatter`).

/// Major tick step for a `[lo, hi]` range: the smallest "nice" step (1, 2 or
/// 5 times a power of ten) that puts at most `target` ticks in the range, as
/// matplotlib's `MaxNLocator` does.
///
/// Choosing the smallest step that fits, and not rounding the range up
/// first, keeps an axis from being left with two labels: `[-0.55, 1.55]`
/// with 7 ticks wanted steps by 0.5 (five ticks), not by 1 (two).
#[must_use]
pub fn nice_step(lo: f64, hi: f64, target: usize) -> f64 {
    let span = (hi - lo).abs();
    if span < f64::EPSILON || !span.is_finite() {
        return 1.0;
    }
    let (lo, hi) = (lo.min(hi), lo.max(hi));
    let target = target.max(2);
    let inside = |step: f64| {
        let tol = step * 1e-6;
        ((hi + tol) / step).floor() - ((lo - tol) / step).ceil() + 1.0
    };
    // Start a decade below the step that would give `target` intervals: no
    // smaller step can fit.
    let exponent = crate::num::floor_i32((span / target as f64).log10()) - 1;
    // A power of ten below one as a division, so that 2 × 10⁻¹ is exactly 0.2.
    let scaled = |mantissa: f64, e: i32| {
        if e < 0 {
            mantissa / 10f64.powi(-e)
        } else {
            mantissa * 10f64.powi(e)
        }
    };
    let mut fallback = scaled(1.0, exponent);
    for decade in 0..4 {
        for mantissa in [1.0, 2.0, 5.0] {
            let step = scaled(mantissa, exponent + decade);
            let count = inside(step);
            if count >= 2.0 {
                fallback = step;
            }
            if count <= target as f64 {
                // The first step that fits; if it leaves fewer than two
                // ticks, the finer one before it reads better.
                return if count >= 2.0 { step } else { fallback };
            }
        }
    }
    fallback
}

/// Major tick positions within `[lo, hi]` (inclusive, with a tiny tolerance).
#[must_use]
pub fn ticks(lo: f64, hi: f64, target: usize) -> Vec<f64> {
    if !lo.is_finite() || !hi.is_finite() {
        return Vec::new();
    }
    let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
    let step = nice_step(lo, hi, target);
    if step <= 0.0 || !step.is_finite() {
        return vec![lo, hi];
    }
    let first = (lo / step).floor();
    let mut out = Vec::new();
    let tol = step * 1e-6;
    // Each tick is `k · step`, not a running sum, so rounding does not build
    // up (100000.6, not 100000.59999999999). The loop is bounded defensively.
    for k in 0..1000 {
        let v = (first + f64::from(k)) * step;
        if v > hi + tol {
            break;
        }
        if v >= lo - tol {
            // Snap values extremely close to zero to exactly zero.
            out.push(if v.abs() < tol { 0.0 } else { v });
        }
    }
    if out.is_empty() {
        out.push(lo);
        out.push(hi);
    }
    out
}

/// Minor tick positions between the majors (matplotlib `AutoMinorLocator`):
/// subdivide each major interval into `n` parts.
#[must_use]
pub fn minor_ticks(lo: f64, hi: f64, majors: &[f64], n: usize) -> Vec<f64> {
    if majors.len() < 2 || n < 2 {
        return Vec::new();
    }
    let step = majors[1] - majors[0];
    let sub = step / n as f64;
    let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
    let tol = step * 1e-6;
    let mut out = Vec::new();
    // Extend one major step beyond the ends so partial intervals get minors too.
    let start = majors[0] - step;
    let mut v = start;
    for _ in 0..10000 {
        if v > hi + tol {
            break;
        }
        // Skip positions coinciding with a major.
        let on_major = ((v - majors[0]) / step).round();
        let nearest_major = majors[0] + on_major * step;
        if (v - nearest_major).abs() > tol && v >= lo - tol && v <= hi + tol {
            out.push(v);
        }
        v += sub;
    }
    out
}

/// Major ticks of a log axis over `[lo, hi]` (both > 0): the powers of ten
/// inside, thinned to every n-th decade so there are at most `target`; when
/// fewer than two decades fall inside, the 1·, 2·, 5· multiples, or every
/// integer multiple when those are still fewer than three.
#[must_use]
pub fn log_ticks(lo: f64, hi: f64, target: usize) -> Vec<f64> {
    let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
    if !(lo > 0.0 && hi.is_finite()) {
        return Vec::new();
    }
    let tol = 1e-9;
    let inside = |v: f64| v >= lo * (1.0 - tol) && v <= hi * (1.0 + tol);
    let k0 = crate::num::floor_i32(lo.log10());
    let k1 = crate::num::ceil_i32(hi.log10());
    let decades: Vec<i32> = (k0..=k1).filter(|&k| inside(10f64.powi(k))).collect();
    if decades.len() >= 2 {
        let every = decades.len().div_ceil(target.max(2)).max(1);
        return decades
            .iter()
            .filter(|&&k| k.rem_euclid(i32::try_from(every).unwrap_or(i32::MAX)) == 0)
            .map(|&k| 10f64.powi(k))
            .collect();
    }
    let multiples = |ms: &[f64]| -> Vec<f64> {
        (k0..=k1)
            .flat_map(|k| ms.iter().map(move |m| m * 10f64.powi(k)))
            .filter(|&v| inside(v))
            .collect()
    };
    let coarse = multiples(&[1.0, 2.0, 5.0]);
    if coarse.len() >= 3 {
        return coarse;
    }
    // Under a decade: every integer multiple, thinned to the target.
    let fine = multiples(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]);
    let every = fine.len().div_ceil(target.max(2)).max(1);
    fine.into_iter().step_by(every).collect()
}

/// Minor ticks of a log axis: 2·…9· each decade, inside `[lo, hi]`, when
/// the axis spans few enough decades for them to be told apart.
#[must_use]
pub fn log_minor_ticks(lo: f64, hi: f64) -> Vec<f64> {
    let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
    if !(lo > 0.0 && hi.is_finite()) || (hi / lo).log10() > 12.0 {
        return Vec::new();
    }
    let (k0, k1) = (
        crate::num::floor_i32(lo.log10()),
        crate::num::ceil_i32(hi.log10()),
    );
    (k0..=k1)
        .flat_map(|k| (2..10).map(move |m| f64::from(m) * 10f64.powi(k)))
        .filter(|&v| v >= lo && v <= hi)
        .collect()
}

/// Labels for log-axis ticks: a power of ten as `10^k` (typeset as TeX math
/// with the `math` feature; `1e3` without it), anything else as a plain
/// number (`20`, `0.05`).
#[must_use]
pub fn format_log_ticks(values: &[f64]) -> Vec<String> {
    values
        .iter()
        .map(|&v| {
            let k = v.log10().round();
            if (v / 10f64.powf(k) - 1.0).abs() < 1e-9 {
                let k = crate::num::round_i32(k);
                if cfg!(feature = "math") {
                    format!("$10^{{{k}}}$")
                } else {
                    format!("1e{k}")
                }
            } else if (1e-4..1e5).contains(&v.abs()) {
                let s = format!("{v:.6}");
                s.trim_end_matches('0').trim_end_matches('.').to_string()
            } else {
                format_sci(v, 1)
            }
        })
        .collect()
}

/// Number of decimal places needed to show ticks at the given step.
fn decimals_for_step(step: f64) -> usize {
    if step <= 0.0 || !step.is_finite() {
        return 1;
    }
    let d = -step.log10().floor();
    crate::num::to_index(d.clamp(0.0, 10.0))
}

/// Format a set of tick values (a plain `ScalarFormatter`). Uses fixed decimals
/// derived from the step, switching to scientific notation for very large or
/// very small magnitudes.
#[must_use]
pub fn format_ticks(values: &[f64], step: f64) -> Vec<String> {
    let max_abs = values.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let decimals = decimals_for_step(step);
    // Digits after the mantissa's point that tell neighbouring ticks apart:
    // 1e5 … 3e5 in steps of 1e5 need none, 1.00e-5 … 1.10e-5 in steps of
    // 2e-7 need two.
    let sci_digits = if max_abs > 0.0 && step > 0.0 && step.is_finite() {
        crate::num::to_index((max_abs.log10().floor() - step.log10().floor()).max(0.0))
    } else {
        0
    };
    // Scientific notation for very large or very small magnitudes, unless the
    // ticks are so close together (relative to their size) that it would need
    // a long mantissa: 100000.2, 100000.4 read better as they are.
    let use_sci = max_abs != 0.0 && !(1e-4..1e5).contains(&max_abs) && sci_digits <= 3;
    values
        .iter()
        .map(|&v| {
            let v = if v == 0.0 { 0.0 } else { v }; // normalize -0
            if use_sci {
                format_sci(v, sci_digits.max(1))
            } else {
                let s = format!("{v:.decimals$}");
                // Normalize a "-0" / "-0.00" result.
                if s.trim_start_matches('-')
                    .chars()
                    .all(|c| c == '0' || c == '.')
                {
                    s.trim_start_matches('-').to_string()
                } else {
                    s
                }
            }
        })
        .collect()
}

/// `v` as `<mantissa>e<exp>`, with up to `digits` decimals in the mantissa
/// (trailing zeros dropped).
fn format_sci(v: f64, digits: usize) -> String {
    if v == 0.0 {
        return "0".to_string();
    }
    let mut exp = crate::num::floor_i32(v.abs().log10());
    let mut mantissa = v / 10f64.powi(exp);
    // Rounding can carry the mantissa to 10 (9.996 → "10.00"): renormalize.
    if format!("{:.digits$}", mantissa.abs()).starts_with("10") {
        exp += 1;
        mantissa = v / 10f64.powi(exp);
    }
    let m = format!("{mantissa:.digits$}");
    let m = m.trim_end_matches('0').trim_end_matches('.');
    format!("{m}e{exp}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(lo: f64, hi: f64, target: usize) -> Vec<String> {
        format_ticks(&ticks(lo, hi, target), nice_step(lo, hi, target))
    }

    fn distinct(v: &[String]) -> bool {
        v.windows(2).all(|w| w[0] != w[1])
    }

    #[test]
    fn ticks_are_exact_multiples_of_the_step() {
        let t = ticks(100000.0, 100001.0, 6);
        assert_eq!(t[3], 100000.6);
    }

    /// An axis is filled up to the number of ticks asked for, not left with
    /// the two that a rounded-up range would give.
    #[test]
    fn the_step_is_the_finest_that_fits() {
        assert_eq!(ticks(-0.55, 1.55, 7), [-0.5, 0.0, 0.5, 1.0, 1.5]);
        assert_eq!(ticks(50.0, 130.0, 3), [50.0, 100.0]);
        assert_eq!(ticks(50.0, 130.0, 4), [60.0, 80.0, 100.0, 120.0]);
        assert_eq!(
            labels(0.0, 1.0, 6),
            ["0.0", "0.2", "0.4", "0.6", "0.8", "1.0"]
        );
        for target in 2..12 {
            let n = ticks(0.0277, 0.0287, target).len();
            assert!((2..=target).contains(&n), "{target} wanted, {n} drawn");
        }
    }

    #[test]
    fn a_narrow_axis_still_gets_two_ticks() {
        // A 150 px axis asks for 3 ticks; [-3, 3] at a step of 5 had one.
        assert_eq!(ticks(-3.0, 3.0, 3), [-2.0, 0.0, 2.0]);
        assert!(ticks(0.1, 0.9, 3).len() >= 2);
        // Ranges that already had two or more ticks keep their step.
        assert_eq!(nice_step(0.0, 10.0, 6), 2.0);
        assert_eq!(nice_step(0.0, 1000.0, 3), 500.0);
    }

    #[test]
    fn non_finite_ranges_have_no_ticks() {
        assert!(ticks(f64::NAN, 1.0, 5).is_empty());
        assert!(ticks(0.0, f64::INFINITY, 5).is_empty());
    }

    #[test]
    fn neighbouring_labels_never_read_the_same() {
        for (lo, hi) in [
            (100000.0, 100001.0),
            (1.0e-5, 1.1e-5),
            (0.0, 3e5),
            (123456.0, 123789.0),
            (0.0, 1.0),
            (-2.5e-6, 2.5e-6),
        ] {
            let l = labels(lo, hi, 6);
            assert!(distinct(&l), "{lo}..{hi}: {l:?}");
        }
        assert_eq!(labels(0.0, 3e5, 6), ["0", "1e5", "2e5", "3e5"]);
        assert_eq!(labels(100000.0, 100001.0, 6)[1], "100000.2");
    }

    #[test]
    fn log_ticks_step_by_decades() {
        assert_eq!(log_ticks(1.0, 1e4, 6), [1.0, 10.0, 100.0, 1000.0, 1e4]);
        // Thinned to every other decade past the target.
        let wide = log_ticks(1e-10, 1e10, 6);
        assert!((3..=6).contains(&wide.len()), "{wide:?}");
        assert!(
            log_ticks(1e-10, 1e10, 6)
                .iter()
                .all(|v| crate::num::round_i32(v.log10()) % 4 == 0)
        );
        // Under two decades: 1·, 2·, 5·.
        assert_eq!(log_ticks(3.0, 60.0, 6), [5.0, 10.0, 20.0, 50.0]);
        // Under a decade: integer multiples.
        assert_eq!(
            log_ticks(18.0, 75.0, 6),
            [20.0, 30.0, 40.0, 50.0, 60.0, 70.0]
        );
        assert!(log_ticks(0.0, 10.0, 6).is_empty());
        assert!(log_ticks(1.0, f64::INFINITY, 6).is_empty());
    }

    #[test]
    fn log_minor_ticks_fill_each_decade() {
        let m = log_minor_ticks(1.0, 100.0);
        assert_eq!(m.len(), 16);
        assert!(m.contains(&2.0) && m.contains(&90.0) && !m.contains(&10.0));
    }

    #[test]
    fn log_labels_write_powers_of_ten() {
        let l = format_log_ticks(&[0.01, 1.0, 1000.0, 20.0, 2e-7]);
        if cfg!(feature = "math") {
            assert_eq!(l[..3], ["$10^{-2}$", "$10^{0}$", "$10^{3}$"]);
        } else {
            assert_eq!(l[..3], ["1e-2", "1e0", "1e3"]);
        }
        assert_eq!(l[3..], ["20", "2e-7"]);
    }

    #[test]
    fn sci_mantissa_rounding_carries_into_the_exponent() {
        assert_eq!(format_sci(9.9996e5, 2), "1e6");
        assert_eq!(format_sci(-2.5e-7, 1), "-2.5e-7");
    }
}
