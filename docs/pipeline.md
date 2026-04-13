# Pipeline

The `.eh()` method is the terni-functor bind. It chains operations and accumulates loss through the middle state.

## The bind

```rust
pub fn eh<U>(self, f: impl FnOnce(T) -> Imperfect<U, E, L>) -> Imperfect<U, E, L>
```

Takes a function from `T` to `Imperfect<U, E, L>`. Returns a new `Imperfect<U, E, L>` with loss accumulated.

## How loss accumulates

Four rules. No exceptions.

### Success x Success = Success

No loss on either side. The pipeline is perfect.

```rust
use terni::{Imperfect, ConvergenceLoss};

let result = Imperfect::<i32, String, ConvergenceLoss>::Success(1)
    .eh(|x| Imperfect::Success(x + 1));

assert_eq!(result, Imperfect::Success(2));
```

### Success x Partial = Partial

The function introduced loss. It carries forward.

```rust
use terni::{Imperfect, ConvergenceLoss};

let result = Imperfect::<i32, String, ConvergenceLoss>::Success(1)
    .eh(|x| Imperfect::Partial(x + 1, ConvergenceLoss::new(3)));

assert!(result.is_partial());
assert_eq!(result.loss().steps(), 3);
```

### Partial x Partial = Partial (combined)

Both sides had loss. Losses combine.

```rust
use terni::{Imperfect, ConvergenceLoss};

let result = Imperfect::<i32, String, ConvergenceLoss>::Partial(1, ConvergenceLoss::new(3))
    .eh(|x| Imperfect::Partial(x + 1, ConvergenceLoss::new(5)));

assert!(result.is_partial());
assert_eq!(result.loss().steps(), 5);  // max(3, 5) for ConvergenceLoss
```

### Anything x Failure = Failure (loss carried)

Failure short-circuits. If the input is `Failure(E, L)`, `f` is never called — the carried loss is preserved. If `f` returns `Failure`, prior loss is combined with the failure's loss — the value is gone, but the cost of getting here is measured.

```rust
use terni::{Imperfect, ConvergenceLoss};

// Failure input: f is never called, carried loss preserved
let result = Imperfect::<i32, String, ConvergenceLoss>::Failure("gone".into(), ConvergenceLoss::new(4))
    .eh(|x| Imperfect::Success(x + 1));

assert!(result.is_err());
assert_eq!(result.loss().steps(), 4);  // carried loss, not total()

// Partial then failure: losses combine
let result = Imperfect::<i32, String, ConvergenceLoss>::Partial(1, ConvergenceLoss::new(3))
    .eh(|_| Imperfect::<i32, String, ConvergenceLoss>::Failure("broke".into(), ConvergenceLoss::new(5)));

assert!(result.is_err());
assert_eq!(result.loss().steps(), 5);  // max(3, 5) for ConvergenceLoss
```

## Chaining

`.eh()` composes naturally. Each step sees the value from the previous step. Loss accumulates across the entire chain.

```rust
use terni::{Imperfect, ConvergenceLoss};

fn validate(input: &str) -> Imperfect<i32, String, ConvergenceLoss> {
    match input.parse::<i32>() {
        Ok(n) if n > 0 => Imperfect::Success(n),
        Ok(n) => Imperfect::Partial(n.abs(), ConvergenceLoss::new(1)),  // corrected sign
        Err(_) => Imperfect::Failure(format!("not a number: {}", input), ConvergenceLoss::zero()),
    }
}

fn normalize(n: i32) -> Imperfect<i32, String, ConvergenceLoss> {
    if n > 100 {
        Imperfect::Partial(100, ConvergenceLoss::new(1))  // clamped
    } else {
        Imperfect::Success(n)
    }
}

fn score(n: i32) -> Imperfect<f64, String, ConvergenceLoss> {
    Imperfect::Success(n as f64 / 100.0)
}

// Full pipeline
let result = validate("-150")
    .eh(normalize)
    .eh(score);

assert!(result.is_partial());
assert_eq!(result.ok(), Some(1.0));
assert_eq!(result.loss().steps(), 1);  // max(1, 1) = 1 — sign corrected + clamped
```

## Recovery

`.recover()` attempts to salvage a value from `Failure`. Recovery from `Failure` always produces `Partial` — the failure happened, and that cost is carried forward.

```rust
use terni::{Imperfect, ConvergenceLoss};

let result = Imperfect::<i32, String, ConvergenceLoss>::Success(1)
    .eh(|_| Imperfect::<i32, String, ConvergenceLoss>::Failure("broke".into(), ConvergenceLoss::new(3)))
    .recover(|_e| Imperfect::Success(0));

assert!(result.is_partial());  // never Success — the failure was real
assert_eq!(result.ok(), Some(0));
assert_eq!(result.loss().steps(), 3);  // cost survives
```

`.unwrap_or()` and `.unwrap_or_else()` are shorthand for recovery with a default:

```rust
use terni::{Imperfect, ConvergenceLoss};

let failed: Imperfect<i32, String, ConvergenceLoss> =
    Imperfect::Failure("gone".into(), ConvergenceLoss::new(5));

let recovered = failed.unwrap_or(0);
assert!(recovered.is_partial());
assert_eq!(recovered.ok(), Some(0));
assert_eq!(recovered.loss().steps(), 5);
```

Success and Partial pass through `.recover()`, `.unwrap_or()`, and `.unwrap_or_else()` unchanged.

## Aliases

`.imp()` and `.tri()` are identical to `.eh()`. Same function, different name.

- **`.eh()`** — the shrug. Short, informal, gets the point across.
- **`.imp()`** — the word. Self-documenting in code that reads like prose.
- **`.tri()`** — the math. For code where "terni-functor" is the right frame.

Use whichever makes your code clearest. They compile to the same thing.

```rust
use terni::{Imperfect, ConvergenceLoss};

// All three are identical
let a = Imperfect::<i32, String, ConvergenceLoss>::Success(1)
    .eh(|x| Imperfect::Success(x + 1));
let b = Imperfect::<i32, String, ConvergenceLoss>::Success(1)
    .imp(|x| Imperfect::Success(x + 1));
let c = Imperfect::<i32, String, ConvergenceLoss>::Success(1)
    .tri(|x| Imperfect::Success(x + 1));

assert_eq!(a, b);
assert_eq!(b, c);
```

## Real-world: prism-core

prism-core's `Beam` trait uses `Imperfect` as its value carrier. Every beam carries an `Imperfect<Out, E, L>` internally, and the semifunctor `smap` maps over it using `.eh()`-style closures that return `Imperfect`.

From [`core/src/beam.rs`](https://github.com/systemic-engineering/prism/blob/main/core/src/beam.rs) — the `smap` method on the `Beam` trait:

```rust
fn smap<T>(
    self,
    f: impl FnOnce(&Self::Out) -> Imperfect<T, Self::Error, Self::Loss>,
) -> Self::Tick<T, Self::Error> {
    let imp = match self.result() {
        Imperfect::Success(v) | Imperfect::Partial(v, _) => f(v),
        Imperfect::Failure(_, _) => panic!("smap on Err beam"),
    };
    self.tick(imp)
}
```

And how it's used in practice — Traversal's split operation collapses a multi-element result:

```rust
let focused = traversal.focus(seed(vec![1, 2, 3]));
let first = focused.smap(|v| Imperfect::success(v.first().cloned().unwrap_or(0)));
assert_eq!(first.result().ok(), Some(&2));
```

The `PureBeam` constructors use terni's constructor methods directly:

```rust
pub fn ok(input: In, output: Out) -> Self {
    Self { input, imperfect: Imperfect::success(output) }
}
pub fn partial(input: In, output: Out, loss: L) -> Self {
    Self { input, imperfect: Imperfect::partial(output, loss) }
}
pub fn err(input: In, error: E) -> Self {
    Self { input, imperfect: Imperfect::failure(error) }
}
```

Loss propagation through the beam pipeline uses the same accumulation rules as `.eh()`:

```rust
fn propagate<T, E, L: Loss>(loss: L, next: Imperfect<T, E, L>) -> Imperfect<T, E, L> {
    match next {
        Imperfect::Success(v) => Imperfect::partial(v, loss),
        Imperfect::Partial(v, loss2) => Imperfect::partial(v, loss.combine(loss2)),
        Imperfect::Failure(e, loss2) => Imperfect::failure_with_loss(e, loss.combine(loss2)),
    }
}
```

[Back to README](../README.md) · [Context →](context.md)
