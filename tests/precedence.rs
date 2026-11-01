//! `join_with` under a signed `Precedence`: collects instead of
//! short-circuiting, and the caller's order decides what is visible.
//!
//! The swallowing it answers is characterized in `tests/swallowing.rs`.
//! The no-default proof is the `compile_fail` doctests in `src/precedence.rs`.

use std::cell::Cell;

use terni::{join_with, Imperfect, Loss, Outranks, Precedence};

type Marks = Vec<&'static str>;
type Part = Imperfect<i32, &'static str, Marks>;

fn dark() -> Part {
    Imperfect::Failure("dark", vec!["dark"])
}

fn twisted() -> Part {
    Imperfect::Partial(7, vec!["twist"])
}

fn plain() -> Part {
    Imperfect::Partial(3, vec!["plain"])
}

fn clean() -> Part {
    Imperfect::Success(1)
}

fn marks_twist(loss: &Marks) -> bool {
    loss.contains(&"twist")
}

/// Signed order: a known twist outranks received darkness.
struct TwistFirst;

impl Precedence<&'static str, Marks> for TwistFirst {
    fn known_twist(&self, loss: &Marks) -> bool {
        marks_twist(loss)
    }
    fn outranks(&self, _darkness: &&'static str) -> Outranks {
        Outranks::Twist
    }
}

/// A different signed order: received darkness outranks a known twist.
struct DarknessFirst;

impl Precedence<&'static str, Marks> for DarknessFirst {
    fn known_twist(&self, loss: &Marks) -> bool {
        marks_twist(loss)
    }
    fn outranks(&self, _darkness: &&'static str) -> Outranks {
        Outranks::Darkness
    }
}

/// An order that reads the error: only darkness named "deep" outranks a twist.
struct DeepDarknessFirst;

impl Precedence<&'static str, Marks> for DeepDarknessFirst {
    fn known_twist(&self, loss: &Marks) -> bool {
        marks_twist(loss)
    }
    fn outranks(&self, darkness: &&'static str) -> Outranks {
        if *darkness == "deep" {
            Outranks::Darkness
        } else {
            Outranks::Twist
        }
    }
}

// ---------------------------------------------------------------------------
// 2. Signed order: twist first.
// ---------------------------------------------------------------------------

#[test]
fn twist_first_shows_the_twist_after_darkness() {
    let joined = join_with(&TwistFirst, [dark(), twisted()]);
    assert_eq!(joined, Imperfect::Partial(vec![7], vec!["dark", "twist"]));
}

#[test]
fn twist_first_shows_the_twist_before_darkness() {
    let joined = join_with(&TwistFirst, [twisted(), dark()]);
    assert_eq!(joined, Imperfect::Partial(vec![7], vec!["twist", "dark"]));
}

#[test]
fn twist_first_keeps_the_darkness_in_the_loss() {
    for parts in [[dark(), twisted()], [twisted(), dark()]] {
        let joined = join_with(&TwistFirst, parts);
        assert!(joined.is_partial());
        assert!(joined.loss().contains(&"dark"), "darkness kept: {joined:?}");
        assert!(marks_twist(&joined.loss()), "twist visible: {joined:?}");
    }
}

// ---------------------------------------------------------------------------
// 3. A different signed order gives the other answer.
// ---------------------------------------------------------------------------

#[test]
fn darkness_first_shows_the_darkness_after_a_twist() {
    let joined = join_with(&DarknessFirst, [twisted(), dark()]);
    assert_eq!(joined, Imperfect::Failure("dark", vec!["twist", "dark"]));
}

#[test]
fn darkness_first_shows_the_darkness_before_a_twist() {
    let joined = join_with(&DarknessFirst, [dark(), twisted()]);
    assert_eq!(joined, Imperfect::Failure("dark", vec!["dark", "twist"]));
}

#[test]
fn the_order_is_the_callers_not_ternis() {
    let parts = || [dark(), twisted()];
    let twist_first = join_with(&TwistFirst, parts());
    let darkness_first = join_with(&DarknessFirst, parts());
    assert!(twist_first.is_partial());
    assert!(darkness_first.is_err());
    assert_eq!(twist_first.loss(), darkness_first.loss());
}

#[test]
fn an_order_may_read_the_darkness() {
    let deep: Part = Imperfect::Failure("deep", vec!["deep"]);
    let joined = join_with(&DeepDarknessFirst, [dark(), twisted(), deep]);
    assert_eq!(
        joined,
        Imperfect::Failure("deep", vec!["dark", "twist", "deep"])
    );

    let shallow = join_with(&DeepDarknessFirst, [dark(), twisted()]);
    assert_eq!(shallow, Imperfect::Partial(vec![7], vec!["dark", "twist"]));
}

// ---------------------------------------------------------------------------
// Outcomes where the precedence has nothing to decide.
// ---------------------------------------------------------------------------

/// Counts how often the order is asked.
struct Counting<'a>(&'a Cell<usize>);

impl Precedence<&'static str, Marks> for Counting<'_> {
    fn known_twist(&self, loss: &Marks) -> bool {
        marks_twist(loss)
    }
    fn outranks(&self, _darkness: &&'static str) -> Outranks {
        self.0.set(self.0.get() + 1);
        Outranks::Twist
    }
}

#[test]
fn without_a_twist_the_first_darkness_shows_and_the_order_is_not_asked() {
    let asked = Cell::new(0);
    let other: Part = Imperfect::Failure("other", vec!["other"]);
    let joined = join_with(&Counting(&asked), [plain(), dark(), other]);
    assert_eq!(
        joined,
        Imperfect::Failure("dark", vec!["plain", "dark", "other"])
    );
    assert_eq!(asked.get(), 0);
}

#[test]
fn twist_outranking_every_darkness_keeps_every_value() {
    let asked = Cell::new(0);
    let other: Part = Imperfect::Failure("other", vec!["other"]);
    let joined = join_with(
        &Counting(&asked),
        [clean(), dark(), twisted(), other, plain()],
    );
    assert_eq!(
        joined,
        Imperfect::Partial(vec![1, 7, 3], vec!["dark", "twist", "other", "plain"])
    );
    assert_eq!(asked.get(), 2, "asked once per darkness");
}

#[test]
fn all_success_is_success() {
    let joined = join_with(&TwistFirst, [clean(), clean()]);
    assert_eq!(joined, Imperfect::Success(vec![1, 1]));
}

#[test]
fn nothing_to_join_is_success() {
    let joined = join_with(&TwistFirst, Vec::<Part>::new());
    assert_eq!(joined, Imperfect::Success(vec![]));
}

#[test]
fn partial_is_sticky_even_with_zero_loss() {
    let unsettled: Part = Imperfect::Partial(2, vec![]);
    let joined = join_with(&DarknessFirst, [clean(), unsettled]);
    assert_eq!(joined, Imperfect::Partial(vec![1, 2], vec![]));
}

#[test]
fn a_twist_without_darkness_is_partial() {
    let joined = join_with(&DarknessFirst, [clean(), twisted()]);
    assert_eq!(joined, Imperfect::Partial(vec![1, 7], vec!["twist"]));
}

#[test]
fn a_dyn_precedence_is_accepted() {
    let order: &dyn Precedence<&'static str, Marks> = &TwistFirst;
    let joined = join_with(order, [dark(), twisted()]);
    assert!(joined.is_partial());
}

// ---------------------------------------------------------------------------
// 4. Loss laws: precedence decides visibility, never what's kept.
// ---------------------------------------------------------------------------

/// The `Loss` combination of every part's loss, in part order.
fn combined(parts: &[Part]) -> Marks {
    parts
        .iter()
        .fold(Marks::zero(), |acc, part| acc.combine(part.loss()))
}

/// Every sequence of up to three parts drawn from the five shapes.
fn sequences() -> Vec<Vec<Part>> {
    let shapes: [fn() -> Part; 5] = [clean, plain, twisted, dark, || {
        Imperfect::Failure("deep", vec!["deep"])
    }];
    let mut out: Vec<Vec<Part>> = vec![vec![]];
    for len in 1..=3 {
        let mut idx = vec![0usize; len];
        loop {
            out.push(idx.iter().map(|&i| shapes[i]()).collect());
            let mut k = 0;
            while k < len {
                idx[k] += 1;
                if idx[k] < shapes.len() {
                    break;
                }
                idx[k] = 0;
                k += 1;
            }
            if k == len {
                break;
            }
        }
    }
    out
}

fn law_combined_loss_is_every_parts_loss<P: Precedence<&'static str, Marks>>(order: &P) {
    for parts in sequences() {
        let expected = combined(&parts);
        let joined = join_with(order, parts.clone());
        assert_eq!(joined.loss(), expected, "parts: {parts:?}");
    }
}

#[test]
fn loss_law_twist_first() {
    law_combined_loss_is_every_parts_loss(&TwistFirst);
}

#[test]
fn loss_law_darkness_first() {
    law_combined_loss_is_every_parts_loss(&DarknessFirst);
}

#[test]
fn loss_law_deep_darkness_first() {
    law_combined_loss_is_every_parts_loss(&DeepDarknessFirst);
}

#[test]
fn loss_law_loss_is_independent_of_the_order() {
    for parts in sequences() {
        let a = join_with(&TwistFirst, parts.clone()).loss();
        let b = join_with(&DarknessFirst, parts.clone()).loss();
        let c = join_with(&DeepDarknessFirst, parts.clone()).loss();
        assert_eq!(a, b, "parts: {parts:?}");
        assert_eq!(b, c, "parts: {parts:?}");
    }
}

#[test]
fn loss_law_success_iff_every_part_is_success() {
    for parts in sequences() {
        let all_clean = parts.iter().all(|p| matches!(p, Imperfect::Success(_)));
        for joined in [
            join_with(&TwistFirst, parts.clone()),
            join_with(&DarknessFirst, parts.clone()),
        ] {
            assert_eq!(
                matches!(joined, Imperfect::Success(_)),
                all_clean,
                "parts: {parts:?}"
            );
        }
    }
}

#[test]
fn loss_law_identity_join_of_one_is_that_part() {
    for part in [clean(), plain(), twisted(), dark()] {
        let joined = join_with(&TwistFirst, [part.clone()]);
        assert_eq!(joined, part.map(|v| vec![v]));
    }
}

#[test]
fn loss_law_associative_on_count_loss() {
    // Joining in two halves and joining all at once keep the same loss,
    // because Loss::combine is associative.
    type Counted = Imperfect<i32, &'static str, u64>;
    struct Twist3;
    impl Precedence<&'static str, u64> for Twist3 {
        fn known_twist(&self, loss: &u64) -> bool {
            *loss == 3
        }
        fn outranks(&self, _darkness: &&'static str) -> Outranks {
            Outranks::Twist
        }
    }
    let parts = || -> Vec<Counted> {
        vec![
            Imperfect::Partial(1, 2),
            Imperfect::Failure("dark", 5),
            Imperfect::Partial(2, 3),
        ]
    };
    let whole = join_with(&Twist3, parts()).loss();
    let left = join_with(&Twist3, parts().into_iter().take(2)).loss();
    let right = join_with(&Twist3, parts().into_iter().skip(2)).loss();
    assert_eq!(whole, left.combine(right));
    assert_eq!(whole, 10);
}
