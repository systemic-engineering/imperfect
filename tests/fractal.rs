//! The `fractal` feature: darknesses compose by union of content addresses,
//! the join keeps every part as data, and what shows is a projection under
//! the caller's signed kind order.
//!
//! Law (mirror-015, signed shape plus Mara's correction): the error side is
//! the free bounded join-semilattice of content-addressed darknesses, with
//! the empty set as identity. `zoom(join(A, B)) = A ∪ B`.
#![cfg(feature = "fractal")]

use std::collections::BTreeSet;

use terni::fractal::{darkness_laws, join, Darkness, Darknesses, Joined, KindOrder};
use terni::Imperfect;

// --- Darkness impls under test -------------------------------------------

/// A located hole: the location is part of the address.
#[derive(Clone, Debug, PartialEq)]
struct Hole {
    location: &'static str,
    kind: Kind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Received,
    Paradox,
}

impl Darkness for Hole {
    type Address = (&'static str, Kind);
    fn address(&self) -> Self::Address {
        (self.location, self.kind)
    }
}

fn hole(location: &'static str, kind: Kind) -> Hole {
    Hole { location, kind }
}

/// A darkness addressed by an integer id (a stand-in for a content hash).
#[derive(Clone, Debug, PartialEq)]
struct Hashed(u64);

impl Darkness for Hashed {
    type Address = u64;
    fn address(&self) -> u64 {
        self.0
    }
}

/// A darkness whose content is a string, addressed by the string itself.
#[derive(Clone, Debug, PartialEq)]
struct Named(String);

impl Darkness for Named {
    type Address = String;
    fn address(&self) -> String {
        self.0.clone()
    }
}

/// Not lawful: the address leaves the location out, so two distinct holes
/// of one kind read as one event.
#[derive(Clone, Debug, PartialEq)]
struct KindOnly(Hole);

impl Darkness for KindOnly {
    type Address = Kind;
    fn address(&self) -> Kind {
        self.0.kind
    }
}

// --- Signed orders (the caller's; terni ships none) ----------------------

/// Paradox above received darkness.
struct ParadoxFirst;
impl KindOrder<Hole> for ParadoxFirst {
    type Kind = Kind;
    fn kind(&self, d: &Hole) -> Kind {
        d.kind
    }
}

/// The reverse: received darkness above paradox.
struct ReceivedFirst;
impl KindOrder<Hole> for ReceivedFirst {
    type Kind = std::cmp::Reverse<Kind>;
    fn kind(&self, d: &Hole) -> Self::Kind {
        std::cmp::Reverse(d.kind)
    }
}

struct KindOnlyOrder;
impl KindOrder<KindOnly> for KindOnlyOrder {
    type Kind = Kind;
    fn kind(&self, d: &KindOnly) -> Kind {
        d.0.kind
    }
}

/// Odd ids outrank even ones.
struct OddFirst;
impl KindOrder<Hashed> for OddFirst {
    type Kind = bool;
    fn kind(&self, d: &Hashed) -> bool {
        d.0 % 2 == 1
    }
}

/// Longer names outrank shorter ones.
struct Longest;
impl KindOrder<Named> for Longest {
    type Kind = usize;
    fn kind(&self, d: &Named) -> usize {
        d.0.len()
    }
}

fn holes() -> Vec<Hole> {
    vec![
        hole("a", Kind::Received),
        hole("b", Kind::Received),
        hole("a", Kind::Paradox),
        hole("c", Kind::Paradox),
    ]
}

fn set<E: Darkness>(items: impl IntoIterator<Item = E>) -> Darknesses<E> {
    items.into_iter().collect()
}

// --- The darkness algebra -------------------------------------------------

#[test]
fn round_trip_zoom_of_join_is_the_union() {
    let a = set([hole("a", Kind::Received), hole("b", Kind::Received)]);
    let b = set([hole("b", Kind::Received), hole("c", Kind::Paradox)]);
    let joined = a.clone().join(b.clone());

    let union: BTreeSet<_> = a.iter().chain(b.iter()).map(|d| d.address()).collect();
    let got: BTreeSet<_> = joined.iter().map(|d| d.address()).collect();
    assert_eq!(got, union);
    assert_eq!(joined.count(), 3);

    // Every darkness is recovered by its address.
    for d in a.iter().chain(b.iter()) {
        assert_eq!(joined.get(&d.address()), Some(d));
    }
    assert_eq!(joined.get(&("z", Kind::Paradox)), None);
}

#[test]
fn union_is_commutative_associative_idempotent_with_empty_identity() {
    let a = set([hole("a", Kind::Received)]);
    let b = set([hole("b", Kind::Received), hole("a", Kind::Paradox)]);
    let c = set([hole("c", Kind::Paradox), hole("a", Kind::Received)]);
    let empty = Darknesses::<Hole>::empty();

    assert_eq!(a.clone().join(b.clone()), b.clone().join(a.clone()));
    assert_eq!(
        a.clone().join(b.clone()).join(c.clone()),
        a.clone().join(b.clone().join(c.clone()))
    );
    assert_eq!(b.clone().join(b.clone()), b);
    assert_eq!(empty.clone().join(c.clone()), c);
    assert_eq!(c.clone().join(empty.clone()), c);
    assert!(empty.is_empty());
    assert_eq!(Darknesses::<Hole>::default(), empty);
}

#[test]
fn same_kind_different_location_are_two_events() {
    let joined =
        Darknesses::of(hole("a", Kind::Received)).join(Darknesses::of(hole("b", Kind::Received)));
    assert_eq!(joined.count(), 2);
}

#[test]
fn the_law_helper_accepts_lawful_impls() {
    assert_eq!(darkness_laws(&holes(), &ParadoxFirst), Ok(()));
    assert_eq!(darkness_laws(&holes(), &ReceivedFirst), Ok(()));
    assert_eq!(
        darkness_laws(&[Hashed(1), Hashed(2), Hashed(3), Hashed(8)], &OddFirst),
        Ok(())
    );
    assert_eq!(
        darkness_laws(
            &[
                Named("x".into()),
                Named("yy".into()),
                Named("zz".into()),
                Named(String::new())
            ],
            &Longest
        ),
        Ok(())
    );
    assert_eq!(darkness_laws::<Hashed, OddFirst>(&[], &OddFirst), Ok(()));
}

#[test]
fn the_law_helper_refuses_an_address_without_location() {
    let samples: Vec<KindOnly> = holes().into_iter().map(KindOnly).collect();
    assert_eq!(
        darkness_laws(&samples, &KindOnlyOrder),
        Err("address: equal addresses must mean equal darkness")
    );
}

#[test]
fn the_darkness_counter_never_falls_across_joins() {
    let samples = holes();
    let mut acc = Darknesses::<Hole>::empty();
    let mut last = 0;
    for d in samples.iter().chain(samples.iter()) {
        acc = acc.join(Darknesses::of(d.clone()));
        assert!(acc.count() >= last);
        last = acc.count();
    }
    assert_eq!(last, 4);
}

// --- The fractal join: nothing hidden is dropped -------------------------

type Marks = Vec<&'static str>;
type Part = Imperfect<i32, Hole, Marks>;

fn mixed() -> Vec<Part> {
    vec![
        Imperfect::Success(1),
        Imperfect::Partial(2, vec!["l"]),
        Imperfect::Failure(hole("e1", Kind::Received), vec![]),
        Imperfect::Failure(hole("e2", Kind::Paradox), vec![]),
    ]
}

#[test]
fn join_keeps_every_value_every_darkness_and_the_loss() {
    let joined = join(mixed());

    assert_eq!(
        joined.values(),
        &[Imperfect::Success(1), Imperfect::Partial(2, vec!["l"])]
    );
    assert_eq!(joined.zoom().count(), 2);
    assert_eq!(
        joined.zoom().get(&("e1", Kind::Received)),
        Some(&hole("e1", Kind::Received))
    );
    assert_eq!(
        joined.zoom().get(&("e2", Kind::Paradox)),
        Some(&hole("e2", Kind::Paradox))
    );
    assert_eq!(joined.loss(), &vec!["l"]);
    assert_eq!(joined.darkness_count(), 2);
}

#[test]
fn view_under_two_orders_shows_different_things_over_identical_data() {
    let joined = join(mixed());
    let kept = joined.clone();

    let paradox = joined.view(&ParadoxFirst);
    let received = joined.view(&ReceivedFirst);

    assert_eq!(
        paradox,
        Imperfect::Failure(vec![&hole("e2", Kind::Paradox)], vec!["l"])
    );
    assert_eq!(
        received,
        Imperfect::Failure(vec![&hole("e1", Kind::Received)], vec!["l"])
    );
    assert_ne!(paradox, received);
    // The view is a projection: the kept data did not move.
    assert_eq!(joined, kept);
}

#[test]
fn view_without_darkness_shows_the_values() {
    let clean: Joined<i32, Hole, Marks> = join(vec![Imperfect::Success(1), Imperfect::Success(2)]);
    assert_eq!(clean.view(&ParadoxFirst), Imperfect::Success(vec![&1, &2]));

    let partial: Joined<i32, Hole, Marks> =
        join(vec![Imperfect::Success(1), Imperfect::Partial(2, vec![])]);
    // Partial is sticky, even with zero loss.
    assert_eq!(
        partial.view(&ParadoxFirst),
        Imperfect::Partial(vec![&1, &2], vec![])
    );

    let nothing: Joined<i32, Hole, Marks> = Joined::empty();
    assert_eq!(nothing.view(&ParadoxFirst), Imperfect::Success(vec![]));
}

#[test]
fn view_shows_every_member_of_the_top_kind() {
    let joined: Joined<i32, Hole, Marks> = join(vec![
        Imperfect::Failure(hole("c", Kind::Paradox), vec![]),
        Imperfect::Failure(hole("b", Kind::Received), vec![]),
        Imperfect::Failure(hole("a", Kind::Paradox), vec![]),
    ]);
    assert_eq!(
        joined.view(&ParadoxFirst),
        Imperfect::Failure(
            vec![&hole("a", Kind::Paradox), &hole("c", Kind::Paradox)],
            vec![]
        )
    );
}

#[test]
fn joined_join_is_associative_and_join_of_parts_is_the_fold() {
    let parts = mixed();
    let one = |p: &Part| Joined::of(p.clone());
    let left = one(&parts[0])
        .join(one(&parts[1]))
        .join(one(&parts[2]).join(one(&parts[3])));
    let right = one(&parts[0]).join(one(&parts[1]).join(one(&parts[2])).join(one(&parts[3])));
    assert_eq!(left, right);
    assert_eq!(left, join(parts));
}

#[test]
fn into_parts_returns_everything_kept() {
    let (values, dark, loss) = join(mixed()).into_parts();
    assert_eq!(values.len(), 2);
    assert_eq!(dark.count(), 2);
    assert_eq!(loss, vec!["l"]);
}
