//! I wanna thank Brené Brown for her work.
//!
//!
//! Result extended with partial success. Three states:
//!
//! - **Success** — the transformation preserved everything. Zero loss.
//! - **Partial** — a value came through, but something was lost getting here.
//!   The loss is measured and carried forward.
//! - **Failure** — no value survived.
//!
//! The middle state is the point. Most real transformations are not perfect
//! and not failed. They are partial: a value exists, and it cost something.
//! Collapsing that into `Ok` or `Err` destroys the information about what
//! was lost.
//!
//! [`Loss`] is the trait that measures what didn't survive. [`ShannonLoss`]
//! is the default implementation, measuring information loss in bits using
//! Shannon's base-2 logarithmic measure.

// ---------------------------------------------------------------------------
// ApertureLoss — which dimensions were dark at observation time
// ---------------------------------------------------------------------------

/// Loss that tracks which feature dimensions were dark (unobservable) at
/// the time of observation.
///
/// `dark_dims` — indices of the dimensions that were not illuminated.
/// `total_dims` — total number of dimensions in the space.
///
/// Zero loss means all dimensions were illuminated (no dark dims).
/// Total loss means all dimensions were dark.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApertureLoss {
    dark_dims: Vec<usize>,
    total_dims: usize,
}

impl ApertureLoss {
    /// Create a new ApertureLoss.
    ///
    /// `dark_dims` — indices of dimensions that were dark at observation.
    /// `total_dims` — total number of dimensions in the feature space.
    pub fn new(dark_dims: Vec<usize>, total_dims: usize) -> Self {
        ApertureLoss { dark_dims, total_dims }
    }

    /// The dark dimension indices.
    pub fn dark_dims(&self) -> &[usize] {
        &self.dark_dims
    }

    /// Total number of dimensions in the space.
    pub fn total_dims(&self) -> usize {
        self.total_dims
    }

    /// Number of illuminated (active) dimensions.
    pub fn active_count(&self) -> usize {
        self.total_dims.saturating_sub(self.dark_dims.len())
    }
}

impl Default for ApertureLoss {
    fn default() -> Self {
        Self::zero()
    }
}

impl Loss for ApertureLoss {
    fn zero() -> Self {
        ApertureLoss { dark_dims: Vec::new(), total_dims: 0 }
    }

    fn total() -> Self {
        // Sentinel: total_dims = usize::MAX, dark_dims empty (all dims dark conceptually)
        ApertureLoss { dark_dims: Vec::new(), total_dims: usize::MAX }
    }

    fn is_zero(&self) -> bool {
        self.dark_dims.is_empty() && self.total_dims != usize::MAX
    }

    fn combine(mut self, other: Self) -> Self {
        // Union of dark dims; take the larger total_dims
        for d in other.dark_dims {
            if !self.dark_dims.contains(&d) {
                self.dark_dims.push(d);
            }
        }
        self.total_dims = self.total_dims.max(other.total_dims);
        self
    }
}

// ---------------------------------------------------------------------------

/// A measure of what didn't survive a transformation.
///
/// Loss forms a monoid: `zero()` is the identity element, `combine` is
/// associative, and `total()` is the absorbing element (annihilator).
pub trait Loss: Clone + Default {
    /// The identity: no loss occurred. `combine(zero(), x) == x`.
    fn zero() -> Self;

    /// Total loss: the transformation destroyed everything.
    /// Acts as an absorbing element under `combine`.
    fn total() -> Self;

    /// Whether this loss is zero (lossless).
    fn is_zero(&self) -> bool;

    /// Accumulate two losses. Associative: `a.combine(b).combine(c) == a.combine(b.combine(c))`.
    fn combine(self, other: Self) -> Self;
}

/// Information loss measured in bits (Shannon entropy, base-2 logarithm).
///
/// Bits are the natural unit because every transformation is a channel,
/// and channels lose information in bits. The default [`Loss`] implementation.
#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub struct ShannonLoss(f64);

impl ShannonLoss {
    pub fn new(bits: f64) -> Self {
        debug_assert!(bits >= 0.0, "loss must be non-negative");
        ShannonLoss(bits)
    }

    pub fn as_f64(&self) -> f64 {
        self.0
    }

    pub fn is_lossless(&self) -> bool {
        self.is_zero()
    }
}

impl Default for ShannonLoss {
    fn default() -> Self {
        Self::zero()
    }
}

impl Loss for ShannonLoss {
    fn zero() -> Self {
        ShannonLoss(0.0)
    }

    fn total() -> Self {
        ShannonLoss(f64::INFINITY)
    }

    fn is_zero(&self) -> bool {
        self.0 == 0.0
    }

    fn combine(self, other: Self) -> Self {
        ShannonLoss(self.0 + other.0)
    }
}

impl std::ops::Add for ShannonLoss {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        ShannonLoss(self.0 + rhs.0)
    }
}

impl std::ops::AddAssign for ShannonLoss {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl std::fmt::Display for ShannonLoss {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.6} bits", self.0)
    }
}

impl From<f64> for ShannonLoss {
    fn from(v: f64) -> Self {
        ShannonLoss(v)
    }
}

/// Result extended with partial success.
///
/// Three states:
/// - `Success(T)` — perfect result, zero loss.
/// - `Partial(T, L)` — value present, some information lost getting here.
/// - `Failure(E)` — failure, no value.
///
/// The design descends from PbtA (Powered by the Apocalypse) tabletop games,
/// which use three outcome tiers: 10+ is full success, 7-9 is success with
/// complications, 6- is failure. The middle tier — success with cost — is the
/// design innovation that PbtA contributed to game design. This crate encodes
/// that structure in types.
///
/// Follows `Result` conventions: `is_ok()` means "has a value" (Success or Partial).
/// The `.ok()` and `.err()` extractor methods follow `Result` naming conventions.
#[derive(Clone, Debug, PartialEq)]
pub enum Imperfect<T, E, L: Loss = ShannonLoss> {
    Success(T),
    Partial(T, L),
    Failure(E),
}

impl<T, E, L: Loss> Imperfect<T, E, L> {
    pub fn is_ok(&self) -> bool {
        !self.is_err()
    }

    pub fn is_partial(&self) -> bool {
        matches!(self, Imperfect::Partial(_, _))
    }

    pub fn is_err(&self) -> bool {
        matches!(self, Imperfect::Failure(_))
    }

    pub fn ok(self) -> Option<T> {
        match self {
            Imperfect::Success(v) | Imperfect::Partial(v, _) => Some(v),
            Imperfect::Failure(_) => None,
        }
    }

    pub fn err(self) -> Option<E> {
        match self {
            Imperfect::Failure(e) => Some(e),
            _ => None,
        }
    }

    pub fn loss(&self) -> L {
        match self {
            Imperfect::Success(_) => L::zero(),
            Imperfect::Partial(_, l) => l.clone(),
            Imperfect::Failure(_) => L::total(),
        }
    }

    pub fn as_ref(&self) -> Imperfect<&T, &E, L> {
        match self {
            Imperfect::Success(t) => Imperfect::Success(t),
            Imperfect::Partial(t, l) => Imperfect::Partial(t, l.clone()),
            Imperfect::Failure(e) => Imperfect::Failure(e),
        }
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Imperfect<U, E, L> {
        match self {
            Imperfect::Success(t) => Imperfect::Success(f(t)),
            Imperfect::Partial(t, l) => Imperfect::Partial(f(t), l),
            Imperfect::Failure(e) => Imperfect::Failure(e),
        }
    }

    pub fn map_err<F>(self, f: impl FnOnce(E) -> F) -> Imperfect<T, F, L> {
        match self {
            Imperfect::Success(t) => Imperfect::Success(t),
            Imperfect::Partial(t, l) => Imperfect::Partial(t, l),
            Imperfect::Failure(e) => Imperfect::Failure(f(e)),
        }
    }

    /// Propagate accumulated loss from `self` through `next`.
    ///
    /// - Success + next → next (no loss to propagate)
    /// - Partial(_, loss) + Success(v) → Partial(v, loss)
    /// - Partial(_, loss1) + Partial(v, loss2) → Partial(v, loss1.combine(loss2))
    /// - Partial(_, _) + Failure(e) → Failure(e)
    /// - Failure + anything → panics (programming error)
    pub fn compose<T2, E2>(self, next: Imperfect<T2, E2, L>) -> Imperfect<T2, E2, L> {
        match self {
            Imperfect::Failure(_) => panic!("compose called on Failure — check is_ok() first"),
            Imperfect::Success(_) => next,
            Imperfect::Partial(_, loss) => match next {
                Imperfect::Success(v) => Imperfect::Partial(v, loss),
                Imperfect::Partial(v, loss2) => Imperfect::Partial(v, loss.combine(loss2)),
                Imperfect::Failure(e) => Imperfect::Failure(e),
            },
        }
    }
}

// --- std interop ---

impl<T, E, L: Loss> From<Result<T, E>> for Imperfect<T, E, L> {
    fn from(r: Result<T, E>) -> Self {
        match r {
            Ok(v) => Imperfect::Success(v),
            Err(e) => Imperfect::Failure(e),
        }
    }
}

impl<T, E, L: Loss> From<Imperfect<T, E, L>> for Result<T, E> {
    fn from(i: Imperfect<T, E, L>) -> Self {
        match i {
            Imperfect::Success(v) | Imperfect::Partial(v, _) => Ok(v),
            Imperfect::Failure(e) => Err(e),
        }
    }
}

/// `None` maps to `Failure(())` because absence is total loss — there is no
/// value and no meaningful error to report. `Some(v)` maps to `Success(v)`.
impl<T, L: Loss> From<Option<T>> for Imperfect<T, (), L> {
    fn from(o: Option<T>) -> Self {
        match o {
            Some(v) => Imperfect::Success(v),
            None => Imperfect::Failure(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn double_u32(v: u32) -> u32 {
        v * 2
    }

    #[test]
    fn shannon_zero() {
        let l = ShannonLoss::zero();
        assert!(l.is_zero());
        assert_eq!(l.as_f64(), 0.0);
    }

    #[test]
    fn shannon_total() {
        let l = ShannonLoss::total();
        assert!(!l.is_zero());
        assert!(l.as_f64().is_infinite());
    }

    #[test]
    fn shannon_new() {
        let l = ShannonLoss::new(1.5);
        assert_eq!(l.as_f64(), 1.5);
        assert!(!l.is_zero());
    }

    #[test]
    fn shannon_combine() {
        let a = ShannonLoss::new(1.0);
        let b = ShannonLoss::new(2.5);
        let c = a.combine(b);
        assert_eq!(c.as_f64(), 3.5);
    }

    #[test]
    fn shannon_combine_zero_is_identity() {
        let a = ShannonLoss::new(3.0);
        let b = a.clone().combine(ShannonLoss::zero());
        assert_eq!(a, b);
    }

    #[test]
    fn shannon_default_is_zero() {
        let l = ShannonLoss::default();
        assert!(l.is_zero());
    }

    #[test]
    fn shannon_display() {
        let l = ShannonLoss::new(2.0);
        assert_eq!(format!("{}", l), "2.000000 bits");
    }

    #[test]
    fn shannon_from_f64() {
        let l: ShannonLoss = 3.14.into();
        assert_eq!(l.as_f64(), 3.14);
    }

    #[test]
    fn shannon_add_operator() {
        let a = ShannonLoss::new(1.0);
        let b = ShannonLoss::new(2.5);
        let c = a + b;
        assert_eq!(c.as_f64(), 3.5);
    }

    #[test]
    fn shannon_add_assign() {
        let mut a = ShannonLoss::new(1.0);
        a += ShannonLoss::new(0.5);
        assert_eq!(a.as_f64(), 1.5);
    }

    #[test]
    fn shannon_ordering() {
        let a = ShannonLoss::new(1.0);
        let b = ShannonLoss::new(2.0);
        assert!(a < b);
    }

    #[test]
    fn shannon_is_lossless() {
        assert!(ShannonLoss::zero().is_lossless());
        assert!(!ShannonLoss::new(0.1).is_lossless());
    }

    #[test]
    #[should_panic]
    #[cfg(debug_assertions)]
    fn shannon_negative_panics_debug() {
        ShannonLoss::new(-1.0);
    }

    #[test]
    fn total_is_absorbing() {
        let t = ShannonLoss::total();
        let x = ShannonLoss::new(5.0);
        assert!(t.clone().combine(x.clone()).as_f64().is_infinite());
        assert!(x.combine(t).as_f64().is_infinite());
    }

    // --- Imperfect ---

    #[test]
    fn ok_is_ok() {
        let i: Imperfect<u32, String> = Imperfect::Success(42);
        assert!(i.is_ok());
        assert!(!i.is_partial());
        assert!(!i.is_err());
    }

    #[test]
    fn partial_is_partial() {
        let i: Imperfect<u32, String> = Imperfect::Partial(42, ShannonLoss::new(1.5));
        assert!(i.is_ok());
        assert!(i.is_partial());
        assert!(!i.is_err());
    }

    #[test]
    fn err_is_err() {
        let i: Imperfect<u32, String> = Imperfect::Failure("oops".into());
        assert!(!i.is_ok());
        assert!(!i.is_partial());
        assert!(i.is_err());
    }

    #[test]
    fn ok_returns_value() {
        let i: Imperfect<u32, String> = Imperfect::Success(42);
        assert_eq!(i.ok(), Some(42));
    }

    #[test]
    fn partial_ok_returns_value() {
        let i: Imperfect<u32, String> = Imperfect::Partial(42, ShannonLoss::new(1.0));
        assert_eq!(i.ok(), Some(42));
    }

    #[test]
    fn err_ok_returns_none() {
        let i: Imperfect<u32, String> = Imperfect::Failure("oops".into());
        assert_eq!(i.ok(), None);
    }

    #[test]
    fn err_returns_error() {
        let i: Imperfect<u32, String> = Imperfect::Failure("oops".into());
        assert_eq!(i.err(), Some("oops".into()));
    }

    #[test]
    fn ok_err_returns_none() {
        let i: Imperfect<u32, String> = Imperfect::Success(42);
        assert_eq!(i.err(), None);
    }

    #[test]
    fn loss_ok_is_zero() {
        let i: Imperfect<u32, String> = Imperfect::Success(42);
        assert!(i.loss().is_zero());
    }

    #[test]
    fn loss_partial() {
        let i: Imperfect<u32, String> = Imperfect::Partial(42, ShannonLoss::new(1.5));
        assert_eq!(i.loss().as_f64(), 1.5);
    }

    #[test]
    fn loss_err_is_total() {
        let i: Imperfect<u32, String> = Imperfect::Failure("oops".into());
        assert!(i.loss().as_f64().is_infinite());
    }

    #[test]
    fn as_ref_ok() {
        let i: Imperfect<u32, String> = Imperfect::Success(42);
        let r = i.as_ref();
        assert_eq!(r.ok(), Some(&42));
    }

    #[test]
    fn as_ref_partial() {
        let i: Imperfect<u32, String> = Imperfect::Partial(42, ShannonLoss::new(1.0));
        let r = i.as_ref();
        assert!(r.is_partial());
        assert_eq!(r.ok(), Some(&42));
    }

    #[test]
    fn as_ref_err() {
        let i: Imperfect<u32, String> = Imperfect::Failure("oops".into());
        let r = i.as_ref();
        assert_eq!(r.err(), Some(&"oops".to_string()));
    }

    #[test]
    fn map_ok() {
        let i: Imperfect<u32, String> = Imperfect::Success(42);
        let m = i.map(double_u32);
        assert_eq!(m.ok(), Some(84));
    }

    #[test]
    fn map_partial_preserves_loss() {
        let i: Imperfect<u32, String> = Imperfect::Partial(42, ShannonLoss::new(1.0));
        let m = i.map(double_u32);
        assert!(m.is_partial());
        assert_eq!(m.ok(), Some(84));
    }

    #[test]
    fn map_err_is_noop() {
        let i: Imperfect<u32, String> = Imperfect::Failure("oops".into());
        let m = i.map(double_u32);
        assert!(m.is_err());
    }

    #[test]
    fn map_err_transforms_error() {
        let i: Imperfect<u32, String> = Imperfect::Failure("oops".into());
        let m = i.map_err(|e| e.len());
        assert_eq!(m.err(), Some(4));
    }

    #[test]
    fn map_err_success_passes_through() {
        let i: Imperfect<u32, String> = Imperfect::Success(7);
        let m = i.map_err(|_e| 99usize);
        assert_eq!(m.ok(), Some(7));
    }

    #[test]
    fn map_err_partial_passes_through() {
        let i: Imperfect<u32, String> = Imperfect::Partial(42, ShannonLoss::new(1.5));
        let m = i.map_err(|_e| 99usize);
        assert!(m.is_partial());
        assert_eq!(m.loss().as_f64(), 1.5);
        assert_eq!(m.ok(), Some(42));
    }

    // --- PartialEq ---

    #[test]
    fn partial_eq_success_equal() {
        let a: Imperfect<u32, String> = Imperfect::Success(1);
        let b: Imperfect<u32, String> = Imperfect::Success(1);
        assert_eq!(a, b);
    }

    #[test]
    fn partial_eq_success_not_equal() {
        let a: Imperfect<u32, String> = Imperfect::Success(1);
        let b: Imperfect<u32, String> = Imperfect::Success(2);
        assert_ne!(a, b);
    }

    #[test]
    fn partial_eq_partial_equal() {
        let a: Imperfect<u32, String> = Imperfect::Partial(5, ShannonLoss::new(1.0));
        let b: Imperfect<u32, String> = Imperfect::Partial(5, ShannonLoss::new(1.0));
        assert_eq!(a, b);
    }

    #[test]
    fn partial_eq_partial_not_equal_value() {
        let a: Imperfect<u32, String> = Imperfect::Partial(5, ShannonLoss::new(1.0));
        let b: Imperfect<u32, String> = Imperfect::Partial(6, ShannonLoss::new(1.0));
        assert_ne!(a, b);
    }

    #[test]
    fn partial_eq_failure_equal() {
        let a: Imperfect<u32, String> = Imperfect::Failure("err".into());
        let b: Imperfect<u32, String> = Imperfect::Failure("err".into());
        assert_eq!(a, b);
    }

    #[test]
    fn partial_eq_different_variants_not_equal() {
        let a: Imperfect<u32, String> = Imperfect::Success(1);
        let b: Imperfect<u32, String> = Imperfect::Failure("err".into());
        assert_ne!(a, b);
    }

    // --- compose ---

    #[test]
    fn compose_ok_ok() {
        let a: Imperfect<u32, String> = Imperfect::Success(1);
        let b: Imperfect<&str, String> = Imperfect::Success("hi");
        let c = a.compose(b);
        assert!(matches!(c, Imperfect::Success("hi")));
    }

    #[test]
    fn compose_ok_partial() {
        let a: Imperfect<u32, String> = Imperfect::Success(1);
        let b: Imperfect<u32, String> = Imperfect::Partial(2, ShannonLoss::new(1.0));
        let c = a.compose(b);
        assert!(c.is_partial());
        assert_eq!(c.loss().as_f64(), 1.0);
        assert_eq!(c.ok(), Some(2));
    }

    #[test]
    fn compose_ok_err() {
        let a: Imperfect<u32, String> = Imperfect::Success(1);
        let b: Imperfect<u32, String> = Imperfect::Failure("fail".into());
        let c = a.compose(b);
        assert!(c.is_err());
    }

    #[test]
    fn compose_partial_ok_carries_loss() {
        let a: Imperfect<u32, String> = Imperfect::Partial(1, ShannonLoss::new(1.0));
        let b: Imperfect<u32, String> = Imperfect::Success(2);
        let c = a.compose(b);
        assert!(c.is_partial());
        assert_eq!(c.loss().as_f64(), 1.0);
        assert_eq!(c.ok(), Some(2));
    }

    #[test]
    fn compose_partial_partial_accumulates() {
        let a: Imperfect<u32, String> = Imperfect::Partial(1, ShannonLoss::new(1.0));
        let b: Imperfect<u32, String> = Imperfect::Partial(2, ShannonLoss::new(0.5));
        let c = a.compose(b);
        assert!(c.is_partial());
        assert_eq!(c.loss().as_f64(), 1.5);
        assert_eq!(c.ok(), Some(2));
    }

    #[test]
    fn compose_partial_err() {
        let a: Imperfect<u32, String> = Imperfect::Partial(1, ShannonLoss::new(1.0));
        let b: Imperfect<u32, String> = Imperfect::Failure("fail".into());
        let c = a.compose(b);
        assert!(c.is_err());
    }

    #[test]
    #[should_panic(expected = "compose called on Failure")]
    fn compose_err_panics() {
        let a: Imperfect<u32, String> = Imperfect::Failure("fail".into());
        let b: Imperfect<u32, String> = Imperfect::Success(2);
        let _ = a.compose(b);
    }

    // --- std interop ---

    #[test]
    fn from_result_ok() {
        let r: Result<u32, String> = Ok(42);
        let i: Imperfect<u32, String> = r.into();
        assert_eq!(i.ok(), Some(42));
    }

    #[test]
    fn from_result_err() {
        let r: Result<u32, String> = Err("oops".into());
        let i: Imperfect<u32, String> = r.into();
        assert!(i.is_err());
    }

    #[test]
    fn into_result_ok() {
        let i: Imperfect<u32, String> = Imperfect::Success(42);
        let r: Result<u32, String> = i.into();
        assert_eq!(r, Ok(42));
    }

    #[test]
    fn into_result_partial_keeps_value() {
        let i: Imperfect<u32, String> = Imperfect::Partial(42, ShannonLoss::new(1.0));
        let r: Result<u32, String> = i.into();
        assert_eq!(r, Ok(42));
    }

    #[test]
    fn into_result_err() {
        let i: Imperfect<u32, String> = Imperfect::Failure("oops".into());
        let r: Result<u32, String> = i.into();
        assert_eq!(r, Err("oops".into()));
    }

    #[test]
    fn from_option_some() {
        let o: Option<u32> = Some(42);
        let i: Imperfect<u32, ()> = o.into();
        assert_eq!(i.ok(), Some(42));
    }

    #[test]
    fn from_option_none() {
        let o: Option<u32> = None;
        let i: Imperfect<u32, ()> = o.into();
        assert!(i.is_err());
    }
}
