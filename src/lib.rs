#![deny(missing_docs)]

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
//! [`Loss`] is the trait that measures what didn't survive. Each domain
//! carries its own loss type: [`ConvergenceLoss`] for iterative refinement,
//! [`ApertureLoss`] for partial observation, [`RoutingLoss`] for decision
//! uncertainty.

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
pub enum Imperfect<T, E, L: Loss> {
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

// --- Domain-specific loss types ---

/// Distance to crystal. Zero means crystallized. Combine takes the max
/// (the furthest from crystal dominates).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConvergenceLoss(usize);

impl ConvergenceLoss {
    pub fn new(steps: usize) -> Self {
        ConvergenceLoss(steps)
    }

    pub fn steps(&self) -> usize {
        self.0
    }
}

impl Default for ConvergenceLoss {
    fn default() -> Self {
        Self::zero()
    }
}

impl Loss for ConvergenceLoss {
    fn zero() -> Self {
        ConvergenceLoss(0)
    }

    fn total() -> Self {
        ConvergenceLoss(usize::MAX)
    }

    fn is_zero(&self) -> bool {
        self.0 == 0
    }

    fn combine(self, other: Self) -> Self {
        ConvergenceLoss(self.0.max(other.0))
    }
}

impl std::fmt::Display for ConvergenceLoss {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} steps from crystal", self.0)
    }
}

/// Which dimensions were dark during observation. Zero means all observed.
/// Combine takes the union of dark dims. Total is represented by aperture = 1.0.
#[derive(Clone, Debug, PartialEq)]
pub struct ApertureLoss {
    dark_dims: Vec<usize>,
    aperture: f64,
}

impl ApertureLoss {
    /// Create a new ApertureLoss from dark dimensions and total dimension count.
    /// `aperture` is the fraction of dimensions that were dark (0.0 to 1.0).
    pub fn new(dark_dims: Vec<usize>, total_dims: usize) -> Self {
        let aperture = if total_dims == 0 {
            0.0
        } else {
            dark_dims.len() as f64 / total_dims as f64
        };
        ApertureLoss { dark_dims, aperture }
    }

    pub fn dark_dims(&self) -> &[usize] {
        &self.dark_dims
    }

    pub fn aperture(&self) -> f64 {
        self.aperture
    }
}

impl Default for ApertureLoss {
    fn default() -> Self {
        Self::zero()
    }
}

impl Loss for ApertureLoss {
    fn zero() -> Self {
        ApertureLoss {
            dark_dims: vec![],
            aperture: 0.0,
        }
    }

    fn total() -> Self {
        ApertureLoss {
            dark_dims: vec![],
            aperture: 1.0,
        }
    }

    fn is_zero(&self) -> bool {
        self.dark_dims.is_empty() && self.aperture == 0.0
    }

    fn combine(self, other: Self) -> Self {
        let mut dims = self.dark_dims;
        for d in other.dark_dims {
            if !dims.contains(&d) {
                dims.push(d);
            }
        }
        dims.sort();
        let aperture = self.aperture.max(other.aperture);
        ApertureLoss { dark_dims: dims, aperture }
    }
}

impl std::fmt::Display for ApertureLoss {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.1}% dark (dims: {:?})", self.aperture * 100.0, self.dark_dims)
    }
}

/// Decision uncertainty at a routing point. Zero means one model at 100%.
/// Combine takes max entropy (most uncertain dominates).
#[derive(Clone, Debug, PartialEq)]
pub struct RoutingLoss {
    entropy: f64,
    runner_up_gap: f64,
}

impl RoutingLoss {
    /// Create a new RoutingLoss.
    /// - `entropy`: Shannon entropy of the routing distribution (bits).
    /// - `runner_up_gap`: probability gap between top pick and runner-up (0.0 to 1.0).
    pub fn new(entropy: f64, runner_up_gap: f64) -> Self {
        debug_assert!(entropy >= 0.0, "entropy must be non-negative");
        debug_assert!(
            (0.0..=1.0).contains(&runner_up_gap),
            "runner_up_gap must be in [0.0, 1.0]"
        );
        RoutingLoss { entropy, runner_up_gap }
    }

    pub fn entropy(&self) -> f64 {
        self.entropy
    }

    pub fn runner_up_gap(&self) -> f64 {
        self.runner_up_gap
    }
}

impl Default for RoutingLoss {
    fn default() -> Self {
        Self::zero()
    }
}

impl Loss for RoutingLoss {
    fn zero() -> Self {
        RoutingLoss {
            entropy: 0.0,
            runner_up_gap: 1.0,
        }
    }

    fn total() -> Self {
        RoutingLoss {
            entropy: f64::INFINITY,
            runner_up_gap: 0.0,
        }
    }

    fn is_zero(&self) -> bool {
        self.entropy == 0.0
    }

    fn combine(self, other: Self) -> Self {
        if self.entropy >= other.entropy {
            RoutingLoss {
                entropy: self.entropy,
                runner_up_gap: self.runner_up_gap.min(other.runner_up_gap),
            }
        } else {
            RoutingLoss {
                entropy: other.entropy,
                runner_up_gap: self.runner_up_gap.min(other.runner_up_gap),
            }
        }
    }
}

impl std::fmt::Display for RoutingLoss {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:.4} bits entropy, {:.1}% gap",
            self.entropy,
            self.runner_up_gap * 100.0
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn double_u32(v: u32) -> u32 {
        v * 2
    }

    // --- Imperfect with ConvergenceLoss ---

    #[test]
    fn ok_is_ok() {
        let i: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Success(42);
        assert!(i.is_ok());
        assert!(!i.is_partial());
        assert!(!i.is_err());
    }

    #[test]
    fn partial_is_partial() {
        let i: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Partial(42, ConvergenceLoss::new(3));
        assert!(i.is_ok());
        assert!(i.is_partial());
        assert!(!i.is_err());
    }

    #[test]
    fn err_is_err() {
        let i: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Failure("oops".into());
        assert!(!i.is_ok());
        assert!(!i.is_partial());
        assert!(i.is_err());
    }

    #[test]
    fn ok_returns_value() {
        let i: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Success(42);
        assert_eq!(i.ok(), Some(42));
    }

    #[test]
    fn partial_ok_returns_value() {
        let i: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Partial(42, ConvergenceLoss::new(1));
        assert_eq!(i.ok(), Some(42));
    }

    #[test]
    fn err_ok_returns_none() {
        let i: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Failure("oops".into());
        assert_eq!(i.ok(), None);
    }

    #[test]
    fn err_returns_error() {
        let i: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Failure("oops".into());
        assert_eq!(i.err(), Some("oops".into()));
    }

    #[test]
    fn ok_err_returns_none() {
        let i: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Success(42);
        assert_eq!(i.err(), None);
    }

    #[test]
    fn loss_ok_is_zero() {
        let i: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Success(42);
        assert!(i.loss().is_zero());
    }

    #[test]
    fn loss_partial() {
        let i: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Partial(42, ConvergenceLoss::new(3));
        assert_eq!(i.loss().steps(), 3);
    }

    #[test]
    fn loss_err_is_total() {
        let i: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Failure("oops".into());
        assert_eq!(i.loss().steps(), usize::MAX);
    }

    #[test]
    fn as_ref_ok() {
        let i: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Success(42);
        let r = i.as_ref();
        assert_eq!(r.ok(), Some(&42));
    }

    #[test]
    fn as_ref_partial() {
        let i: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Partial(42, ConvergenceLoss::new(1));
        let r = i.as_ref();
        assert!(r.is_partial());
        assert_eq!(r.ok(), Some(&42));
    }

    #[test]
    fn as_ref_err() {
        let i: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Failure("oops".into());
        let r = i.as_ref();
        assert_eq!(r.err(), Some(&"oops".to_string()));
    }

    #[test]
    fn map_ok() {
        let i: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Success(42);
        let m = i.map(double_u32);
        assert_eq!(m.ok(), Some(84));
    }

    #[test]
    fn map_partial_preserves_loss() {
        let i: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Partial(42, ConvergenceLoss::new(2));
        let m = i.map(double_u32);
        assert!(m.is_partial());
        assert_eq!(m.ok(), Some(84));
    }

    #[test]
    fn map_err_is_noop() {
        let i: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Failure("oops".into());
        let m = i.map(double_u32);
        assert!(m.is_err());
    }

    #[test]
    fn map_err_transforms_error() {
        let i: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Failure("oops".into());
        let m = i.map_err(|e| e.len());
        assert_eq!(m.err(), Some(4));
    }

    #[test]
    fn map_err_success_passes_through() {
        let i: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Success(7);
        let m = i.map_err(|_e| 99usize);
        assert_eq!(m.ok(), Some(7));
    }

    #[test]
    fn map_err_partial_passes_through() {
        let i: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Partial(42, ConvergenceLoss::new(3));
        let m = i.map_err(|_e| 99usize);
        assert!(m.is_partial());
        assert_eq!(m.loss().steps(), 3);
        assert_eq!(m.ok(), Some(42));
    }

    // --- PartialEq ---

    #[test]
    fn partial_eq_success_equal() {
        let a: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Success(1);
        let b: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Success(1);
        assert_eq!(a, b);
    }

    #[test]
    fn partial_eq_success_not_equal() {
        let a: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Success(1);
        let b: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Success(2);
        assert_ne!(a, b);
    }

    #[test]
    fn partial_eq_partial_equal() {
        let a: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Partial(5, ConvergenceLoss::new(1));
        let b: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Partial(5, ConvergenceLoss::new(1));
        assert_eq!(a, b);
    }

    #[test]
    fn partial_eq_partial_not_equal_value() {
        let a: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Partial(5, ConvergenceLoss::new(1));
        let b: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Partial(6, ConvergenceLoss::new(1));
        assert_ne!(a, b);
    }

    #[test]
    fn partial_eq_failure_equal() {
        let a: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Failure("err".into());
        let b: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Failure("err".into());
        assert_eq!(a, b);
    }

    #[test]
    fn partial_eq_different_variants_not_equal() {
        let a: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Success(1);
        let b: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Failure("err".into());
        assert_ne!(a, b);
    }

    // --- compose ---

    #[test]
    fn compose_ok_ok() {
        let a: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Success(1);
        let b: Imperfect<&str, String, ConvergenceLoss> = Imperfect::Success("hi");
        let c = a.compose(b);
        assert!(matches!(c, Imperfect::Success("hi")));
    }

    #[test]
    fn compose_ok_partial() {
        let a: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Success(1);
        let b: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Partial(2, ConvergenceLoss::new(3));
        let c = a.compose(b);
        assert!(c.is_partial());
        assert_eq!(c.loss().steps(), 3);
        assert_eq!(c.ok(), Some(2));
    }

    #[test]
    fn compose_ok_err() {
        let a: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Success(1);
        let b: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Failure("fail".into());
        let c = a.compose(b);
        assert!(c.is_err());
    }

    #[test]
    fn compose_partial_ok_carries_loss() {
        let a: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Partial(1, ConvergenceLoss::new(3));
        let b: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Success(2);
        let c = a.compose(b);
        assert!(c.is_partial());
        assert_eq!(c.loss().steps(), 3);
        assert_eq!(c.ok(), Some(2));
    }

    #[test]
    fn compose_partial_partial_accumulates() {
        let a: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Partial(1, ConvergenceLoss::new(3));
        let b: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Partial(2, ConvergenceLoss::new(5));
        let c = a.compose(b);
        assert!(c.is_partial());
        // ConvergenceLoss::combine takes max
        assert_eq!(c.loss().steps(), 5);
        assert_eq!(c.ok(), Some(2));
    }

    #[test]
    fn compose_partial_err() {
        let a: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Partial(1, ConvergenceLoss::new(3));
        let b: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Failure("fail".into());
        let c = a.compose(b);
        assert!(c.is_err());
    }

    #[test]
    #[should_panic(expected = "compose called on Failure")]
    fn compose_err_panics() {
        let a: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Failure("fail".into());
        let b: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Success(2);
        let _ = a.compose(b);
    }

    // --- std interop ---

    #[test]
    fn from_result_ok() {
        let r: Result<u32, String> = Ok(42);
        let i: Imperfect<u32, String, ConvergenceLoss> = r.into();
        assert_eq!(i.ok(), Some(42));
    }

    #[test]
    fn from_result_err() {
        let r: Result<u32, String> = Err("oops".into());
        let i: Imperfect<u32, String, ConvergenceLoss> = r.into();
        assert!(i.is_err());
    }

    #[test]
    fn into_result_ok() {
        let i: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Success(42);
        let r: Result<u32, String> = i.into();
        assert_eq!(r, Ok(42));
    }

    #[test]
    fn into_result_partial_keeps_value() {
        let i: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Partial(42, ConvergenceLoss::new(1));
        let r: Result<u32, String> = i.into();
        assert_eq!(r, Ok(42));
    }

    #[test]
    fn into_result_err() {
        let i: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Failure("oops".into());
        let r: Result<u32, String> = i.into();
        assert_eq!(r, Err("oops".into()));
    }

    #[test]
    fn from_option_some() {
        let o: Option<u32> = Some(42);
        let i: Imperfect<u32, (), ConvergenceLoss> = o.into();
        assert_eq!(i.ok(), Some(42));
    }

    #[test]
    fn from_option_none() {
        let o: Option<u32> = None;
        let i: Imperfect<u32, (), ConvergenceLoss> = o.into();
        assert!(i.is_err());
    }

    // --- ConvergenceLoss ---

    #[test]
    fn convergence_zero() {
        let l = ConvergenceLoss::zero();
        assert!(l.is_zero());
        assert_eq!(l.steps(), 0);
    }

    #[test]
    fn convergence_total() {
        let l = ConvergenceLoss::total();
        assert!(!l.is_zero());
        assert_eq!(l.steps(), usize::MAX);
    }

    #[test]
    fn convergence_new() {
        let l = ConvergenceLoss::new(5);
        assert_eq!(l.steps(), 5);
        assert!(!l.is_zero());
    }

    #[test]
    fn convergence_combine_takes_max() {
        let a = ConvergenceLoss::new(3);
        let b = ConvergenceLoss::new(7);
        let c = a.combine(b);
        assert_eq!(c.steps(), 7);
    }

    #[test]
    fn convergence_combine_zero_is_identity() {
        let a = ConvergenceLoss::new(5);
        let b = a.clone().combine(ConvergenceLoss::zero());
        assert_eq!(a, b);
    }

    #[test]
    fn convergence_total_is_absorbing() {
        let t = ConvergenceLoss::total();
        let x = ConvergenceLoss::new(42);
        assert_eq!(t.clone().combine(x.clone()).steps(), usize::MAX);
        assert_eq!(x.combine(t).steps(), usize::MAX);
    }

    #[test]
    fn convergence_default_is_zero() {
        let l = ConvergenceLoss::default();
        assert!(l.is_zero());
    }

    #[test]
    fn convergence_display() {
        let l = ConvergenceLoss::new(3);
        assert_eq!(format!("{}", l), "3 steps from crystal");
    }

    #[test]
    fn imperfect_with_convergence_loss() {
        let i: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Partial(42, ConvergenceLoss::new(3));
        assert!(i.is_partial());
        assert_eq!(i.loss().steps(), 3);
        assert_eq!(i.ok(), Some(42));
    }

    #[test]
    fn imperfect_convergence_map() {
        let i: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Partial(10, ConvergenceLoss::new(2));
        let m = i.map(|v| v * 3);
        assert_eq!(m.loss().steps(), 2);
        assert_eq!(m.ok(), Some(30));
    }

    #[test]
    fn imperfect_convergence_compose() {
        let a: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Partial(1, ConvergenceLoss::new(3));
        let b: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Partial(2, ConvergenceLoss::new(5));
        let c = a.compose(b);
        assert!(c.is_partial());
        assert_eq!(c.loss().steps(), 5);
        assert_eq!(c.ok(), Some(2));
    }

    // --- ApertureLoss ---

    #[test]
    fn aperture_zero() {
        let l = ApertureLoss::zero();
        assert!(l.is_zero());
        assert!(l.dark_dims().is_empty());
        assert_eq!(l.aperture(), 0.0);
    }

    #[test]
    fn aperture_total() {
        let l = ApertureLoss::total();
        assert!(!l.is_zero());
        assert_eq!(l.aperture(), 1.0);
    }

    #[test]
    fn aperture_new() {
        let l = ApertureLoss::new(vec![0, 2], 4);
        assert_eq!(l.dark_dims(), &[0, 2]);
        assert_eq!(l.aperture(), 0.5);
        assert!(!l.is_zero());
    }

    #[test]
    fn aperture_new_zero_total_dims() {
        let l = ApertureLoss::new(vec![], 0);
        assert!(l.is_zero());
    }

    #[test]
    fn aperture_combine_unions_dims() {
        let a = ApertureLoss::new(vec![0, 2], 4);
        let b = ApertureLoss::new(vec![1, 2], 4);
        let c = a.combine(b);
        assert_eq!(c.dark_dims(), &[0, 1, 2]);
    }

    #[test]
    fn aperture_combine_zero_is_identity() {
        let a = ApertureLoss::new(vec![1, 3], 4);
        let b = a.clone().combine(ApertureLoss::zero());
        assert_eq!(a.dark_dims(), b.dark_dims());
    }

    #[test]
    fn aperture_total_is_absorbing() {
        let t = ApertureLoss::total();
        let x = ApertureLoss::new(vec![0], 4);
        let c = x.combine(t);
        assert_eq!(c.aperture(), 1.0);
    }

    #[test]
    fn aperture_default_is_zero() {
        let l = ApertureLoss::default();
        assert!(l.is_zero());
    }

    #[test]
    fn aperture_display() {
        let l = ApertureLoss::new(vec![0, 2], 4);
        assert_eq!(format!("{}", l), "50.0% dark (dims: [0, 2])");
    }

    #[test]
    fn imperfect_with_aperture_loss() {
        let i: Imperfect<Vec<f64>, String, ApertureLoss> =
            Imperfect::Partial(vec![1.0, 0.0, 3.0, 0.0], ApertureLoss::new(vec![1, 3], 4));
        assert!(i.is_partial());
        assert_eq!(i.loss().dark_dims(), &[1, 3]);
        assert_eq!(i.loss().aperture(), 0.5);
    }

    #[test]
    fn imperfect_aperture_map() {
        let i: Imperfect<u32, String, ApertureLoss> =
            Imperfect::Partial(10, ApertureLoss::new(vec![0], 3));
        let m = i.map(|v| v + 1);
        assert_eq!(m.loss().dark_dims(), &[0]);
        assert_eq!(m.ok(), Some(11));
    }

    #[test]
    fn imperfect_aperture_compose() {
        let a: Imperfect<u32, String, ApertureLoss> =
            Imperfect::Partial(1, ApertureLoss::new(vec![0], 4));
        let b: Imperfect<u32, String, ApertureLoss> =
            Imperfect::Partial(2, ApertureLoss::new(vec![2], 4));
        let c = a.compose(b);
        assert!(c.is_partial());
        assert_eq!(c.loss().dark_dims(), &[0, 2]);
        assert_eq!(c.ok(), Some(2));
    }

    // --- RoutingLoss ---

    #[test]
    fn routing_zero() {
        let l = RoutingLoss::zero();
        assert!(l.is_zero());
        assert_eq!(l.entropy(), 0.0);
        assert_eq!(l.runner_up_gap(), 1.0);
    }

    #[test]
    fn routing_total() {
        let l = RoutingLoss::total();
        assert!(!l.is_zero());
        assert!(l.entropy().is_infinite());
        assert_eq!(l.runner_up_gap(), 0.0);
    }

    #[test]
    fn routing_new() {
        let l = RoutingLoss::new(1.5, 0.3);
        assert_eq!(l.entropy(), 1.5);
        assert_eq!(l.runner_up_gap(), 0.3);
        assert!(!l.is_zero());
    }

    #[test]
    fn routing_combine_takes_max_entropy() {
        let a = RoutingLoss::new(1.0, 0.5);
        let b = RoutingLoss::new(2.0, 0.8);
        let c = a.combine(b);
        assert_eq!(c.entropy(), 2.0);
        assert_eq!(c.runner_up_gap(), 0.5);
    }

    #[test]
    fn routing_combine_zero_is_identity() {
        let a = RoutingLoss::new(1.5, 0.4);
        let z = RoutingLoss::zero();
        let c = a.combine(z);
        assert_eq!(c.entropy(), 1.5);
    }

    #[test]
    fn routing_total_is_absorbing() {
        let t = RoutingLoss::total();
        let x = RoutingLoss::new(1.0, 0.5);
        let c = x.combine(t);
        assert!(c.entropy().is_infinite());
        assert_eq!(c.runner_up_gap(), 0.0);
    }

    #[test]
    fn routing_default_is_zero() {
        let l = RoutingLoss::default();
        assert!(l.is_zero());
    }

    #[test]
    fn routing_display() {
        let l = RoutingLoss::new(1.5, 0.3);
        assert_eq!(format!("{}", l), "1.5000 bits entropy, 30.0% gap");
    }

    #[test]
    fn imperfect_with_routing_loss() {
        let i: Imperfect<String, String, RoutingLoss> =
            Imperfect::Partial("gpt-4".into(), RoutingLoss::new(0.8, 0.15));
        assert!(i.is_partial());
        assert_eq!(i.loss().entropy(), 0.8);
        assert_eq!(i.loss().runner_up_gap(), 0.15);
    }

    #[test]
    fn imperfect_routing_map() {
        let i: Imperfect<u32, String, RoutingLoss> =
            Imperfect::Partial(10, RoutingLoss::new(0.5, 0.9));
        let m = i.map(|v| v * 2);
        assert_eq!(m.loss().entropy(), 0.5);
        assert_eq!(m.ok(), Some(20));
    }

    #[test]
    fn imperfect_routing_compose() {
        let a: Imperfect<u32, String, RoutingLoss> =
            Imperfect::Partial(1, RoutingLoss::new(0.5, 0.8));
        let b: Imperfect<u32, String, RoutingLoss> =
            Imperfect::Partial(2, RoutingLoss::new(1.2, 0.3));
        let c = a.compose(b);
        assert!(c.is_partial());
        assert_eq!(c.loss().entropy(), 1.2);
        assert_eq!(c.loss().runner_up_gap(), 0.3);
        assert_eq!(c.ok(), Some(2));
    }
}
