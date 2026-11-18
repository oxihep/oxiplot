//! Tick location and formatting — a "nice numbers" locator (like matplotlib's
//! `MaxNLocator`) and a plain decimal/scientific formatter (`ScalarFormatter`).

/// Round `x` to a "nice" number (1, 2, 5, or 10 × a power of ten).
fn nice_num(x: f64, round: bool) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let exp = x.log10().floor();
    let frac = x / 10f64.powf(exp);
    let nice = if round {
        if frac < 1.5 {
            1.0
        } else if frac < 3.0 {
            2.0
        } else if frac < 7.0 {
            5.0
        } else {
            10.0
        }
    } else if frac <= 1.0 {
        1.0
    } else if frac <= 2.0 {
        2.0
    } else if frac <= 5.0 {
        5.0
    } else {
        10.0
    };
    nice * 10f64.powf(exp)
}

/// Major tick step for a `[lo, hi]` range targeting roughly `target` ticks.
#[must_use]
pub fn nice_step(lo: f64, hi: f64, target: usize) -> f64 {
    let span = (hi - lo).abs();
    if span < f64::EPSILON || !span.is_finite() {
        return 1.0;
    }
    let range = nice_num(span, false);
    nice_num(range / (target.max(2) - 1) as f64, true)
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
        let v = (first + k as f64) * step;
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

/// Number of decimal places needed to show ticks at the given step.
fn decimals_for_step(step: f64) -> usize {
    if step <= 0.0 || !step.is_finite() {
        return 1;
    }
    let d = -step.log10().floor();
    d.clamp(0.0, 10.0) as usize
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
        (max_abs.log10().floor() - step.log10().floor()).max(0.0) as usize
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
    let mut exp = v.abs().log10().floor() as i32;
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
    fn sci_mantissa_rounding_carries_into_the_exponent() {
        assert_eq!(format_sci(9.9996e5, 2), "1e6");
        assert_eq!(format_sci(-2.5e-7, 1), "-2.5e-7");
    }
}
