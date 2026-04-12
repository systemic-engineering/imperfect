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
    /// Perfect result, zero loss.
    Success(T),
    /// Value present, some information lost getting here.
    Partial(T, L),
    /// Failure, no value.
    Failure(E),
}

impl<T, E, L: Loss> Imperfect<T, E, L> {
    /// Returns `true` if the result has a value (Success or Partial).
    pub fn is_ok(&self) -> bool {
        !self.is_err()
    }

    /// Returns `true` if this is a Partial result.
    pub fn is_partial(&self) -> bool {
        matches!(self, Imperfect::Partial(_, _))
    }

    /// Returns `true` if this is a Failure.
    pub fn is_err(&self) -> bool {
        matches!(self, Imperfect::Failure(_))
    }

    /// Extract the value, discarding loss information. Returns `None` on Failure.
    pub fn ok(self) -> Option<T> {
        match self {
            Imperfect::Success(v) | Imperfect::Partial(v, _) => Some(v),
            Imperfect::Failure(_) => None,
        }
    }

    /// Extract the error. Returns `None` on Success or Partial.
    pub fn err(self) -> Option<E> {
        match self {
            Imperfect::Failure(e) => Some(e),
            _ => None,
        }
    }

    /// The loss incurred. Zero for Success, total for Failure, carried for Partial.
    pub fn loss(&self) -> L {
        match self {
            Imperfect::Success(_) => L::zero(),
            Imperfect::Partial(_, l) => l.clone(),
            Imperfect::Failure(_) => L::total(),
        }
    }

    /// Borrow the inner value and error without consuming `self`.
    pub fn as_ref(&self) -> Imperfect<&T, &E, L> {
        match self {
            Imperfect::Success(t) => Imperfect::Success(t),
            Imperfect::Partial(t, l) => Imperfect::Partial(t, l.clone()),
            Imperfect::Failure(e) => Imperfect::Failure(e),
        }
    }

    /// Transform the value, preserving loss and failure.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Imperfect<U, E, L> {
        match self {
            Imperfect::Success(t) => Imperfect::Success(f(t)),
            Imperfect::Partial(t, l) => Imperfect::Partial(f(t), l),
            Imperfect::Failure(e) => Imperfect::Failure(e),
        }
    }

    /// Transform the error, preserving value and loss.
    pub fn map_err<F>(self, f: impl FnOnce(E) -> F) -> Imperfect<T, F, L> {
        match self {
            Imperfect::Success(t) => Imperfect::Success(t),
            Imperfect::Partial(t, l) => Imperfect::Partial(t, l),
            Imperfect::Failure(e) => Imperfect::Failure(f(e)),
        }
    }

    /// Terni-functor bind. Chain an operation, accumulating loss.
    ///
    /// - Success: apply f, return its result
    /// - Partial: apply f, combine losses
    /// - Failure: short-circuit, f never called
    pub fn eh<U>(self, f: impl FnOnce(T) -> Imperfect<U, E, L>) -> Imperfect<U, E, L> {
        match self {
            Imperfect::Success(t) => f(t),
            Imperfect::Partial(t, loss) => match f(t) {
                Imperfect::Success(u) => Imperfect::Partial(u, loss),
                Imperfect::Partial(u, loss2) => Imperfect::Partial(u, loss.combine(loss2)),
                Imperfect::Failure(e) => Imperfect::Failure(e),
            },
            Imperfect::Failure(e) => Imperfect::Failure(e),
        }
    }

    /// Alias for [`eh`](Self::eh). Readable prose form of the terni-functor bind.
    pub fn imperfect<U>(self, f: impl FnOnce(T) -> Imperfect<U, E, L>) -> Imperfect<U, E, L> {
        self.eh(f)
    }

    /// Alias for [`eh`](Self::eh). Mathematical form — the terni-functor bind.
    pub fn tri<U>(self, f: impl FnOnce(T) -> Imperfect<U, E, L>) -> Imperfect<U, E, L> {
        self.eh(f)
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

/// Terni-functor composition context.
///
/// Accumulates loss across a sequence of `Imperfect` operations,
/// converting each to `Result` for use with `?`.
///
/// Call `.finish()` to wrap the final value with accumulated loss.
#[must_use = "call .finish() to collect accumulated loss — dropping Eh discards loss"]
pub struct Eh<L: Loss> {
    accumulated: Option<L>,
}

impl<L: Loss> Eh<L> {
    /// Create a new composition context with zero accumulated loss.
    pub fn new() -> Self {
        Eh { accumulated: None }
    }

    /// Extract value from Imperfect, accumulating loss.
    /// Returns `Result<T, E>` for use with `?`.
    pub fn eh<T, E>(&mut self, imp: Imperfect<T, E, L>) -> Result<T, E> {
        match imp {
            Imperfect::Success(t) => Ok(t),
            Imperfect::Partial(t, loss) => {
                self.accumulated = Some(match self.accumulated.take() {
                    Some(existing) => existing.combine(loss),
                    None => loss,
                });
                Ok(t)
            }
            Imperfect::Failure(e) => Err(e),
        }
    }

    /// Alias for [`eh`](Self::eh).
    pub fn imperfect<T, E>(&mut self, imp: Imperfect<T, E, L>) -> Result<T, E> {
        self.eh(imp)
    }

    /// Alias for [`eh`](Self::eh).
    pub fn tri<T, E>(&mut self, imp: Imperfect<T, E, L>) -> Result<T, E> {
        self.eh(imp)
    }

    /// Wrap a final value with accumulated loss.
    /// Success if no loss accumulated. Partial if any did.
    pub fn finish<T, E>(self, value: T) -> Imperfect<T, E, L> {
        match self.accumulated {
            Some(loss) => Imperfect::Partial(value, loss),
            None => Imperfect::Success(value),
        }
    }

    /// Inspect accumulated loss without consuming the context.
    pub fn loss(&self) -> Option<&L> {
        self.accumulated.as_ref()
    }
}

impl<L: Loss> Default for Eh<L> {
    fn default() -> Self {
        Self::new()
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
    /// Create a new ConvergenceLoss with the given number of steps from crystal.
    pub fn new(steps: usize) -> Self {
        ConvergenceLoss(steps)
    }

    /// The number of steps remaining to reach crystal (convergence).
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
        ApertureLoss {
            dark_dims,
            aperture,
        }
    }

    /// Which dimension indices were dark (unobserved).
    pub fn dark_dims(&self) -> &[usize] {
        &self.dark_dims
    }

    /// Fraction of dimensions that were dark (0.0 to 1.0).
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
        ApertureLoss {
            dark_dims: dims,
            aperture,
        }
    }
}

impl std::fmt::Display for ApertureLoss {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:.1}% dark (dims: {:?})",
            self.aperture * 100.0,
            self.dark_dims
        )
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
        RoutingLoss {
            entropy,
            runner_up_gap,
        }
    }

    /// Shannon entropy of the routing distribution (bits).
    pub fn entropy(&self) -> f64 {
        self.entropy
    }

    /// Probability gap between top pick and runner-up (0.0 to 1.0).
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

    // --- eh: terni-functor bind ---

    #[test]
    fn eh_chains_success() {
        let result: Imperfect<i32, String, ConvergenceLoss> = Imperfect::Success(1)
            .eh(|x| Imperfect::Success(x + 1))
            .eh(|x| Imperfect::Success(x + 1));
        assert_eq!(result, Imperfect::Success(3));
    }

    #[test]
    fn eh_accumulates_loss() {
        let result = Imperfect::<i32, String, ConvergenceLoss>::Partial(1, ConvergenceLoss(3))
            .eh(|x| Imperfect::Partial(x + 1, ConvergenceLoss(5)));
        assert!(result.is_partial());
        assert_eq!(result.loss(), ConvergenceLoss(5));
    }

    #[test]
    fn eh_shortcircuits_on_failure() {
        let result = Imperfect::<i32, String, ConvergenceLoss>::Failure("boom".into())
            .eh(|x| Imperfect::Success(x + 1));
        assert!(result.is_err());
    }

    #[test]
    fn eh_partial_then_success_stays_partial() {
        let result = Imperfect::<i32, String, ConvergenceLoss>::Partial(1, ConvergenceLoss(3))
            .eh(|x| Imperfect::Success(x + 1));
        assert!(result.is_partial());
        assert_eq!(result.clone().ok(), Some(2));
        assert_eq!(result.loss(), ConvergenceLoss(3));
    }

    #[test]
    fn eh_success_then_partial_becomes_partial() {
        let result = Imperfect::<i32, String, ConvergenceLoss>::Success(1)
            .eh(|x| Imperfect::Partial(x + 1, ConvergenceLoss(5)));
        assert!(result.is_partial());
        assert_eq!(result.ok(), Some(2));
    }

    #[test]
    fn imperfect_alias_works() {
        let result = Imperfect::<i32, String, ConvergenceLoss>::Success(1)
            .imperfect(|x| Imperfect::Success(x + 1));
        assert_eq!(result, Imperfect::Success(2));
    }

    #[test]
    fn tri_alias_works() {
        let result = Imperfect::<i32, String, ConvergenceLoss>::Success(1)
            .tri(|x| Imperfect::Success(x + 1));
        assert_eq!(result, Imperfect::Success(2));
    }

    // --- Eh context struct ---

    #[test]
    fn eh_context_accumulates_loss() {
        let mut eh = Eh::new();
        let a: Result<i32, String> = eh.eh(Imperfect::<i32, String, ConvergenceLoss>::Partial(
            1,
            ConvergenceLoss(3),
        ));
        assert_eq!(a, Ok(1));
        assert_eq!(eh.loss(), Some(&ConvergenceLoss(3)));
    }

    #[test]
    fn eh_context_success_no_loss() {
        let mut eh: Eh<ConvergenceLoss> = Eh::new();
        let a = eh.eh(Imperfect::<i32, String, ConvergenceLoss>::Success(1));
        assert_eq!(a, Ok(1));
        assert_eq!(eh.loss(), None);
    }

    #[test]
    fn eh_context_failure_returns_err() {
        let mut eh: Eh<ConvergenceLoss> = Eh::new();
        let a: Result<i32, String> = eh.eh(Imperfect::<i32, String, ConvergenceLoss>::Failure(
            "boom".into(),
        ));
        assert_eq!(a, Err("boom".into()));
    }

    #[test]
    fn eh_context_combines_losses() {
        let mut eh = Eh::new();
        let _ = eh.eh(Imperfect::<i32, String, ConvergenceLoss>::Partial(
            1,
            ConvergenceLoss(3),
        ));
        let _ = eh.eh(Imperfect::<i32, String, ConvergenceLoss>::Partial(
            2,
            ConvergenceLoss(7),
        ));
        assert_eq!(eh.loss(), Some(&ConvergenceLoss(7)));
    }

    #[test]
    fn eh_context_finish_success_when_no_loss() {
        let eh: Eh<ConvergenceLoss> = Eh::new();
        let result: Imperfect<i32, String, ConvergenceLoss> = eh.finish(42);
        assert_eq!(result, Imperfect::Success(42));
    }

    #[test]
    fn eh_context_finish_partial_when_loss() {
        let mut eh = Eh::new();
        let _ = eh.eh(Imperfect::<i32, String, ConvergenceLoss>::Partial(
            1,
            ConvergenceLoss(5),
        ));
        let result: Imperfect<i32, String, ConvergenceLoss> = eh.finish(42);
        assert!(result.is_partial());
        assert_eq!(result.clone().ok(), Some(42));
    }

    #[test]
    fn eh_context_imperfect_alias() {
        let mut eh = Eh::new();
        let a = eh.imperfect(Imperfect::<i32, String, ConvergenceLoss>::Success(1));
        assert_eq!(a, Ok(1));
    }

    #[test]
    fn eh_context_tri_alias() {
        let mut eh = Eh::new();
        let a = eh.tri(Imperfect::<i32, String, ConvergenceLoss>::Success(1));
        assert_eq!(a, Ok(1));
    }

    fn example_pipeline(input: i32) -> Imperfect<i32, String, ConvergenceLoss> {
        let mut eh = Eh::new();
        let a: Result<i32, String> = eh.eh(if input > 0 {
            Imperfect::Success(input)
        } else {
            Imperfect::Failure("negative".into())
        });

        match a {
            Ok(val) => eh.finish(val * 2),
            Err(_) => Imperfect::Failure("negative".into()),
        }
    }

    #[test]
    fn example_pipeline_success() {
        let result = example_pipeline(5);
        assert_eq!(result, Imperfect::Success(10));
    }

    #[test]
    fn example_pipeline_failure() {
        let result = example_pipeline(-1);
        assert!(result.is_err());
    }

    #[test]
    fn eh_context_default() {
        let eh: Eh<ConvergenceLoss> = Eh::default();
        assert_eq!(eh.loss(), None);
    }
}
