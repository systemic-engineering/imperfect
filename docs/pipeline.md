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

### Anything x Failure = Failure

Failure short-circuits. If the input is Failure, `f` is never called. If `f` returns Failure, prior loss is discarded — the value is gone.

```rust
use terni::{Imperfect, ConvergenceLoss};

// Failure input: f is never called
let result = Imperfect::<i32, String, ConvergenceLoss>::Failure("gone".into())
    .eh(|x| Imperfect::Success(x + 1));

assert!(result.is_err());

// Partial then failure: loss discarded, only error survives
let result = Imperfect::<i32, String, ConvergenceLoss>::Partial(1, ConvergenceLoss::new(3))
    .eh(|_| Imperfect::Failure("broke".into()));

assert!(result.is_err());
```

## Chaining

`.eh()` composes naturally. Each step sees the value from the previous step. Loss accumulates across the entire chain.

```rust
use terni::{Imperfect, ConvergenceLoss};

fn validate(input: &str) -> Imperfect<i32, String, ConvergenceLoss> {
    match input.parse::<i32>() {
        Ok(n) if n > 0 => Imperfect::Success(n),
        Ok(n) => Imperfect::Partial(n.abs(), ConvergenceLoss::new(1)),  // corrected sign
        Err(_) => Imperfect::Failure(format!("not a number: {}", input)),
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

[Back to README](../README.md) · [Context →](context.md)
