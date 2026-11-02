//! Mandelbrot membership: the first [`Membership`] instance, with exact
//! certificates only.
//!
//! The orbit is `z₀ = 0`, `zₙ₊₁ = zₙ² + c`, computed in exact rationals
//! (`i128` numerator and denominator, every operation checked).
//!
//! - **`Out`:** some `|zₙ|² > 4`, compared exactly. Every `c` outside `M`
//!   escapes at a finite `n` (the escape lemma), so `Out` is semi-decidable
//!   and the witness is `(n, zₙ)`. The escape time is the witness, not loss.
//! - **`In`, interior tests (tried first, before any iteration):** with
//!   `c = x + iy` and `q = (x - ¼)² + y²`, the closed main cardioid
//!   `q(q + (x - ¼)) ≤ y²/4` and the closed period-2 bulb
//!   `(x + 1)² + y² ≤ 1/16`, each decided exactly. Both lie in `M`, which
//!   is closed. Derivations: the cardioid is `c = μ/2 - μ²/4`, `|μ| ≤ 1`
//!   (an attracting or indifferent fixed point with multiplier `μ`); around
//!   `¼` that is `ρ ≤ (1 - cos θ)/2`, and the polynomial form reduces to
//!   `(ρ + cos θ / 2)² ≤ ¼`, the same condition. The 2-cycle's multiplier
//!   is `4(c + 1)`, so the bulb is `|c + 1| ≤ ¼`. Where a point also has an
//!   exact cycle (0, -1), the interior proof is the one reported; the cycle
//!   is still a valid certificate and still verifies.
//! - **`In`, exact recurrence:** `zₘ₊ₚ = zₘ` in exact rationals. The orbit
//!   is then eventually periodic, so bounded, so `c ∈ M`.
//! - **`In`, exact real invariant interval:** for real `c`, an interval `I`
//!   with `0 ∈ I` and `f(I) ⊆ I`, checked exactly. The candidate is
//!   `[-β, β]` with `β = (1 + √(1 - 4c)) / 2`, tried only when that root
//!   is rational. This certifies boundary points such as `c = ¼` (the cusp)
//!   and `c = -¾`, whose orbits are not periodic.
//! - **`Undecided`:** the budget ran out ([`Stop::Budget`]) or exact
//!   arithmetic outgrew `i128` ([`Stop::Precision`]; denominators square at
//!   every step). A float or grain-level cycle is never `In` here: nothing
//!   in this module compares rounded values.
//! - **`Failure`:** non-finite input (NaN, infinity), refused by [`of_f64`].
//!
//! The boundary of `M` is not "where `Partial` never resolves": `c = -2`
//! and `c = ¼` lie on it and resolve `In` exactly. What never resolves is
//! the `c` for which no finite certificate exists in the certificate
//! language used.
//!
//! Every certificate can be re-checked independently with
//! [`InWitness::verifies`] and [`Escape::verifies`].
//!
//! Not included: interval boxes with outward rounding for complex `c`, and
//! the interior tests of other hyperbolic components. Those can come later
//! as further `InWitness` arms. Until then, a point such as `c = -3/2` (in
//! `M`, outside both interior tests, no rational interval, no exact cycle)
//! is honestly `Undecided`: no finite certificate in this slice.

use std::cmp::Ordering;

use super::{Darkness, Membership};
use crate::{ConvergenceLoss, Imperfect, Loss};

/// An exact rational, normalized: `den > 0`, `gcd(num, den) = 1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rational {
    num: i128,
    den: i128,
}

fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

impl Rational {
    const ZERO: Rational = Rational { num: 0, den: 1 };
    const ONE: Rational = Rational { num: 1, den: 1 };

    /// `num / den`, normalized. `None` if `den` is zero or the value does
    /// not fit.
    pub fn new(num: i128, den: i128) -> Option<Self> {
        if den == 0 {
            return None;
        }
        let (num, den) = if den < 0 {
            (num.checked_neg()?, den.checked_neg()?)
        } else {
            (num, den)
        };
        // den > 0 here, and the gcd divides den, so it fits in i128.
        let g = gcd(num.unsigned_abs(), den as u128) as i128;
        Some(Rational {
            num: num / g,
            den: den / g,
        })
    }

    /// The exact value of a finite `f64` (every finite float is a dyadic
    /// rational). `None` if `x` is not finite or does not fit.
    pub fn from_f64(x: f64) -> Option<Self> {
        if !x.is_finite() {
            return None;
        }
        if x == 0.0 {
            return Some(Self::ZERO);
        }
        let bits = x.to_bits();
        let biased = ((bits >> 52) & 0x7ff) as i32;
        let fraction = bits & ((1u64 << 52) - 1);
        let (mut mantissa, mut exp) = if biased == 0 {
            (fraction, -1074)
        } else {
            (fraction | (1u64 << 52), biased - 1075)
        };
        let tz = mantissa.trailing_zeros();
        mantissa >>= tz;
        exp += tz as i32;
        let width = 64 - mantissa.leading_zeros() as i32;
        let (num, den) = if exp >= 0 {
            if width + exp > 126 {
                return None;
            }
            ((mantissa as i128) << exp, 1)
        } else {
            if -exp > 126 {
                return None;
            }
            (mantissa as i128, 1i128 << -exp)
        };
        let num = if x < 0.0 { -num } else { num };
        Rational::new(num, den)
    }

    /// The numerator.
    pub fn num(&self) -> i128 {
        self.num
    }

    /// The denominator, always positive.
    pub fn den(&self) -> i128 {
        self.den
    }

    fn is_zero(&self) -> bool {
        self.num == 0
    }

    fn add(self, o: Self) -> Option<Self> {
        let g = gcd(self.den as u128, o.den as u128) as i128;
        let left = self.num.checked_mul(o.den / g)?;
        let right = o.num.checked_mul(self.den / g)?;
        Rational::new(left.checked_add(right)?, (self.den / g).checked_mul(o.den)?)
    }

    fn neg(self) -> Option<Self> {
        Some(Rational {
            num: self.num.checked_neg()?,
            den: self.den,
        })
    }

    fn sub(self, o: Self) -> Option<Self> {
        self.add(o.neg()?)
    }

    fn mul(self, o: Self) -> Option<Self> {
        let g1 = gcd(self.num.unsigned_abs(), o.den as u128) as i128;
        let g2 = gcd(o.num.unsigned_abs(), self.den as u128) as i128;
        let num = (self.num / g1).checked_mul(o.num / g2)?;
        let den = (self.den / g2).checked_mul(o.den / g1)?;
        Rational::new(num, den)
    }

    /// Exact comparison; `None` only if the difference does not fit.
    fn compare(self, o: Self) -> Option<Ordering> {
        Some(self.sub(o)?.num.cmp(&0))
    }

    /// The exact square root, if it is rational.
    fn sqrt(self) -> Option<Self> {
        if self.num < 0 {
            return None;
        }
        let root = |n: i128| {
            let r = (n as u128).isqrt() as i128;
            (r * r == n).then_some(r)
        };
        Rational::new(root(self.num)?, root(self.den)?)
    }
}

/// An exact complex rational.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Complex {
    /// The real part.
    pub re: Rational,
    /// The imaginary part.
    pub im: Rational,
}

impl Complex {
    const ZERO: Complex = Complex {
        re: Rational::ZERO,
        im: Rational::ZERO,
    };

    /// `re + i·im`.
    pub fn new(re: Rational, im: Rational) -> Self {
        Complex { re, im }
    }

    /// One step of the orbit: `self² + c`, exactly.
    fn step(self, c: Complex) -> Option<Complex> {
        let re = self
            .re
            .mul(self.re)?
            .sub(self.im.mul(self.im)?)?
            .add(c.re)?;
        let two = Rational { num: 2, den: 1 };
        let im = two.mul(self.re)?.mul(self.im)?.add(c.im)?;
        Some(Complex { re, im })
    }

    /// `|self|² > 4`, exactly.
    fn escapes(self) -> Option<bool> {
        let norm = self.re.mul(self.re)?.add(self.im.mul(self.im)?)?;
        let four = Rational { num: 4, den: 1 };
        Some(norm.compare(four)? == Ordering::Greater)
    }
}

/// A certificate that `c ∈ M`.
#[derive(Clone, Debug, PartialEq)]
pub enum InWitness {
    /// `c` is in the closed main cardioid: `q(q + (x - ¼)) ≤ y²/4`, with
    /// `q = (x - ¼)² + y²`, exactly.
    Cardioid,
    /// `c` is in the closed period-2 bulb: `(x + 1)² + y² ≤ 1/16`, exactly.
    Bulb,
    /// `z_start, …, z_{start+period-1}` repeat exactly: `z_{start+period} = z_start`.
    Cycle {
        /// The first index of the cycle.
        start: usize,
        /// The cycle length.
        period: usize,
        /// The exact cycle, from `z_start`.
        cycle: Vec<Complex>,
    },
    /// For real `c`: `0 ∈ [lo, hi]` and `f([lo, hi]) ⊆ [lo, hi]`, exactly.
    Interval {
        /// The lower end.
        lo: Rational,
        /// The upper end.
        hi: Rational,
    },
}

impl InWitness {
    /// Re-check this certificate for `c` in exact arithmetic.
    pub fn verifies(&self, c: &Complex) -> bool {
        match self {
            InWitness::Cardioid => in_cardioid(*c).unwrap_or(false),
            InWitness::Bulb => in_bulb(*c).unwrap_or(false),
            InWitness::Cycle {
                start,
                period,
                cycle,
            } => cycle_holds(*c, *start, *period, cycle).unwrap_or(false),
            InWitness::Interval { lo, hi } => interval_holds(*c, *lo, *hi).unwrap_or(false),
        }
    }
}

fn at_most(a: Rational, b: Rational) -> Option<bool> {
    Some(a.compare(b)? != Ordering::Greater)
}

fn in_cardioid(c: Complex) -> Option<bool> {
    let quarter = Rational { num: 1, den: 4 };
    let a = c.re.sub(quarter)?;
    let y2 = c.im.mul(c.im)?;
    let q = a.mul(a)?.add(y2)?;
    at_most(q.mul(q.add(a)?)?, y2.mul(quarter)?)
}

fn in_bulb(c: Complex) -> Option<bool> {
    let b = c.re.add(Rational::ONE)?;
    let lhs = b.mul(b)?.add(c.im.mul(c.im)?)?;
    at_most(lhs, Rational { num: 1, den: 16 })
}

fn cycle_holds(c: Complex, start: usize, period: usize, cycle: &[Complex]) -> Option<bool> {
    if period == 0 || cycle.len() != period {
        return Some(false);
    }
    let mut z = Complex::ZERO;
    for k in 0..start + period {
        if k >= start && z != cycle[k - start] {
            return Some(false);
        }
        z = z.step(c)?;
    }
    Some(z == cycle[0])
}

fn interval_holds(c: Complex, lo: Rational, hi: Rational) -> Option<bool> {
    if !c.im.is_zero() {
        return Some(false);
    }
    let zero = Rational::ZERO;
    if lo.compare(zero)? == Ordering::Greater || hi.compare(zero)? == Ordering::Less {
        return Some(false);
    }
    // 0 ∈ [lo, hi], so x² over it is [0, max(lo², hi²)], and the image is
    // [c, max(lo², hi²) + c].
    let (lo2, hi2) = (lo.mul(lo)?, hi.mul(hi)?);
    let top = if lo2.compare(hi2)? == Ordering::Greater {
        lo2
    } else {
        hi2
    };
    Some(lo.compare(c.re)? != Ordering::Greater && top.add(c.re)?.compare(hi)? != Ordering::Greater)
}

/// A witness that `c ∉ M`: `z_n = z` and `|z|² > 4`, exactly.
#[derive(Clone, Debug, PartialEq)]
pub struct Escape {
    /// The escape time.
    pub n: usize,
    /// The exact `z_n`.
    pub z: Complex,
}

impl Escape {
    /// Re-check this witness for `c` in exact arithmetic.
    pub fn verifies(&self, c: &Complex) -> bool {
        let replay = || {
            let mut z = Complex::ZERO;
            for _ in 0..self.n {
                z = z.step(*c)?;
            }
            Some(z == self.z && z.escapes()?)
        };
        replay().unwrap_or(false)
    }
}

/// Why no certificate was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    /// The iteration budget ran out.
    Budget,
    /// Exact arithmetic outgrew `i128`.
    Precision,
}

/// Where the orbit was when the search stopped undecided.
#[derive(Clone, Debug, PartialEq)]
pub struct Open {
    /// Steps taken.
    pub iterations: usize,
    /// The last exact `z` reached.
    pub last: Complex,
    /// Why it stopped.
    pub stop: Stop,
}

/// Refused input: a non-finite coordinate. Addressed by the input's bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refused {
    re_bits: u64,
    im_bits: u64,
}

impl Refused {
    /// The refused input, as `f64` bit patterns `(re, im)`.
    pub fn input_bits(&self) -> (u64, u64) {
        (self.re_bits, self.im_bits)
    }
}

impl Darkness for Refused {
    type Address = (u64, u64);
    fn address(&self) -> (u64, u64) {
        self.input_bits()
    }
}

/// Decide membership of an exact `c` within `budget` steps.
pub fn membership(c: Complex, budget: usize) -> Membership<InWitness, Escape, Open> {
    for interior in [InWitness::Cardioid, InWitness::Bulb] {
        if interior.verifies(&c) {
            return Membership::In(interior);
        }
    }
    let mut orbit = vec![Complex::ZERO];
    let mut z = Complex::ZERO;
    let mut n = 0;
    let stop = loop {
        if n == budget {
            break Stop::Budget;
        }
        let Some(next) = z.step(c) else {
            break Stop::Precision;
        };
        match next.escapes() {
            None => break Stop::Precision,
            Some(true) => return Membership::Out(Escape { n: n + 1, z: next }),
            Some(false) => {}
        }
        n += 1;
        z = next;
        if let Some(start) = orbit.iter().position(|w| *w == z) {
            return Membership::In(InWitness::Cycle {
                start,
                period: n - start,
                cycle: orbit.split_off(start),
            });
        }
        orbit.push(z);
    };
    match real_interval(c) {
        Some(w) => Membership::In(w),
        None => Membership::Undecided(Open {
            iterations: n,
            last: z,
            stop,
        }),
    }
}

/// The invariant interval `[-β, β]`, `β = (1 + √(1 - 4c)) / 2`, when `c` is
/// real, the root is rational, and the interval checks exactly.
fn real_interval(c: Complex) -> Option<InWitness> {
    if !c.im.is_zero() {
        return None;
    }
    let four = Rational { num: 4, den: 1 };
    let root = Rational::ONE.sub(four.mul(c.re)?)?.sqrt()?;
    let half = Rational { num: 1, den: 2 };
    let beta = Rational::ONE.add(root)?.mul(half)?;
    let w = InWitness::Interval {
        lo: beta.neg()?,
        hi: beta,
    };
    w.verifies(&c).then_some(w)
}

/// Membership of `re + i·im`, mapped into [`Imperfect`]:
///
/// - `In`, `Out` → `Success`;
/// - `Undecided` → `Partial`, loss = steps taken;
/// - non-finite input → `Failure(Refused)`.
///
/// A finite input too large or too small for exact `i128` arithmetic is
/// `Undecided` with [`Stop::Precision`] after 0 steps: it is not nonsense.
pub fn of_f64(
    re: f64,
    im: f64,
    budget: usize,
) -> Imperfect<Membership<InWitness, Escape, Open>, Refused, ConvergenceLoss> {
    if !re.is_finite() || !im.is_finite() {
        return Imperfect::Failure(
            Refused {
                re_bits: re.to_bits(),
                im_bits: im.to_bits(),
            },
            ConvergenceLoss::zero(),
        );
    }
    let verdict = match (Rational::from_f64(re), Rational::from_f64(im)) {
        (Some(re), Some(im)) => membership(Complex::new(re, im), budget),
        _ => Membership::Undecided(Open {
            iterations: 0,
            last: Complex::ZERO,
            stop: Stop::Precision,
        }),
    };
    verdict.into_imperfect(|open| ConvergenceLoss::new(open.iterations))
}
