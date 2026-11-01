//! Precedence: which outcome is visible when results are collected.
//!
//! The bind (`.eh()`, `eh!`) is sequential and short-circuits: after a
//! `Failure` nothing later runs, so a later `Partial` whose loss carries a
//! *known twist* is never seen. That is the right behaviour for a bind. It
//! is the wrong one when several results already exist and the question is
//! which of them to show.
//!
//! [`join_with`] collects instead of short-circuiting. Every part is read,
//! every part's loss is kept, and a caller-supplied [`Precedence`] decides
//! one thing only: when a known twist (a `Partial` the caller marks) meets
//! darkness (a `Failure`), which one is visible.
//!
//! - **Precedence decides visibility, never what's kept.** The combined loss
//!   is the [`Loss::combine`] of every part's loss, in part order, under any
//!   precedence.
//! - **There is no default order.** terni ships no implementation of
//!   [`Precedence`], no `Default` for [`Outranks`], and no blanket impl. The
//!   order is a parameter the caller signs; terni cannot pick one.
//! - **terni stays ignorant of what a twist is.** The caller's
//!   [`Precedence::known_twist`] reads the loss and says whether it carries
//!   one. [`Precedence::outranks`] reads each error and says which side wins.
//!
//! ```rust
//! use terni::{join_with, Imperfect, Outranks, Precedence};
//!
//! type Marks = Vec<&'static str>;
//!
//! /// A signed order: a known twist outranks received darkness.
//! struct TwistFirst;
//!
//! impl Precedence<&'static str, Marks> for TwistFirst {
//!     fn known_twist(&self, loss: &Marks) -> bool {
//!         loss.contains(&"twist")
//!     }
//!     fn outranks(&self, _darkness: &&'static str) -> Outranks {
//!         Outranks::Twist
//!     }
//! }
//!
//! let parts: Vec<Imperfect<i32, &'static str, Marks>> = vec![
//!     Imperfect::Failure("dark", vec!["dark"]),
//!     Imperfect::Partial(7, vec!["twist"]),
//! ];
//!
//! // The twist shows, and the darkness is still in the loss.
//! let joined = join_with(&TwistFirst, parts);
//! assert_eq!(joined, Imperfect::Partial(vec![7], vec!["dark", "twist"]));
//! ```
//!
//! # No default
//!
//! Collecting without an order does not compile. There is no blanket
//! implementation that would make some type an order by accident:
//!
//! ```compile_fail,E0277
//! use terni::{join_with, Imperfect};
//!
//! let parts: Vec<Imperfect<i32, &'static str, Vec<&'static str>>> = vec![
//!     Imperfect::Failure("dark", vec!["dark"]),
//!     Imperfect::Partial(7, vec!["twist"]),
//! ];
//! let _ = join_with(&(), parts);
//! ```
//!
//! and there is no default side to fall back on:
//!
//! ```compile_fail,E0277
//! use terni::Outranks;
//!
//! let _side: Outranks = Default::default();
//! ```
//!
//! The control: the same call, with a signed order supplied, compiles.
//!
//! ```rust
//! use terni::{join_with, Imperfect, Outranks, Precedence};
//!
//! struct Signed;
//! impl Precedence<&'static str, Vec<&'static str>> for Signed {
//!     fn known_twist(&self, loss: &Vec<&'static str>) -> bool {
//!         loss.contains(&"twist")
//!     }
//!     fn outranks(&self, _darkness: &&'static str) -> Outranks {
//!         Outranks::Darkness
//!     }
//! }
//!
//! let parts: Vec<Imperfect<i32, &'static str, Vec<&'static str>>> = vec![
//!     Imperfect::Failure("dark", vec!["dark"]),
//!     Imperfect::Partial(7, vec!["twist"]),
//! ];
//! let _ = join_with(&Signed, parts);
//! let _side: Outranks = Outranks::Darkness;
//! ```

use crate::{Imperfect, Loss};

/// Which side is visible when a known twist meets darkness.
///
/// Deliberately has no `Default`: the side is always named by a signed
/// [`Precedence`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outranks {
    /// The known twist is visible: the result is `Partial` with the parts'
    /// values. The darkness stays in the combined loss.
    Twist,
    /// The darkness is visible: the result is `Failure` with this error. The
    /// twist stays in the combined loss.
    Darkness,
}

/// A signed order over terni's own shapes, supplied by the caller.
///
/// terni ships no implementation. Whoever composes results under an order
/// writes it, names it, and passes it to [`join_with`].
pub trait Precedence<E, L: Loss> {
    /// Does this `Partial` loss carry something the caller marks as a known
    /// twist?
    fn known_twist(&self, loss: &L) -> bool;

    /// A known twist is present and so is this darkness: which is visible?
    ///
    /// Asked of the `Failure` parts in part order, until one answers
    /// [`Outranks::Darkness`], and only when at least one `Partial` part
    /// carries a known twist.
    fn outranks(&self, darkness: &E) -> Outranks;
}

/// Collect several results under a signed [`Precedence`], without
/// short-circuiting.
///
/// Every part is read. The combined loss is the [`Loss::combine`] of every
/// part's loss in part order (a `Success` contributes nothing), whatever the
/// precedence says. What the precedence decides is the visible outcome:
///
/// - **No `Failure`:** `Success(values)` if every part is a `Success`,
///   otherwise `Partial(values, loss)`. `Partial` is sticky, even with zero
///   loss.
/// - **A `Failure`, and no known twist:** `Failure(first error, loss)`. The
///   precedence is not asked.
/// - **A `Failure` and a known twist:** the first `Failure` whose darkness
///   [`Outranks::Darkness`] is visible as `Failure(error, loss)`. If the
///   twist outranks every darkness, the result is `Partial(values, loss)`.
///
/// `values` are the values of the `Success` and `Partial` parts, in part
/// order. The payloads that are not visible (the values under a visible
/// `Failure`, the errors under a visible twist) are not loss and are not
/// kept; each part's loss is.
pub fn join_with<T, E, L, P>(
    precedence: &P,
    parts: impl IntoIterator<Item = Imperfect<T, E, L>>,
) -> Imperfect<Vec<T>, E, L>
where
    L: Loss,
    P: Precedence<E, L> + ?Sized,
{
    let mut values = Vec::new();
    let mut errors = Vec::new();
    let mut loss: Option<L> = None;
    let mut twist = false;

    for part in parts {
        let part_loss = match part {
            Imperfect::Success(v) => {
                values.push(v);
                continue;
            }
            Imperfect::Partial(v, l) => {
                twist = twist || precedence.known_twist(&l);
                values.push(v);
                l
            }
            Imperfect::Failure(e, l) => {
                // Deliberately broken: short-circuit like the bind.
                let l = match loss {
                    Some(acc) => acc.combine(l),
                    None => l,
                };
                return Imperfect::Failure(e, l);
            }
        };
        loss = Some(match loss {
            Some(acc) => acc.combine(part_loss),
            None => part_loss,
        });
    }

    let mut errors = errors.into_iter();
    let shown = if twist {
        errors.find(|e| precedence.outranks(e) == Outranks::Darkness)
    } else {
        errors.next()
    };

    match (shown, loss) {
        (Some(e), loss) => Imperfect::Failure(e, loss.unwrap_or_else(L::zero)),
        (None, Some(loss)) => Imperfect::Partial(values, loss),
        (None, None) => Imperfect::Success(values),
    }
}
