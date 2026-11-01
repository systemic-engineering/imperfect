# Precedence

Which outcome is visible when several results are collected. The order is a parameter you sign, and terni has no default.

## Why the bind can't do it

`.eh()` and `eh!` are sequential and short-circuit. After a `Failure`, nothing later runs:

```rust
use terni::Imperfect;

let dark: Imperfect<i32, &str, Vec<&str>> = Imperfect::Failure("dark", vec!["dark"]);
let result = dark.eh(|_| Imperfect::Partial(7, vec!["twist"]));

// The later Partial was never seen. Its twist is gone.
assert_eq!(result, Imperfect::Failure("dark", vec!["dark"]));
```

That is right for a bind, because there is no value to feed the next step. It is wrong when the results already exist and the question is *which one to show*. If a later part carries something the caller knows about (a known twist), the bind hides it behind the first darkness. `tests/swallowing.rs` pins this behaviour down. `eh!` has not changed.

## `join_with`

```rust,ignore
pub trait Precedence<E, L: Loss> {
    fn known_twist(&self, loss: &L) -> bool;
    fn outranks(&self, darkness: &E) -> Outranks;
}

pub enum Outranks { Twist, Darkness }

pub fn join_with<T, E, L: Loss, P: Precedence<E, L> + ?Sized>(
    precedence: &P,
    parts: impl IntoIterator<Item = Imperfect<T, E, L>>,
) -> Imperfect<Vec<T>, E, L>;
```

`join_with` reads every part. The rules:

| Parts | Result |
|---|---|
| all `Success` (or none) | `Success(values)` |
| no `Failure`, at least one `Partial` | `Partial(values, loss)`, sticky even with zero loss |
| a `Failure`, and no `Partial` the order marks as a known twist | `Failure(first error, loss)`. The order is not asked. |
| a `Failure` and a known twist | the first `Failure` the order ranks `Outranks::Darkness` shows as `Failure(error, loss)`. If the twist outranks every darkness, the result is `Partial(values, loss)`. |

`values` are the values of the `Success` and `Partial` parts, in part order. `loss` is always the `Loss::combine` of every part's loss, in part order.

## Two signed orders, two answers

```rust
use terni::{join_with, Imperfect, Outranks, Precedence};

type Marks = Vec<&'static str>;

struct TwistFirst;
impl Precedence<&'static str, Marks> for TwistFirst {
    fn known_twist(&self, loss: &Marks) -> bool { loss.contains(&"twist") }
    fn outranks(&self, _: &&'static str) -> Outranks { Outranks::Twist }
}

struct DarknessFirst;
impl Precedence<&'static str, Marks> for DarknessFirst {
    fn known_twist(&self, loss: &Marks) -> bool { loss.contains(&"twist") }
    fn outranks(&self, _: &&'static str) -> Outranks { Outranks::Darkness }
}

let parts = || -> Vec<Imperfect<i32, &'static str, Marks>> {
    vec![Imperfect::Failure("dark", vec!["dark"]), Imperfect::Partial(7, vec!["twist"])]
};

assert_eq!(join_with(&TwistFirst, parts()), Imperfect::Partial(vec![7], vec!["dark", "twist"]));
assert_eq!(join_with(&DarknessFirst, parts()), Imperfect::Failure("dark", vec!["dark", "twist"]));
```

The two results show different things and keep the same loss.

## Laws

1. **Precedence decides visibility, never what's kept.** Under any order, the combined loss is the `combine` of every part's loss, in part order. A `Success` contributes nothing.
2. **`Success` only if every part is a `Success`.** Joining never turns a `Partial` or a `Failure` into a `Success`.
3. **A join of one part is that part**, with its value in a one-element `Vec`.
4. **Splitting a join keeps the loss.** `join(a ++ b).loss() == join(a).loss().combine(join(b).loss())`, because `combine` is associative.
5. **No default.** terni ships no `Precedence` implementation, no `Default` for `Outranks`, and no blanket impl. Collecting without a signed order does not compile. Two `compile_fail` doctests in `src/precedence.rs` prove it, with a control next to them.

## What is not kept

Payloads that aren't visible aren't loss. Under a visible `Failure`, the parts' values are dropped, the same as after `.eh()`. Under a visible twist, the errors are dropped. Each part's *loss* is always kept. If an error must survive under a twist, carry it in the loss (for example, record the darkness in `L` when the `Failure` is built).

## Where the order lives

terni carries the parameter. It doesn't choose the order. The order belongs to the scope that signs it: a crate that wants "a known twist outranks received darkness" writes that `Precedence`, names it, and passes it explicitly at every join.
