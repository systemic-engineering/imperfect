#![deny(missing_docs)]

//! I wanna thank Brené Brown for her work.
//!
//!
//! Result extended with partial success. Three states:
//!
//! - **`Success(T)`** — the transformation preserved everything. Zero loss.
//! - **`Partial(T, L)`** — a value came through, but something was lost getting here.
//!   The loss is measured and carried forward.
//! - **`Failure(E, L)`** — no value survived, but the cost of getting here is measured.
//!   The accumulated loss tells you what it cost to arrive at this failure.
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
//!
//! ## Constructors
//!
//! ```rust
//! use terni::{Imperfect, ConvergenceLoss};
//!
//! // The four ways to construct an Imperfect:
//! let perfect: Imperfect<i32, String, ConvergenceLoss> = Imperfect::Success(42);
//! let lossy: Imperfect<i32, String, ConvergenceLoss> = Imperfect::Partial(42, ConvergenceLoss::new(3));
//! let failed: Imperfect<i32, String, ConvergenceLoss> = Imperfect::Failure("gone".into(), ConvergenceLoss::new(0));
//! let failed_with_cost: Imperfect<i32, String, ConvergenceLoss> = Imperfect::Failure("gone".into(), ConvergenceLoss::new(5));
//!
//! assert!(perfect.is_ok());
//! assert!(lossy.is_partial());
//! assert!(failed.is_err());
//! // Failure carries accumulated loss — the cost of getting here:
//! assert_eq!(failed_with_cost.loss().steps(), 5);
//! ```
//!
//! ## The Terni-Functor
//!
//! `Imperfect` is a terni-functor — a three-state composition that accumulates
//! loss through the middle state. The bind operator comes in three flavors:
//!
//! - `.eh()` — the shrug. For engineers who get it.
//! - `.imp()` — the name. For the mischievous ones.
//! - `.tri()` — the math. For engineers who know what a terni-functor is.
//!
//! ### Pipeline
//!
//! ```rust
//! use terni::{Imperfect, ConvergenceLoss};
//!
//! let result = Imperfect::<i32, String, ConvergenceLoss>::Success(1)
//!     .eh(|x| Imperfect::Success(x * 2))
//!     .eh(|x| Imperfect::Partial(x + 1, ConvergenceLoss::new(3)));
//!
//! assert!(result.is_partial());
//! assert_eq!(result.ok(), Some(3));
//! ```
//!
//! ### Recovery
//!
//! ```rust
//! use terni::{Imperfect, ConvergenceLoss};
//!
//! // Recovery from failure always produces Partial — the failure happened,
//! // and that cost is carried forward.
//! let failed: Imperfect<i32, String, ConvergenceLoss> =
//!     Imperfect::Failure("gone".into(), ConvergenceLoss::new(3));
//!
//! let recovered = failed.recover(|_e| Imperfect::Success(0));
//!
//! assert!(recovered.is_partial());  // never Success — the failure was real
//! assert_eq!(recovered.loss().steps(), 3);  // cost survives
//! assert_eq!(recovered.ok(), Some(0));
//! ```
//!
//! ### Explicit Context
//!
//! ```rust
//! use terni::{Imperfect, Eh, ConvergenceLoss};
//!
//! let mut eh = Eh::new();
//! let a = eh.imp(Imperfect::<i32, String, ConvergenceLoss>::Success(1)).unwrap();
//! let b = eh.imp(Imperfect::<_, String, _>::Partial(a + 1, ConvergenceLoss::new(5))).unwrap();
//! let result: Imperfect<i32, String, ConvergenceLoss> = eh.finish(b);
//!
//! assert!(result.is_partial());
//! ```

#[cfg(feature = "macros")]
pub use terni_macros::eh;

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
/// - `Failure(E, L)` — failure, no value, but the cost of getting here is measured.
///
/// The design descends from PbtA (Powered by the Apocalypse) tabletop games,
/// which use three outcome tiers: 10+ is full success, 7-9 is success with
/// complications, 6- is failure. The middle tier — success with cost — is the
/// design innovation that PbtA contributed to game design. This crate encodes
/// that structure in types.
///
/// Follows `Result` conventions: `is_ok()` means "has a value" (Success or Partial).
/// The `.ok()` and `.err()` extractor methods follow `Result` naming conventions.
#[must_use = "this `Imperfect` may carry loss information that should not be silently discarded"]
#[derive(Clone, Debug, PartialEq)]
pub enum Imperfect<T, E, L: Loss> {
    /// Perfect result, zero loss.
    Success(T),
    /// Value present, some information lost getting here.
    Partial(T, L),
    /// Failure, no value, but the cost of getting here is measured.
    Failure(E, L),
}

/// Propagate accumulated loss through the next step's result.
///
/// Extracted as a standalone function so that LLVM creates a single
/// monomorphization per `(T, E, L)` triple, rather than one per closure.
/// This avoids phantom "missed lines" in coverage from closure-specific
/// monomorphizations that never exercise certain match arms.
fn propagate_loss<T, E, L: Loss>(loss: L, next: Imperfect<T, E, L>) -> Imperfect<T, E, L> {
    match next {
        Imperfect::Success(u) => Imperfect::Partial(u, loss),
        Imperfect::Partial(u, loss2) => Imperfect::Partial(u, loss.combine(loss2)),
        Imperfect::Failure(e, loss2) => Imperfect::Failure(e, loss.combine(loss2)),
    }
}

impl<T, E, L: Loss> Imperfect<T, E, L> {
    // --- Constructors ---

    /// Construct a success. Alias for `Success(value)`.
    pub fn success(value: T) -> Self {
        Imperfect::Success(value)
    }

    /// Construct a partial result with measured loss. Alias for `Partial(value, loss)`.
    pub fn partial(value: T, loss: L) -> Self {
        Imperfect::Partial(value, loss)
    }

    /// Construct a failure with zero accumulated loss.
    pub fn failure(error: E) -> Self {
        Imperfect::Failure(error, L::zero())
    }

    /// Construct a failure carrying accumulated loss from prior steps.
    pub fn failure_with_loss(error: E, loss: L) -> Self {
        Imperfect::Failure(error, loss)
    }

    // --- Queries ---

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
        matches!(self, Imperfect::Failure(_, _))
    }

    /// Extract the value, discarding loss information. Returns `None` on Failure.
    pub fn ok(self) -> Option<T> {
        match self {
            Imperfect::Success(v) | Imperfect::Partial(v, _) => Some(v),
            Imperfect::Failure(_, _) => None,
        }
    }

    /// Extract the error. Returns `None` on Success or Partial.
    pub fn err(self) -> Option<E> {
        match self {
            Imperfect::Failure(e, _) => Some(e),
            _ => None,
        }
    }

    /// Extract the error and accumulated loss. Returns `None` on Success or Partial.
    ///
    /// Unlike `.err()` which drops the loss, this returns both the error and the
    /// loss that accumulated before the failure. This is information you can't
    /// recover any other way — `L::total()` can always be reconstructed from the
    /// type, but the pre-failure loss cannot.
    pub fn err_with_loss(self) -> Option<(E, L)> {
        match self {
            Imperfect::Failure(e, l) => Some((e, l)),
            _ => None,
        }
    }

    /// The loss incurred. Zero for Success, carried for Partial and Failure.
    ///
    /// Failure carries the accumulated loss from before the failure — the cost
    /// of getting here. If you need `L::total()`, check `is_err()`.
    pub fn loss(&self) -> L {
        match self {
            Imperfect::Success(_) => L::zero(),
            Imperfect::Partial(_, l) => l.clone(),
            Imperfect::Failure(_, l) => l.clone(),
        }
    }

    /// Borrow the inner value and error without consuming `self`.
    pub fn as_ref(&self) -> Imperfect<&T, &E, L> {
        match self {
            Imperfect::Success(t) => Imperfect::Success(t),
            Imperfect::Partial(t, l) => Imperfect::Partial(t, l.clone()),
            Imperfect::Failure(e, l) => Imperfect::Failure(e, l.clone()),
        }
    }

    /// Transform the value, preserving loss and failure.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Imperfect<U, E, L> {
        match self {
            Imperfect::Success(t) => Imperfect::Success(f(t)),
            Imperfect::Partial(t, l) => Imperfect::Partial(f(t), l),
            Imperfect::Failure(e, l) => Imperfect::Failure(e, l),
        }
    }

    /// Transform the error, preserving value and loss.
    pub fn map_err<F>(self, f: impl FnOnce(E) -> F) -> Imperfect<T, F, L> {
        match self {
            Imperfect::Success(t) => Imperfect::Success(t),
            Imperfect::Partial(t, l) => Imperfect::Partial(t, l),
            Imperfect::Failure(e, l) => Imperfect::Failure(f(e), l),
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
            Imperfect::Partial(t, loss) => propagate_loss(loss, f(t)),
            Imperfect::Failure(e, loss) => Imperfect::Failure(e, loss),
        }
    }

    /// Alias for [`eh`](Self::eh). The name. For the mischievous ones.
    pub fn imp<U>(self, f: impl FnOnce(T) -> Imperfect<U, E, L>) -> Imperfect<U, E, L> {
        self.eh(f)
    }

    /// Alias for [`eh`](Self::eh). Mathematical form — the terni-functor bind.
    pub fn tri<U>(self, f: impl FnOnce(T) -> Imperfect<U, E, L>) -> Imperfect<U, E, L> {
        self.eh(f)
    }

    /// Attempt recovery from Failure. The loss carries forward.
    ///
    /// Success and Partial pass through unchanged.
    /// Failure → recovery function → result carries the failure's accumulated loss.
    ///
    /// Recovery from Failure never produces Success — because the failure happened.
    /// The loss is real. The best you can do is recover a value and carry the cost.
    pub fn recover(self, f: impl FnOnce(E) -> Imperfect<T, E, L>) -> Imperfect<T, E, L> {
        match self {
            Imperfect::Success(v) => Imperfect::Success(v),
            Imperfect::Partial(v, l) => Imperfect::Partial(v, l),
            Imperfect::Failure(e, loss) => match f(e) {
                Imperfect::Success(v) => Imperfect::Partial(v, loss),
                Imperfect::Partial(v, l2) => Imperfect::Partial(v, loss.combine(l2)),
                Imperfect::Failure(e2, l2) => Imperfect::Failure(e2, loss.combine(l2)),
            },
        }
    }

    /// Recover a default value from Failure. Always produces Partial.
    /// You can't un-fail. But you can get something back. The loss survives.
    pub fn unwrap_or_else(self, f: impl FnOnce(E) -> T) -> Imperfect<T, E, L> {
        match self {
            Imperfect::Success(v) => Imperfect::Success(v),
            Imperfect::Partial(v, l) => Imperfect::Partial(v, l),
            Imperfect::Failure(e, loss) => Imperfect::Partial(f(e), loss),
        }
    }

    /// Recover with a static default. Always produces Partial on Failure.
    pub fn unwrap_or(self, default: T) -> Imperfect<T, E, L> {
        self.unwrap_or_else(|_| default)
    }

    /// Propagate accumulated loss from `self` through `next`.
    ///
    /// Deprecated in favor of [`eh`](Self::eh) / [`imp`](Self::imp) / [`tri`](Self::tri).
    /// Kept for backward compatibility.
    ///
    /// - Success + next → next (no loss to propagate)
    /// - Partial(_, loss) + Success(v) → Partial(v, loss)
    /// - Partial(_, loss1) + Partial(v, loss2) → Partial(v, loss1.combine(loss2))
    /// - Partial(_, loss1) + Failure(e, loss2) → Failure(e, loss1.combine(loss2))
    /// - Failure(e, loss) + anything → Failure(e, loss) (short-circuits, `next` is discarded)
    pub fn compose<T2, E2>(self, next: Imperfect<T2, E2, L>) -> Imperfect<T2, E2, L>
    where
        E: Into<E2>,
    {
        match self {
            Imperfect::Failure(e, loss) => Imperfect::Failure(e.into(), loss),
            Imperfect::Success(_) => next,
            Imperfect::Partial(_, loss) => propagate_loss(loss, next),
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
            Imperfect::Failure(e, loss) => {
                self.accumulated = Some(match self.accumulated.take() {
                    Some(existing) => existing.combine(loss),
                    None => loss,
                });
                Err(e)
            }
        }
    }

    /// Alias for [`eh`](Self::eh). The name. For the mischievous ones.
    pub fn imp<T, E>(&mut self, imp: Imperfect<T, E, L>) -> Result<T, E> {
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

    /// Consume the context and return accumulated loss, if any.
    pub fn into_loss(self) -> Option<L> {
        self.accumulated
    }
}

impl<L: Loss> Default for Eh<L> {
    fn default() -> Self {
        Self::new()
    }
}

// --- IntoEh trait: type-dispatched extraction for eh! macro ---

/// Trait for types that can be extracted through an [`Eh`] context.
///
/// Implemented for [`Imperfect`] (accumulates loss) and [`Result`](core::result::Result)
/// (passes through). Used by the `eh!` macro to handle both types with `?`.
pub trait IntoEh<T, E, L: Loss> {
    /// Extract the value, accumulating loss if applicable.
    fn into_eh(self, ctx: &mut Eh<L>) -> Result<T, E>;
}

impl<T, E, L: Loss> IntoEh<T, E, L> for Imperfect<T, E, L> {
    fn into_eh(self, ctx: &mut Eh<L>) -> Result<T, E> {
        ctx.eh(self)
    }
}

impl<T, E, L: Loss> IntoEh<T, E, L> for Result<T, E> {
    fn into_eh(self, _ctx: &mut Eh<L>) -> Result<T, E> {
        self
    }
}

// --- std interop ---

impl<T, E, L: Loss> From<Result<T, E>> for Imperfect<T, E, L> {
    fn from(r: Result<T, E>) -> Self {
        match r {
            Ok(v) => Imperfect::Success(v),
            Err(e) => Imperfect::Failure(e, L::zero()),
        }
    }
}

impl<T, E, L: Loss> From<Imperfect<T, E, L>> for Result<T, E> {
    fn from(i: Imperfect<T, E, L>) -> Self {
        match i {
            Imperfect::Success(v) | Imperfect::Partial(v, _) => Ok(v),
            Imperfect::Failure(e, _) => Err(e),
        }
    }
}

/// `None` maps to `Failure(())` because absence is total loss — there is no
/// value and no meaningful error to report. `Some(v)` maps to `Success(v)`.
impl<T, L: Loss> From<Option<T>> for Imperfect<T, (), L> {
    fn from(o: Option<T>) -> Self {
        match o {
            Some(v) => Imperfect::Success(v),
            None => Imperfect::Failure((), L::zero()),
        }
    }
}

// --- Standard library Loss implementations ---
//
// These implementations cover common types that naturally form monoids under
// `combine`. They fall into three categories:
//
// **Numeric losses** (`usize`, `u64`, `f64`): The simplest — just a count or
// magnitude. `combine` is addition (saturating for integers, IEEE for floats).
// `total()` is `MAX` / `INFINITY`, which acts as a proper absorbing element.
//
// **Collection losses** (`Vec<T>`, `HashSet<T>`, `BTreeSet<T>`, `String`):
// Track *what* was lost, not just *how much*. `combine` appends or unions.
//
// > **Limitation**: Collections have no natural absorbing element. There is no
// > `Vec` value `t` such that `x.combine(t) == t` for all `x`. `total()`
// > returns the same as `zero()` (empty). The monoid identity and associativity
// > laws hold, but the absorbing property of `total()` does not. If you need
// > absorbing semantics, use a numeric loss type.
//
// **Tuple combinator** (`(A, B)`): Compose two loss dimensions independently.
// Both components must be `Loss` types. `total()` absorbs correctly when both
// components do.

use std::collections::{BTreeSet, HashSet};
use std::hash::Hash;

// --- Vec<T>: labeled loss — track WHAT was lost as a list of events ---

impl<T: Clone> Loss for Vec<T> {
    fn zero() -> Self {
        Vec::new()
    }

    fn total() -> Self {
        Vec::new()
    }

    fn is_zero(&self) -> bool {
        self.is_empty()
    }

    fn combine(mut self, other: Self) -> Self {
        self.extend(other);
        self
    }
}

// --- HashSet<T>: unique losses — deduplicated loss tracking ---

impl<T: Eq + Hash + Clone> Loss for HashSet<T> {
    fn zero() -> Self {
        HashSet::new()
    }

    fn total() -> Self {
        HashSet::new()
    }

    fn is_zero(&self) -> bool {
        self.is_empty()
    }

    fn combine(mut self, other: Self) -> Self {
        self.extend(other);
        self
    }
}

// --- BTreeSet<T>: ordered unique losses ---

impl<T: Ord + Clone> Loss for BTreeSet<T> {
    fn zero() -> Self {
        BTreeSet::new()
    }

    fn total() -> Self {
        BTreeSet::new()
    }

    fn is_zero(&self) -> bool {
        self.is_empty()
    }

    fn combine(mut self, other: Self) -> Self {
        self.extend(other);
        self
    }
}

// --- String: human-readable loss log ---

impl Loss for String {
    fn zero() -> Self {
        String::new()
    }

    fn total() -> Self {
        String::new()
    }

    fn is_zero(&self) -> bool {
        self.is_empty()
    }

    fn combine(mut self, other: Self) -> Self {
        if !self.is_empty() && !other.is_empty() {
            self.push_str("; ");
        }
        self.push_str(&other);
        self
    }
}

// --- Numeric losses: usize, u64, f64 ---

impl Loss for usize {
    fn zero() -> Self {
        0
    }

    fn total() -> Self {
        usize::MAX
    }

    fn is_zero(&self) -> bool {
        *self == 0
    }

    fn combine(self, other: Self) -> Self {
        self.saturating_add(other)
    }
}

impl Loss for u64 {
    fn zero() -> Self {
        0
    }

    fn total() -> Self {
        u64::MAX
    }

    fn is_zero(&self) -> bool {
        *self == 0
    }

    fn combine(self, other: Self) -> Self {
        self.saturating_add(other)
    }
}

impl Loss for f64 {
    fn zero() -> Self {
        0.0
    }

    fn total() -> Self {
        f64::INFINITY
    }

    fn is_zero(&self) -> bool {
        *self == 0.0
    }

    fn combine(self, other: Self) -> Self {
        self + other
    }
}

// --- Tuple combinator: compose two independent loss dimensions ---

impl<A: Loss, B: Loss> Loss for (A, B) {
    fn zero() -> Self {
        (A::zero(), B::zero())
    }

    fn total() -> Self {
        (A::total(), B::total())
    }

    fn is_zero(&self) -> bool {
        self.0.is_zero() && self.1.is_zero()
    }

    fn combine(self, other: Self) -> Self {
        (self.0.combine(other.0), self.1.combine(other.1))
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
        self.entropy == 0.0 && self.runner_up_gap == 1.0
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
    use std::collections::{BTreeSet, HashSet};

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
        let i: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Failure("oops".into(), ConvergenceLoss(0));
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
        let i: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Failure("oops".into(), ConvergenceLoss(0));
        assert_eq!(i.ok(), None);
    }

    #[test]
    fn err_returns_error() {
        let i: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Failure("oops".into(), ConvergenceLoss(0));
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
    fn loss_err_carries_loss() {
        let i: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Failure("oops".into(), ConvergenceLoss(7));
        assert_eq!(i.loss().steps(), 7);
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
        let i: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Failure("oops".into(), ConvergenceLoss(0));
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
        let i: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Failure("oops".into(), ConvergenceLoss(0));
        let m = i.map(double_u32);
        assert!(m.is_err());
    }

    #[test]
    fn map_err_transforms_error() {
        let i: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Failure("oops".into(), ConvergenceLoss(0));
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
        let a: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Failure("err".into(), ConvergenceLoss(0));
        let b: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Failure("err".into(), ConvergenceLoss(0));
        assert_eq!(a, b);
    }

    #[test]
    fn partial_eq_different_variants_not_equal() {
        let a: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Success(1);
        let b: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Failure("err".into(), ConvergenceLoss(0));
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
        let b: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Failure("fail".into(), ConvergenceLoss(0));
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
        let b: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Failure("fail".into(), ConvergenceLoss(0));
        let c = a.compose(b);
        assert!(c.is_err());
    }

    #[test]
    fn compose_err_shortcircuits() {
        let a: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Failure("fail".into(), ConvergenceLoss(0));
        let b: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Success(2);
        let c = a.compose(b);
        assert!(c.is_err());
        assert_eq!(c.err(), Some("fail".into()));
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
        let i: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Failure("oops".into(), ConvergenceLoss(0));
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
            .eh(convergence_add_one)
            .eh(convergence_add_one);
        assert_eq!(result, Imperfect::Success(3));
    }

    #[test]
    fn eh_accumulates_loss() {
        let result = Imperfect::<i32, String, ConvergenceLoss>::Partial(1, ConvergenceLoss(3))
            .eh(convergence_add_one_partial);
        assert!(result.is_partial());
        assert_eq!(result.loss(), ConvergenceLoss(5));
    }

    fn convergence_add_one(x: i32) -> Imperfect<i32, String, ConvergenceLoss> {
        Imperfect::Success(x + 1)
    }

    fn convergence_add_one_partial(x: i32) -> Imperfect<i32, String, ConvergenceLoss> {
        Imperfect::Partial(x + 1, ConvergenceLoss(5))
    }

    fn convergence_fail(_: i32) -> Imperfect<i32, String, ConvergenceLoss> {
        Imperfect::Failure("broke".into(), ConvergenceLoss(0))
    }

    #[test]
    fn eh_shortcircuits_on_failure() {
        let result =
            Imperfect::<i32, String, ConvergenceLoss>::Failure("boom".into(), ConvergenceLoss(0))
                .eh(convergence_add_one);
        assert!(result.is_err());
    }

    #[test]
    fn eh_partial_then_success_stays_partial() {
        let result = Imperfect::<i32, String, ConvergenceLoss>::Partial(1, ConvergenceLoss(3))
            .eh(convergence_add_one);
        assert!(result.is_partial());
        assert_eq!(result.clone().ok(), Some(2));
        assert_eq!(result.loss(), ConvergenceLoss(3));
    }

    #[test]
    fn eh_success_then_partial_becomes_partial() {
        let result =
            Imperfect::<i32, String, ConvergenceLoss>::Success(1).eh(convergence_add_one_partial);
        assert!(result.is_partial());
        assert_eq!(result.ok(), Some(2));
    }

    #[test]
    fn imp_alias_works() {
        let result = Imperfect::<i32, String, ConvergenceLoss>::Success(1).imp(convergence_add_one);
        assert_eq!(result, Imperfect::Success(2));
    }

    #[test]
    fn tri_alias_works() {
        let result = Imperfect::<i32, String, ConvergenceLoss>::Success(1).tri(convergence_add_one);
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
            ConvergenceLoss(0),
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
        let a = eh.imp(Imperfect::<i32, String, ConvergenceLoss>::Success(1));
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
            Imperfect::Failure("negative".into(), ConvergenceLoss(0))
        });

        match a {
            Ok(val) => eh.finish(val * 2),
            Err(_) => Imperfect::Failure("negative".into(), ConvergenceLoss(0)),
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

    // --- Failure(E, L): failure carries accumulated loss ---

    #[test]
    fn failure_carries_loss_through_eh_chain() {
        // Partial(3) -> Partial(5) -> Failure: both partial losses should combine into Failure
        let result = Imperfect::<i32, String, ConvergenceLoss>::Partial(1, ConvergenceLoss(3))
            .eh(|x| Imperfect::Partial(x + 1, ConvergenceLoss(5)))
            .eh(convergence_fail);
        assert!(result.is_err());
        // Failure should carry max(3, 5) = 5 from the partials, combined with the failure's own zero
        assert_eq!(result.loss().steps(), 5);
    }

    #[test]
    fn partial_then_failure_combines_losses() {
        let result =
            Imperfect::<i32, String, ConvergenceLoss>::Partial(1, ConvergenceLoss(3)).eh(|_| {
                Imperfect::<i32, String, ConvergenceLoss>::Failure(
                    "broke".into(),
                    ConvergenceLoss(7),
                )
            });
        assert!(result.is_err());
        // max(3, 7) = 7
        assert_eq!(result.loss().steps(), 7);
    }

    #[test]
    fn failure_loss_returns_carried_not_total() {
        let i: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Failure("oops".into(), ConvergenceLoss(3));
        // Should return 3, not usize::MAX
        assert_eq!(i.loss().steps(), 3);
    }

    #[test]
    fn failure_with_zero_loss() {
        let i: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Failure("oops".into(), ConvergenceLoss::zero());
        assert!(i.loss().is_zero());
    }

    #[test]
    fn err_with_loss_returns_both() {
        let i: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Failure("oops".into(), ConvergenceLoss(5));
        let (e, l) = i.err_with_loss().unwrap();
        assert_eq!(e, "oops");
        assert_eq!(l.steps(), 5);
    }

    #[test]
    fn err_with_loss_none_on_success() {
        let i: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Success(42);
        assert!(i.err_with_loss().is_none());
    }

    #[test]
    fn err_with_loss_none_on_partial() {
        let i: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Partial(42, ConvergenceLoss(3));
        assert!(i.err_with_loss().is_none());
    }

    #[test]
    fn eh_context_failure_accumulates_loss() {
        let mut eh = Eh::new();
        let _ = eh.eh(Imperfect::<i32, String, ConvergenceLoss>::Partial(
            1,
            ConvergenceLoss(3),
        ));
        let _ = eh.eh(Imperfect::<i32, String, ConvergenceLoss>::Failure(
            "boom".into(),
            ConvergenceLoss(5),
        ));
        // Failure's loss should accumulate into context: max(3, 5) = 5
        assert_eq!(eh.loss(), Some(&ConvergenceLoss(5)));
    }

    #[test]
    fn eh_context_failure_alone_carries_loss() {
        let mut eh: Eh<ConvergenceLoss> = Eh::new();
        let _ = eh.eh(Imperfect::<i32, String, ConvergenceLoss>::Failure(
            "boom".into(),
            ConvergenceLoss(7),
        ));
        assert_eq!(eh.loss(), Some(&ConvergenceLoss(7)));
    }

    #[test]
    fn compose_partial_failure_combines_loss() {
        let a: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Partial(1, ConvergenceLoss::new(3));
        let b: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Failure("fail".into(), ConvergenceLoss::new(5));
        let c = a.compose(b);
        assert!(c.is_err());
        // max(3, 5) = 5
        assert_eq!(c.loss().steps(), 5);
    }

    #[test]
    fn compose_failure_carries_loss_through() {
        let a: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Failure("fail".into(), ConvergenceLoss::new(4));
        let b: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Success(2);
        let c = a.compose(b);
        assert!(c.is_err());
        assert_eq!(c.loss().steps(), 4);
    }

    #[test]
    fn success_then_failure_carries_failure_loss() {
        let result = Imperfect::<i32, String, ConvergenceLoss>::Success(1).eh(|_| {
            Imperfect::<i32, String, ConvergenceLoss>::Failure("broke".into(), ConvergenceLoss(9))
        });
        assert!(result.is_err());
        assert_eq!(result.loss().steps(), 9);
    }

    #[test]
    fn failure_propagates_through_multiple_eh() {
        let result =
            Imperfect::<i32, String, ConvergenceLoss>::Failure("early".into(), ConvergenceLoss(3))
                .eh(convergence_add_one)
                .eh(convergence_add_one);
        assert!(result.is_err());
        assert_eq!(result.loss().steps(), 3);
        assert_eq!(result.err(), Some("early".into()));
    }

    #[test]
    fn map_err_preserves_failure_loss() {
        let i: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Failure("oops".into(), ConvergenceLoss(4));
        let m = i.map_err(|e| e.len());
        assert_eq!(m.loss().steps(), 4);
        assert_eq!(m.err(), Some(4));
    }

    #[test]
    fn as_ref_failure_preserves_loss() {
        let i: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Failure("oops".into(), ConvergenceLoss(6));
        let r = i.as_ref();
        assert!(r.is_err());
        assert_eq!(r.loss().steps(), 6);
    }

    #[test]
    fn from_result_err_has_zero_loss() {
        let r: Result<u32, String> = Err("oops".into());
        let i: Imperfect<u32, String, ConvergenceLoss> = r.into();
        assert!(i.is_err());
        assert!(i.loss().is_zero());
    }

    #[test]
    fn into_result_failure_drops_loss() {
        let i: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::Failure("oops".into(), ConvergenceLoss(5));
        let r: Result<u32, String> = i.into();
        assert_eq!(r, Err("oops".into()));
    }

    #[test]
    fn from_option_none_has_zero_loss() {
        let o: Option<u32> = None;
        let i: Imperfect<u32, (), ConvergenceLoss> = o.into();
        assert!(i.is_err());
        assert!(i.loss().is_zero());
    }

    // --- recover ---

    #[test]
    fn recover_success_passes_through() {
        let i: Imperfect<i32, String, ConvergenceLoss> = Imperfect::Success(42);
        let result = i.recover(|_| Imperfect::Success(0));
        assert_eq!(result, Imperfect::Success(42));
    }

    #[test]
    fn recover_partial_passes_through() {
        let i: Imperfect<i32, String, ConvergenceLoss> = Imperfect::Partial(42, ConvergenceLoss(3));
        let result = i.recover(|_| Imperfect::Success(0));
        assert_eq!(result, Imperfect::Partial(42, ConvergenceLoss(3)));
    }

    #[test]
    fn recover_failure_to_success_becomes_partial() {
        let i: Imperfect<i32, String, ConvergenceLoss> =
            Imperfect::Failure("oops".into(), ConvergenceLoss(5));
        let result = i.recover(|_| Imperfect::Success(99));
        // Recovery from failure never produces Success — the loss is real
        assert_eq!(result, Imperfect::Partial(99, ConvergenceLoss(5)));
    }

    #[test]
    fn recover_failure_to_partial_combines_losses() {
        let i: Imperfect<i32, String, ConvergenceLoss> =
            Imperfect::Failure("oops".into(), ConvergenceLoss(3));
        let result = i.recover(|_| Imperfect::Partial(99, ConvergenceLoss(7)));
        // max(3, 7) = 7
        assert_eq!(result, Imperfect::Partial(99, ConvergenceLoss(7)));
    }

    #[test]
    fn recover_failure_to_failure_combines_losses() {
        let i: Imperfect<i32, String, ConvergenceLoss> =
            Imperfect::Failure("first".into(), ConvergenceLoss(3));
        let result = i.recover(|_| Imperfect::Failure("second".into(), ConvergenceLoss(7)));
        assert_eq!(
            result,
            Imperfect::Failure("second".into(), ConvergenceLoss(7))
        );
    }

    #[test]
    fn recover_loss_accumulation_chain() {
        // Partial(3) -> eh fails with loss(2) -> recover succeeds with loss(1)
        let result = Imperfect::<i32, String, ConvergenceLoss>::Partial(1, ConvergenceLoss(3))
            .eh(|_| Imperfect::Failure("broke".into(), ConvergenceLoss(2)))
            .recover(|_| Imperfect::Partial(42, ConvergenceLoss(1)));
        // After eh: Failure with max(3, 2) = 3
        // After recover: Partial(42, max(3, 1)) = Partial(42, 3)
        assert_eq!(result, Imperfect::Partial(42, ConvergenceLoss(3)));
    }

    // --- unwrap_or_else ---

    #[test]
    fn unwrap_or_else_success_passes_through() {
        let i: Imperfect<i32, String, ConvergenceLoss> = Imperfect::Success(42);
        let result = i.unwrap_or_else(|_| 0);
        assert_eq!(result, Imperfect::Success(42));
    }

    #[test]
    fn unwrap_or_else_partial_passes_through() {
        let i: Imperfect<i32, String, ConvergenceLoss> = Imperfect::Partial(42, ConvergenceLoss(3));
        let result = i.unwrap_or_else(|_| 0);
        assert_eq!(result, Imperfect::Partial(42, ConvergenceLoss(3)));
    }

    #[test]
    fn unwrap_or_else_failure_becomes_partial() {
        let i: Imperfect<i32, String, ConvergenceLoss> =
            Imperfect::Failure("oops".into(), ConvergenceLoss(5));
        let result = i.unwrap_or_else(|e| e.len() as i32);
        // Partial with the failure's loss
        assert_eq!(result, Imperfect::Partial(4, ConvergenceLoss(5)));
    }

    // --- unwrap_or ---

    #[test]
    fn unwrap_or_success_passes_through() {
        let i: Imperfect<i32, String, ConvergenceLoss> = Imperfect::Success(42);
        let result = i.unwrap_or(0);
        assert_eq!(result, Imperfect::Success(42));
    }

    #[test]
    fn unwrap_or_partial_passes_through() {
        let i: Imperfect<i32, String, ConvergenceLoss> = Imperfect::Partial(42, ConvergenceLoss(3));
        let result = i.unwrap_or(0);
        assert_eq!(result, Imperfect::Partial(42, ConvergenceLoss(3)));
    }

    #[test]
    fn unwrap_or_failure_becomes_partial() {
        let i: Imperfect<i32, String, ConvergenceLoss> =
            Imperfect::Failure("oops".into(), ConvergenceLoss(5));
        let result = i.unwrap_or(-1);
        assert_eq!(result, Imperfect::Partial(-1, ConvergenceLoss(5)));
    }

    // --- pipeline integration ---

    fn fetch(url: &str) -> Imperfect<String, String, ConvergenceLoss> {
        if url == "good" {
            Imperfect::Success("data".into())
        } else {
            Imperfect::Failure(format!("404: {}", url), ConvergenceLoss(2))
        }
    }

    fn fetch_fallback(_url: &str) -> Imperfect<String, String, ConvergenceLoss> {
        Imperfect::Partial("cached".into(), ConvergenceLoss(5))
    }

    fn process(data: String) -> Imperfect<String, String, ConvergenceLoss> {
        Imperfect::Success(format!("processed:{}", data))
    }

    #[test]
    fn pipeline_recover_then_eh() {
        let result = fetch("bad").recover(|_| fetch_fallback("bad")).eh(process);
        // fetch fails with loss(2)
        // recover: fallback returns Partial("cached", 5) → combined: Partial("cached", max(2,5)=5)
        // eh(process): Success wraps, but Partial carries loss → Partial("processed:cached", 5)
        assert!(result.is_partial());
        assert_eq!(result.clone().ok(), Some("processed:cached".into()));
        assert_eq!(result.loss().steps(), 5);
    }

    #[test]
    fn pipeline_success_skips_recover() {
        let result = fetch("good")
            .recover(|_| fetch_fallback("good"))
            .eh(process);
        assert_eq!(result, Imperfect::Success("processed:data".into()));
    }

    // --- Vec<T> as Loss ---

    #[test]
    fn vec_zero() {
        let l = Vec::<String>::zero();
        assert!(l.is_zero());
        assert!(l.is_empty());
    }

    #[test]
    fn vec_total_is_empty() {
        // Collections have no absorbing element — total() == zero()
        let l = Vec::<String>::total();
        assert!(l.is_empty());
    }

    #[test]
    fn vec_combine_appends() {
        let a = vec!["field_x".to_string()];
        let b = vec!["field_y".to_string(), "field_z".to_string()];
        let c = a.combine(b);
        assert_eq!(c, vec!["field_x", "field_y", "field_z"]);
    }

    #[test]
    fn vec_combine_zero_is_identity() {
        let a = vec!["lost".to_string()];
        let c = a.clone().combine(Vec::zero());
        assert_eq!(a, c);
    }

    #[test]
    fn vec_is_zero_nonempty() {
        let a = vec![1u32];
        assert!(!a.is_zero());
    }

    #[test]
    fn vec_in_imperfect() {
        let i: Imperfect<u32, &str, Vec<String>> =
            Imperfect::Partial(42, vec!["dim_3 dark".into()]);
        assert!(i.is_partial());
        assert_eq!(i.loss(), vec!["dim_3 dark".to_string()]);
    }

    // --- HashSet<T> as Loss ---

    #[test]
    fn hashset_zero() {
        let l = HashSet::<String>::zero();
        assert!(l.is_zero());
    }

    #[test]
    fn hashset_total_is_empty() {
        let l = HashSet::<String>::total();
        assert!(l.is_empty());
    }

    #[test]
    fn hashset_combine_unions() {
        let a: HashSet<String> = ["x".into()].into_iter().collect();
        let b: HashSet<String> = ["x".into(), "y".into()].into_iter().collect();
        let c = a.combine(b);
        assert_eq!(c.len(), 2);
        assert!(c.contains("x"));
        assert!(c.contains("y"));
    }

    #[test]
    fn hashset_combine_zero_is_identity() {
        let a: HashSet<u32> = [1, 2].into_iter().collect();
        let c = a.clone().combine(HashSet::zero());
        assert_eq!(a, c);
    }

    #[test]
    fn hashset_is_zero_nonempty() {
        let a: HashSet<u32> = [1].into_iter().collect();
        assert!(!a.is_zero());
    }

    // --- BTreeSet<T> as Loss ---

    #[test]
    fn btreeset_zero() {
        let l = BTreeSet::<String>::zero();
        assert!(l.is_zero());
    }

    #[test]
    fn btreeset_total_is_empty() {
        let l = BTreeSet::<String>::total();
        assert!(l.is_empty());
    }

    #[test]
    fn btreeset_combine_unions_ordered() {
        let a: BTreeSet<u32> = [3, 1].into_iter().collect();
        let b: BTreeSet<u32> = [2, 1].into_iter().collect();
        let c = a.combine(b);
        let items: Vec<_> = c.into_iter().collect();
        assert_eq!(items, vec![1, 2, 3]);
    }

    #[test]
    fn btreeset_combine_zero_is_identity() {
        let a: BTreeSet<u32> = [5, 10].into_iter().collect();
        let c = a.clone().combine(BTreeSet::zero());
        assert_eq!(a, c);
    }

    #[test]
    fn btreeset_is_zero_nonempty() {
        let a: BTreeSet<u32> = [1].into_iter().collect();
        assert!(!a.is_zero());
    }

    // --- String as Loss ---

    #[test]
    fn string_zero() {
        let l = String::zero();
        assert!(l.is_zero());
        assert!(l.is_empty());
    }

    #[test]
    fn string_total_is_empty() {
        let l = String::total();
        assert!(l.is_empty());
    }

    #[test]
    fn string_combine_with_separator() {
        let a = "field_x lost".to_string();
        let b = "field_y truncated".to_string();
        let c = a.combine(b);
        assert_eq!(c, "field_x lost; field_y truncated");
    }

    #[test]
    fn string_combine_empty_left() {
        let a = String::new();
        let b = "something".to_string();
        let c = a.combine(b);
        assert_eq!(c, "something");
    }

    #[test]
    fn string_combine_empty_right() {
        let a = "something".to_string();
        let b = String::new();
        let c = a.combine(b);
        assert_eq!(c, "something");
    }

    #[test]
    fn string_combine_both_empty() {
        let a = String::new();
        let b = String::new();
        let c = a.combine(b);
        assert!(c.is_zero());
    }

    #[test]
    fn string_is_zero_nonempty() {
        assert!(!"hello".to_string().is_zero());
    }

    #[test]
    fn string_in_imperfect() {
        let i: Imperfect<u32, &str, String> = Imperfect::Partial(42, "precision lost".into());
        assert_eq!(i.loss(), "precision lost");
    }

    // --- usize as Loss ---

    #[test]
    fn usize_zero() {
        assert!(usize::zero().is_zero());
        assert_eq!(usize::zero(), 0);
    }

    #[test]
    fn usize_total() {
        assert_eq!(usize::total(), usize::MAX);
        assert!(!usize::total().is_zero());
    }

    #[test]
    fn usize_combine_adds() {
        assert_eq!(3usize.combine(4), 7);
    }

    #[test]
    fn usize_combine_saturates() {
        assert_eq!(usize::MAX.combine(1), usize::MAX);
    }

    #[test]
    fn usize_combine_zero_is_identity() {
        assert_eq!(5usize.combine(usize::zero()), 5);
    }

    #[test]
    fn usize_total_is_absorbing() {
        assert_eq!(42usize.combine(usize::total()), usize::MAX);
        assert_eq!(usize::total().combine(42), usize::MAX);
    }

    #[test]
    fn usize_in_imperfect() {
        let i: Imperfect<&str, &str, usize> = Imperfect::Partial("value", 3);
        assert_eq!(i.loss(), 3);
    }

    // --- u64 as Loss ---

    #[test]
    fn u64_zero() {
        assert!(u64::zero().is_zero());
        assert_eq!(u64::zero(), 0);
    }

    #[test]
    fn u64_total() {
        assert_eq!(u64::total(), u64::MAX);
    }

    #[test]
    fn u64_combine_adds() {
        assert_eq!(10u64.combine(20), 30);
    }

    #[test]
    fn u64_combine_saturates() {
        assert_eq!(u64::MAX.combine(1), u64::MAX);
    }

    #[test]
    fn u64_combine_zero_is_identity() {
        assert_eq!(7u64.combine(u64::zero()), 7);
    }

    #[test]
    fn u64_total_is_absorbing() {
        assert_eq!(99u64.combine(u64::total()), u64::MAX);
    }

    // --- f64 as Loss ---

    #[test]
    fn f64_zero() {
        assert!(f64::zero().is_zero());
        assert_eq!(f64::zero(), 0.0);
    }

    #[test]
    fn f64_total() {
        assert!(f64::total().is_infinite());
        assert!(!f64::total().is_zero());
    }

    #[test]
    fn f64_combine_adds() {
        assert_eq!((1.5f64).combine(2.5), 4.0);
    }

    #[test]
    fn f64_combine_zero_is_identity() {
        assert_eq!((2.75f64).combine(f64::zero()), 2.75);
    }

    #[test]
    fn f64_total_is_absorbing() {
        assert!((42.0f64).combine(f64::total()).is_infinite());
        assert!(f64::total().combine(42.0).is_infinite());
    }

    #[test]
    fn f64_is_zero_nonzero() {
        assert!(!(0.001f64).is_zero());
    }

    // --- (A, B) tuple as Loss ---

    #[test]
    fn tuple_zero() {
        let l = <(usize, f64)>::zero();
        assert!(l.is_zero());
        assert_eq!(l, (0, 0.0));
    }

    #[test]
    fn tuple_total() {
        let l = <(usize, f64)>::total();
        assert_eq!(l.0, usize::MAX);
        assert!(l.1.is_infinite());
    }

    #[test]
    fn tuple_is_zero_both_must_be_zero() {
        assert!(!(1usize, 0.0f64).is_zero());
        assert!(!(0usize, 1.0f64).is_zero());
        assert!((0usize, 0.0f64).is_zero());
    }

    #[test]
    fn tuple_combine_independent() {
        let a = (3usize, 1.0f64);
        let b = (4usize, 2.5f64);
        let c = a.combine(b);
        assert_eq!(c, (7, 3.5));
    }

    #[test]
    fn tuple_combine_zero_is_identity() {
        let a = (5usize, 2.0f64);
        let c = a.combine(<(usize, f64)>::zero());
        assert_eq!(c, (5, 2.0));
    }

    #[test]
    fn tuple_total_is_absorbing() {
        let a = (10usize, 3.0f64);
        let c = a.combine(<(usize, f64)>::total());
        assert_eq!(c.0, usize::MAX);
        assert!(c.1.is_infinite());
    }

    #[test]
    fn tuple_in_imperfect() {
        // Composite loss: count + magnitude
        let i: Imperfect<&str, &str, (usize, f64)> = Imperfect::Partial("value", (2, 0.5));
        let loss = i.loss();
        assert_eq!(loss.0, 2);
        assert_eq!(loss.1, 0.5);
    }

    #[test]
    fn tuple_nested() {
        // (usize, (f64, u64)) — three dimensions
        let a = (1usize, (0.5f64, 10u64));
        let b = (2usize, (1.5f64, 20u64));
        let c = a.combine(b);
        assert_eq!(c, (3, (2.0, 30)));
    }

    // --- associativity checks ---

    #[test]
    fn usize_associative() {
        let a = 1usize;
        let b = 2usize;
        let c = 3usize;
        assert_eq!(a.combine(b).combine(c), a.combine(b.combine(c)));
    }

    #[test]
    fn string_associative() {
        let a = "a".to_string();
        let b = "b".to_string();
        let c = "c".to_string();
        // (a; b); c vs a; (b; c)  — both should give "a; b; c"
        assert_eq!(
            a.clone().combine(b.clone()).combine(c.clone()),
            a.combine(b.combine(c))
        );
    }

    #[test]
    fn vec_associative() {
        let a = vec![1];
        let b = vec![2];
        let c = vec![3];
        assert_eq!(
            a.clone().combine(b.clone()).combine(c.clone()),
            a.combine(b.combine(c))
        );
    }

    // --- Constructor convenience methods ---

    #[test]
    fn constructor_success() {
        let i: Imperfect<u32, String, ConvergenceLoss> = Imperfect::success(42);
        assert_eq!(i, Imperfect::Success(42));
        assert!(i.is_ok());
        assert!(!i.is_partial());
    }

    #[test]
    fn constructor_partial() {
        let i: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::partial(42, ConvergenceLoss::new(3));
        assert_eq!(i, Imperfect::Partial(42, ConvergenceLoss::new(3)));
        assert!(i.is_partial());
        assert_eq!(i.clone().ok(), Some(42));
        assert_eq!(i.loss().steps(), 3);
    }

    #[test]
    fn constructor_failure() {
        let i: Imperfect<u32, String, ConvergenceLoss> = Imperfect::failure("gone".into());
        assert_eq!(
            i,
            Imperfect::Failure("gone".into(), ConvergenceLoss::new(0))
        );
        assert!(i.is_err());
        assert_eq!(i.loss().steps(), 0);
    }

    #[test]
    fn constructor_failure_with_loss() {
        let i: Imperfect<u32, String, ConvergenceLoss> =
            Imperfect::failure_with_loss("gone".into(), ConvergenceLoss::new(5));
        assert_eq!(
            i,
            Imperfect::Failure("gone".into(), ConvergenceLoss::new(5))
        );
        assert!(i.is_err());
        assert_eq!(i.loss().steps(), 5);
    }

    // --- IntoEh trait ---

    #[test]
    fn into_eh_imperfect_success() {
        let mut ctx: Eh<ConvergenceLoss> = Eh::new();
        let imp: Imperfect<i32, String, ConvergenceLoss> = Imperfect::Success(42);
        let result = imp.into_eh(&mut ctx);
        assert_eq!(result, Ok(42));
        assert!(ctx.loss().is_none());
    }

    #[test]
    fn into_eh_imperfect_partial_accumulates_loss() {
        let mut ctx: Eh<ConvergenceLoss> = Eh::new();
        let imp: Imperfect<i32, String, ConvergenceLoss> =
            Imperfect::Partial(42, ConvergenceLoss::new(3));
        let result = imp.into_eh(&mut ctx);
        assert_eq!(result, Ok(42));
        assert_eq!(ctx.loss().unwrap().steps(), 3);
    }

    #[test]
    fn into_eh_imperfect_failure() {
        let mut ctx: Eh<ConvergenceLoss> = Eh::new();
        let imp: Imperfect<i32, String, ConvergenceLoss> =
            Imperfect::Failure("boom".into(), ConvergenceLoss::new(2));
        let result = imp.into_eh(&mut ctx);
        assert_eq!(result, Err("boom".to_string()));
        assert_eq!(ctx.loss().unwrap().steps(), 2);
    }

    #[test]
    fn into_eh_result_ok_passes_through() {
        let mut ctx: Eh<ConvergenceLoss> = Eh::new();
        let r: Result<i32, String> = Ok(42);
        let result = IntoEh::<i32, String, ConvergenceLoss>::into_eh(r, &mut ctx);
        assert_eq!(result, Ok(42));
        assert!(ctx.loss().is_none());
    }

    #[test]
    fn into_eh_result_err_passes_through() {
        let mut ctx: Eh<ConvergenceLoss> = Eh::new();
        let r: Result<i32, String> = Err("fail".into());
        let result = IntoEh::<i32, String, ConvergenceLoss>::into_eh(r, &mut ctx);
        assert_eq!(result, Err("fail".to_string()));
        assert!(ctx.loss().is_none());
    }

    #[test]
    fn into_eh_result_does_not_accumulate_loss() {
        let mut ctx: Eh<ConvergenceLoss> = Eh::new();
        // First accumulate some loss from an Imperfect
        let imp: Imperfect<i32, String, ConvergenceLoss> =
            Imperfect::Partial(1, ConvergenceLoss::new(5));
        let _ = imp.into_eh(&mut ctx);
        // Then pass a Result through — loss should not change
        let r: Result<i32, String> = Ok(2);
        let result = IntoEh::<i32, String, ConvergenceLoss>::into_eh(r, &mut ctx);
        assert_eq!(result, Ok(2));
        assert_eq!(ctx.loss().unwrap().steps(), 5);
    }

    // --- Eh::into_loss ---

    #[test]
    fn eh_into_loss_none_when_no_loss() {
        let ctx: Eh<ConvergenceLoss> = Eh::new();
        assert!(ctx.into_loss().is_none());
    }

    #[test]
    fn eh_into_loss_some_when_loss_accumulated() {
        let mut ctx: Eh<ConvergenceLoss> = Eh::new();
        let _ = ctx.eh(Imperfect::<i32, String, ConvergenceLoss>::Partial(
            1,
            ConvergenceLoss::new(7),
        ));
        let loss = ctx.into_loss();
        assert!(loss.is_some());
        assert_eq!(loss.unwrap().steps(), 7);
    }
}
