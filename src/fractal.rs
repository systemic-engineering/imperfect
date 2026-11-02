//! Fractal composition: several results joined without losing any of them.
//!
//! Behind the `fractal` cargo feature, off by default. The feature is
//! additive only: it adds the items in this module and changes nothing that
//! exists ([`join_with`](crate::join_with), `eh!`, `.eh()` behave the same
//! with it on), because features unify across a dependency graph.
//!
//! # The law
//!
//! A darkness (an error) names itself by a content address, through
//! [`Darkness::address`]. Darknesses compose by **union**: the free bounded
//! join-semilattice on addresses, with the empty set as identity.
//!
//! - **Associative, commutative, idempotent, `∅` the identity.**
//! - **Lossless because of the addresses.** Equal address means one event,
//!   so a duplicate adds nothing (idempotence). The order a join happened
//!   in is the evaluation schedule, not content, so it is not kept
//!   (commutativity). An address that leaves the location out merges two
//!   distinct holes; [`darkness_laws`] refuses such an impl.
//! - **No absorbing element.** "First error wins" is the left-zero band and
//!   drops the rest by construction; it is not this.
//! - **Round trip:** `zoom(join(A, B)) = A ∪ B`, and every darkness is
//!   recovered by its address ([`Darknesses::get`]).
//!
//! terni does not know how an address is computed. A caller's darkness
//! provides it.
//!
//! # What shows is a projection
//!
//! [`join`] keeps every part as data: the values of the `Success` and
//! `Partial` parts (each with its own grade and loss), the union of the
//! darknesses, and the combined loss. [`Joined::view`] is a pure function of
//! what was kept: under a caller's signed [`KindOrder`] it shows the
//! darknesses of the greatest kind, so `shows(A ∪ B) = max(shows A, shows B)`.
//! The union has no parameter, so it is law; the order belongs to the caller.
//! terni ships no order, no default and no blanket impl.
//!
//! **L measures, E records.** The combined loss may be a lossy measure; the
//! darknesses and the joined structure are recoverable.
//!
//! ```rust
//! use terni::fractal::{join, Darkness, KindOrder};
//! use terni::Imperfect;
//!
//! #[derive(Clone, Debug, PartialEq)]
//! struct Hole { at: &'static str, deep: bool }
//!
//! impl Darkness for Hole {
//!     type Address = (&'static str, bool);
//!     fn address(&self) -> Self::Address { (self.at, self.deep) }
//! }
//!
//! /// A signed order: deep darkness shows first.
//! struct DeepFirst;
//! impl KindOrder<Hole> for DeepFirst {
//!     type Kind = bool;
//!     fn kind(&self, d: &Hole) -> bool { d.deep }
//! }
//!
//! let joined = join(vec![
//!     Imperfect::<i32, Hole, Vec<&str>>::Success(1),
//!     Imperfect::Failure(Hole { at: "x", deep: false }, vec![]),
//!     Imperfect::Failure(Hole { at: "y", deep: true }, vec![]),
//! ]);
//!
//! // Everything is kept.
//! assert_eq!(joined.values(), &[Imperfect::Success(1)]);
//! assert_eq!(joined.darkness_count(), 2);
//! // What shows is the caller's projection.
//! assert_eq!(
//!     joined.view(&DeepFirst),
//!     Imperfect::Failure(vec![&Hole { at: "y", deep: true }], vec![]),
//! );
//! ```
//!
//! A view without a signed order does not compile:
//!
//! ```compile_fail,E0277
//! use terni::fractal::{join, Darkness};
//! use terni::Imperfect;
//!
//! #[derive(Clone, Debug, PartialEq)]
//! struct Hole(u8);
//! impl Darkness for Hole {
//!     type Address = u8;
//!     fn address(&self) -> u8 { self.0 }
//! }
//!
//! let joined = join(vec![Imperfect::<i32, Hole, Vec<u8>>::Failure(Hole(1), vec![])]);
//! let _ = joined.view(&());
//! ```

use std::convert::Infallible;

use crate::{Imperfect, Loss};

pub mod mandelbrot;

/// A membership verdict that names its own arms.
///
/// `In` and `Out` are both decided: each is a theorem with its witness.
/// `Undecided` is a fact about the observer (a budget ran out, or only a
/// ruler's verdict such as a float or grain-level cycle was seen), never a
/// claim about the set.
///
/// The map into [`Imperfect`] is deliberate, see
/// [`into_imperfect`](Self::into_imperfect): an `Out` is a value, never a
/// darkness.
#[derive(Clone, Debug, PartialEq)]
pub enum Membership<I, O, U> {
    /// Proved inside, with its certificate.
    In(I),
    /// Proved outside, with its witness.
    Out(O),
    /// No certificate found yet.
    Undecided(U),
}

impl<I, O, U> Membership<I, O, U> {
    /// Whether this verdict is `In` or `Out`.
    pub fn is_decided(&self) -> bool {
        !matches!(self, Membership::Undecided(_))
    }

    /// The deliberate map: `In` and `Out` to `Success`, `Undecided` to
    /// `Partial` with the loss `measure` reads from it. `measure` is called
    /// only for `Undecided`. Nonsense input never reaches a `Membership`;
    /// an instance refuses it as `Failure` itself.
    pub fn into_imperfect<E, L: Loss>(
        self,
        measure: impl FnOnce(&U) -> L,
    ) -> Imperfect<Self, E, L> {
        match self {
            Membership::Undecided(u) => {
                let loss = measure(&u);
                Imperfect::Partial(Membership::Undecided(u), loss)
            }
            decided => Imperfect::Success(decided),
        }
    }
}

/// An error that names itself by a content address.
///
/// **Contract:** the address is a content address, location included. Two
/// darknesses with equal addresses are the same event, and so are equal.
/// [`darkness_laws`] checks this on samples.
pub trait Darkness {
    /// The address. Its order is the order darknesses are kept in.
    type Address: Ord;

    /// This darkness's content address.
    fn address(&self) -> Self::Address;
}

/// A signed order on darkness kinds, supplied by the caller.
///
/// terni ships no implementation, no blanket impl and no default. What
/// shows is the set of darknesses whose kind is greatest.
pub trait KindOrder<E> {
    /// The kind a darkness is ranked by. Greater shows first.
    type Kind: Ord;

    /// The kind of this darkness.
    fn kind(&self, darkness: &E) -> Self::Kind;
}

/// A finite set of darknesses under union, kept in address order.
///
/// The only constructors are [`empty`](Self::empty), [`of`](Self::of),
/// [`join`](Self::join) and `collect`, so a value of this type always holds
/// at most one darkness per address.
#[derive(Clone, Debug, PartialEq)]
pub struct Darknesses<E> {
    sorted: Vec<E>,
}

impl<E> Default for Darknesses<E> {
    fn default() -> Self {
        Darknesses { sorted: Vec::new() }
    }
}

impl<E: Darkness> Darknesses<E> {
    /// The empty set: the identity of [`join`](Self::join).
    pub fn empty() -> Self {
        Self::default()
    }

    /// The set holding one darkness.
    pub fn of(darkness: E) -> Self {
        Darknesses {
            sorted: vec![darkness],
        }
    }

    /// Union. Associative, commutative, idempotent; `empty()` is the
    /// identity. A darkness whose address is already present adds nothing.
    pub fn join(mut self, other: Self) -> Self {
        for darkness in other.sorted {
            self.insert(darkness);
        }
        self
    }

    fn insert(&mut self, darkness: E) {
        let address = darkness.address();
        if let Err(at) = self
            .sorted
            .binary_search_by(|kept| kept.address().cmp(&address))
        {
            self.sorted.insert(at, darkness);
        }
    }

    /// The darkness at this address, if it is in the set.
    pub fn get(&self, address: &E::Address) -> Option<&E> {
        self.sorted
            .binary_search_by(|kept| kept.address().cmp(address))
            .ok()
            .map(|at| &self.sorted[at])
    }

    /// The darkness counter: the number of distinct addresses. It never
    /// falls across a join.
    pub fn count(&self) -> usize {
        self.sorted.len()
    }

    /// Whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.sorted.is_empty()
    }

    /// Every darkness, in address order.
    pub fn iter(&self) -> std::slice::Iter<'_, E> {
        self.sorted.iter()
    }

    /// What shows under a signed order: every darkness of the greatest
    /// kind, in address order. Empty for the empty set.
    pub fn shows<O: KindOrder<E> + ?Sized>(&self, order: &O) -> Vec<&E> {
        let top = self.sorted.iter().map(|d| order.kind(d)).max();
        match top {
            None => Vec::new(),
            Some(top) => self
                .sorted
                .iter()
                .filter(|d| order.kind(d) == top)
                .collect(),
        }
    }
}

impl<E: Darkness> FromIterator<E> for Darknesses<E> {
    fn from_iter<I: IntoIterator<Item = E>>(iter: I) -> Self {
        let mut set = Self::empty();
        for darkness in iter {
            set.insert(darkness);
        }
        set
    }
}

/// Several results joined, with every part kept as data.
///
/// Built only by [`join`], [`Joined::of`], [`Joined::empty`] and
/// [`Joined::join`].
#[derive(Clone, Debug, PartialEq)]
pub struct Joined<T, E, L: Loss> {
    values: Vec<Imperfect<T, Infallible, L>>,
    darkness: Darknesses<E>,
    loss: L,
}

impl<T, E: Darkness, L: Loss> Joined<T, E, L> {
    /// The join of no parts: no values, no darkness, zero loss.
    pub fn empty() -> Self {
        Joined {
            values: Vec::new(),
            darkness: Darknesses::empty(),
            loss: L::zero(),
        }
    }

    /// The join of one part.
    pub fn of(part: Imperfect<T, E, L>) -> Self {
        match part {
            Imperfect::Success(v) => Joined {
                values: vec![Imperfect::Success(v)],
                darkness: Darknesses::empty(),
                loss: L::zero(),
            },
            Imperfect::Partial(v, l) => Joined {
                values: vec![Imperfect::Partial(v, l.clone())],
                darkness: Darknesses::empty(),
                loss: l,
            },
            Imperfect::Failure(e, l) => Joined {
                values: Vec::new(),
                darkness: Darknesses::of(e),
                loss: l,
            },
        }
    }

    /// Join two joins. Associative: values concatenate in part order,
    /// darknesses unite, losses [`combine`](Loss::combine).
    pub fn join(mut self, other: Self) -> Self {
        self.values.extend(other.values);
        Joined {
            values: self.values,
            darkness: self.darkness.join(other.darkness),
            loss: self.loss.combine(other.loss),
        }
    }

    /// The values of the `Success` and `Partial` parts, in part order, each
    /// with its own grade and loss.
    pub fn values(&self) -> &[Imperfect<T, Infallible, L>] {
        &self.values
    }

    /// Zoom into the darkness: the union of every part's darkness.
    pub fn zoom(&self) -> &Darknesses<E> {
        &self.darkness
    }

    /// The combined loss of every part, in part order. A measure.
    pub fn loss(&self) -> &L {
        &self.loss
    }

    /// The number of distinct darkness addresses.
    pub fn darkness_count(&self) -> usize {
        self.darkness.count()
    }

    /// What shows, under the caller's signed order. A pure function of the
    /// kept parts; nothing is removed.
    ///
    /// - darkness present: `Failure(every darkness of the greatest kind, loss)`;
    /// - otherwise, any `Partial` value: `Partial(values, loss)` (sticky,
    ///   even with zero loss);
    /// - otherwise `Success(values)`.
    pub fn view<O: KindOrder<E> + ?Sized>(&self, order: &O) -> Imperfect<Vec<&T>, Vec<&E>, L> {
        if !self.darkness.is_empty() {
            return Imperfect::Failure(self.darkness.shows(order), self.loss.clone());
        }
        let values: Vec<&T> = self
            .values
            .iter()
            .map(|v| match v {
                Imperfect::Success(t) | Imperfect::Partial(t, _) => t,
                Imperfect::Failure(never, _) => match *never {},
            })
            .collect();
        if self.values.iter().any(Imperfect::is_partial) {
            Imperfect::Partial(values, self.loss.clone())
        } else {
            Imperfect::Success(values)
        }
    }

    /// Everything kept: the values, the darknesses, the combined loss.
    pub fn into_parts(self) -> (Vec<Imperfect<T, Infallible, L>>, Darknesses<E>, L) {
        (self.values, self.darkness, self.loss)
    }
}

/// Join several results without short-circuiting and without dropping any.
///
/// The fold of [`Joined::of`] under [`Joined::join`].
pub fn join<T, E: Darkness, L: Loss>(
    parts: impl IntoIterator<Item = Imperfect<T, E, L>>,
) -> Joined<T, E, L> {
    parts
        .into_iter()
        .fold(Joined::empty(), |acc, part| acc.join(Joined::of(part)))
}

fn law(holds: bool, name: &'static str) -> Result<(), &'static str> {
    if holds {
        Ok(())
    } else {
        Err(name)
    }
}

/// The law every [`Darkness`] impl must pass, checked over samples.
///
/// Returns the name of the first law that breaks:
///
/// - **address:** equal addresses mean equal darkness (the location is in
///   the address);
/// - **round trip:** `zoom(join(A, B)) = A ∪ B`, each recovered by address;
/// - **commutative**, **associative**, **idempotent**, **identity**;
/// - **counter:** the number of distinct addresses never falls;
/// - **view:** what shows depends only on the parts, not on their order or
///   grouping, and `shows(A ∪ B) = max(shows A, shows B)`.
pub fn darkness_laws<E, O>(samples: &[E], order: &O) -> Result<(), &'static str>
where
    E: Darkness + Clone + PartialEq,
    O: KindOrder<E> + ?Sized,
{
    for a in samples {
        for b in samples {
            law(
                a.address() != b.address() || a == b,
                "address: equal addresses must mean equal darkness",
            )?;
        }
    }

    let empty = Darknesses::<E>::empty();
    // Prefixes and suffixes give overlapping and disjoint sets.
    let mut sets: Vec<Darknesses<E>> = Vec::new();
    for i in 0..=samples.len() {
        sets.push(samples[..i].iter().cloned().collect());
        sets.push(samples[i..].iter().cloned().collect());
    }

    for a in &sets {
        law(a.clone().join(a.clone()) == *a, "idempotent")?;
        law(empty.clone().join(a.clone()) == *a, "identity")?;
        law(a.clone().join(empty.clone()) == *a, "identity")?;
        for b in &sets {
            let ab = a.clone().join(b.clone());
            law(ab == b.clone().join(a.clone()), "commutative")?;
            law(
                a.iter()
                    .chain(b.iter())
                    .all(|d| ab.get(&d.address()) == Some(d))
                    && ab
                        .iter()
                        .all(|d| a.get(&d.address()).is_some() || b.get(&d.address()).is_some()),
                "round trip: zoom(join(A, B)) = A ∪ B",
            )?;
            law(ab.count() >= a.count().max(b.count()), "counter")?;

            let shown = ab.shows(order);
            let top = [a.shows(order), b.shows(order)]
                .into_iter()
                .flatten()
                .map(|d| order.kind(d))
                .max();
            law(
                shown.iter().map(|d| order.kind(d)).max() == top,
                "view: shows(A ∪ B) = max(shows A, shows B)",
            )?;

            for c in &sets {
                law(
                    ab.clone().join(c.clone()) == a.clone().join(b.clone().join(c.clone())),
                    "associative",
                )?;
            }
        }
    }

    // The view depends only on the parts: order and grouping do not move it.
    let fail = |e: &E| Imperfect::<(), E, u64>::Failure(e.clone(), 0);
    let forward = join(samples.iter().map(fail));
    let backward = join(samples.iter().rev().map(fail));
    let (left, right) = samples.split_at(samples.len() / 2);
    let grouped = join(left.iter().map(fail)).join(join(right.iter().map(fail)));
    law(
        forward == backward && forward == grouped,
        "view: the kept parts depend on order or grouping",
    )?;
    law(
        forward.view(order) == backward.view(order) && forward.view(order) == grouped.view(order),
        "view: depends only on the parts",
    )
}
