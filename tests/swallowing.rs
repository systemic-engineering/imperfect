//! Characterization: the bind short-circuits, so a `Failure` swallows a
//! later `Partial` that carries a known twist.
//!
//! This is today's behaviour of `.eh()` and `eh!`, and it is not a bug in
//! them: a bind is sequential, and after a `Failure` there is no value to
//! feed the next step. These tests pin it down so nobody reads the
//! precedence combinator (`terni::join_with`) as a change to `eh!`. When a
//! known twist must outrank received darkness, collect the parts with
//! `join_with` under a signed order instead of binding them.

use terni::{eh, Imperfect};

type Marks = Vec<&'static str>;

fn dark() -> Imperfect<i32, &'static str, Marks> {
    Imperfect::Failure("dark", vec!["dark"])
}

fn twisted() -> Imperfect<i32, &'static str, Marks> {
    Imperfect::Partial(7, vec!["twist"])
}

#[test]
fn bind_failure_swallows_a_later_twist() {
    let mut ran = false;
    let result = dark().eh(|_| {
        ran = true;
        twisted()
    });

    assert!(!ran, "the bind never runs the step after a Failure");
    assert_eq!(result, Imperfect::Failure("dark", vec!["dark"]));
    assert!(!result.loss().contains(&"twist"), "the twist is gone");
}

#[test]
fn eh_macro_failure_swallows_a_later_twist() {
    let result: Imperfect<i32, &'static str, Marks> = eh! {
        let _d = dark()?;
        let t = twisted()?;
        t
    };

    assert_eq!(result, Imperfect::Failure("dark", vec!["dark"]));
    assert!(!result.loss().contains(&"twist"), "the twist is gone");
}

#[test]
fn eh_macro_twist_before_failure_is_kept_as_loss_but_not_shown() {
    // The other order: the twist's loss is carried into the Failure, so it is
    // kept, but the visible outcome is still the darkness. `eh!` has one
    // order, and it is the order of the steps.
    let result: Imperfect<i32, &'static str, Marks> = eh! {
        let t = twisted()?;
        let _d = dark()?;
        t
    };

    assert_eq!(result, Imperfect::Failure("dark", vec!["twist", "dark"]));
}
