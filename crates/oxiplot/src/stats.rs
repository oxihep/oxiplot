//! The little statistics plotting needs: the exact Poisson interval a count
//! is drawn with.

/// `ln Γ(x)` for `x > 0` (Lanczos, g = 7, n = 9: about 15 digits).
fn ln_gamma(x: f64) -> f64 {
    const G: f64 = 7.0;
    const C: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    let x = x - 1.0;
    let mut a = C[0];
    for (i, c) in C.iter().enumerate().skip(1) {
        a += c / (x + i as f64);
    }
    let t = x + G + 0.5;
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

/// The regularized lower incomplete gamma function `P(a, x)`: the
/// probability that a Gamma(`a`) variable is below `x`.
fn gamma_p(a: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let front = (a * x.ln() - x - ln_gamma(a)).exp();
    if x < a + 1.0 {
        // The series converges fast below the mean.
        let (mut term, mut sum, mut n) = (1.0 / a, 1.0 / a, a);
        for _ in 0..500 {
            n += 1.0;
            term *= x / n;
            sum += term;
            if term.abs() < sum.abs() * 1e-15 {
                break;
            }
        }
        (front * sum).clamp(0.0, 1.0)
    } else {
        // Above it, the continued fraction for Q = 1 − P (modified Lentz).
        let tiny = 1e-300;
        let mut b = x + 1.0 - a;
        let mut c = 1.0 / tiny;
        let mut d = 1.0 / b;
        let mut h = d;
        for i in 1..500 {
            let i = f64::from(i);
            let an = -i * (i - a);
            b += 2.0;
            d = an * d + b;
            if d.abs() < tiny {
                d = tiny;
            }
            c = b + an / c;
            if c.abs() < tiny {
                c = tiny;
            }
            d = 1.0 / d;
            let delta = d * c;
            h *= delta;
            if (delta - 1.0).abs() < 1e-15 {
                break;
            }
        }
        (1.0 - front * h).clamp(0.0, 1.0)
    }
}

/// The `x` at which `P(a, x) = p`, by bisection (`P` rises from 0 to 1).
fn gamma_p_inverse(a: f64, p: f64) -> f64 {
    let (mut lo, mut hi) = (0.0, a + 10.0 * a.sqrt() + 20.0);
    while gamma_p(a, hi) < p {
        hi *= 2.0;
    }
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if gamma_p(a, mid) < p {
            lo = mid;
        } else {
            hi = mid;
        }
        if hi - lo <= 1e-12 * hi.max(1.0) {
            break;
        }
    }
    0.5 * (lo + hi)
}

/// The exact (Garwood) central 68.27 % interval of a Poisson mean given
/// `count` observed events: the `(lower, upper)` limits. The lower limit of
/// zero events is zero. This is the error bar a bin of data is drawn with.
pub(crate) fn poisson_interval(count: f64) -> (f64, f64) {
    // One standard deviation on each side: (1 − 0.6827) / 2.
    const TAIL: f64 = 0.158_655_253_931_457_05;
    let lower = if count > 0.0 {
        gamma_p_inverse(count, TAIL)
    } else {
        0.0
    };
    (lower, gamma_p_inverse(count + 1.0, 1.0 - TAIL))
}

/// Whether `v` is a count: a non-negative whole number.
pub(crate) fn is_count(v: f64) -> bool {
    v >= 0.0 && v.is_finite() && (v - v.round()).abs() < 1e-9
}

#[cfg(test)]
mod tests {
    use super::{is_count, poisson_interval};

    /// The Garwood limits, against the published table (the PDG's Table 40.3
    /// values at 68.27 %, and ROOT's `TH1::kPoisson` error bars).
    #[test]
    fn poisson_intervals_match_the_table() {
        for (n, lo, hi) in [
            (0.0, 0.0, 1.841),
            (1.0, 0.173, 3.300),
            (2.0, 0.708, 4.638),
            (5.0, 2.840, 8.382),
            (10.0, 6.891, 14.267),
            (100.0, 90.017, 111.034),
        ] {
            let (l, h) = poisson_interval(n);
            assert!((l - lo).abs() < 2e-3, "{n}: lower {l} vs {lo}");
            assert!((h - hi).abs() < 2e-3, "{n}: upper {h} vs {hi}");
        }
    }

    #[test]
    fn a_count_is_a_non_negative_whole_number() {
        assert!(is_count(0.0) && is_count(7.0));
        assert!(!is_count(2.5) && !is_count(-1.0) && !is_count(f64::NAN));
    }
}
