# Context

The `Eh` struct is a composition context that accumulates loss across a sequence of `Imperfect` operations, converting each to `Result`.

## Why

The `.eh()` pipeline is clean when every step returns `Imperfect`. But sometimes you need to interleave `Imperfect` and `Result` operations in the same function, or you need early return on failure. `Eh` bridges the two worlds.

**Note:** `Imperfect` does not implement the `Try` trait (it's nightly-only), so you can't use `?` directly in functions returning `Imperfect`. Use `match` on the `Result` from `eh.eh()` and return `Imperfect::Failure` on `Err`.

## Basic usage

```rust
use terni::{Imperfect, Eh, ConvergenceLoss};

fn process() -> Imperfect<i32, String, ConvergenceLoss> {
    let mut eh = Eh::new();

    let a = match eh.eh(Imperfect::<i32, String, ConvergenceLoss>::Success(10)) {
        Ok(v) => v,
        Err(e) => return Imperfect::Failure(e),
    };

    let b = match eh.eh(Imperfect::<_, String, _>::Partial(a + 5, ConvergenceLoss::new(3))) {
        Ok(v) => v,
        Err(e) => return Imperfect::Failure(e),
    };

    // If any step was Failure, we already returned.
    // If any step was Partial, loss is accumulated in eh.
    eh.finish(b)
}

# let result = process();
# assert!(result.is_partial());
```

## API

### `Eh::new()`

Creates a context with zero accumulated loss.

```rust
use terni::{Eh, ConvergenceLoss};

let eh: Eh<ConvergenceLoss> = Eh::new();
assert!(eh.loss().is_none());
```

### `.eh(imp) -> Result<T, E>`

Extracts the value from an `Imperfect`, accumulating any loss. Returns `Ok(T)` for Success and Partial, `Err(E)` for Failure.

This is where loss gets absorbed into the context. Success adds nothing. Partial adds its loss (via `combine` if loss already exists). Failure returns `Err` immediately.

### `.imp()` and `.tri()`

Aliases for `.eh()`, same as on `Imperfect` itself.

### `.loss() -> Option<&L>`

Inspect accumulated loss without consuming the context. Returns `None` if no loss has accumulated (all steps were Success).

```rust
use terni::{Imperfect, Eh, ConvergenceLoss};

let mut eh: Eh<ConvergenceLoss> = Eh::new();
assert!(eh.loss().is_none());

let _ = eh.eh(Imperfect::<i32, String, ConvergenceLoss>::Partial(1, ConvergenceLoss::new(3)));
assert_eq!(eh.loss().unwrap().steps(), 3);

let _ = eh.eh(Imperfect::<i32, String, ConvergenceLoss>::Partial(2, ConvergenceLoss::new(7)));
assert_eq!(eh.loss().unwrap().steps(), 7);  // max(3, 7)
```

### `.finish(value) -> Imperfect<T, E, L>`

Wraps the final value with accumulated loss. If no loss accumulated, returns `Success`. If any did, returns `Partial`.

This is the exit point. It converts back from `Result`-land to `Imperfect`.

## `#[must_use]`

`Eh` is marked `#[must_use]`. If you create an `Eh` and drop it without calling `.finish()`, the compiler warns you. Dropping the context silently discards accumulated loss — exactly the information `Imperfect` exists to preserve.

## Mixing Imperfect and Result

`Eh` is the bridge between `Imperfect` and `Result`. Inside an `Eh` block, you can freely mix both:

```rust
use terni::{Imperfect, Eh, ConvergenceLoss};
use std::num::ParseIntError;

fn parse_and_validate(input: &str) -> Imperfect<i32, String, ConvergenceLoss> {
    let mut eh = Eh::new();

    // Result operation — parse the input
    let raw: i32 = match input.parse::<i32>() {
        Ok(n) => n,
        Err(e) => return Imperfect::Failure(e.to_string()),
    };

    // Imperfect operation — validate range
    let validated = match eh.eh(if raw > 100 {
        Imperfect::Partial(100, ConvergenceLoss::new(1))  // clamped
    } else if raw < 0 {
        Imperfect::<_, String, _>::Failure("negative".into())
    } else {
        Imperfect::Success(raw)
    }) {
        Ok(v) => v,
        Err(e) => return Imperfect::Failure(e),
    };

    // Another Result operation
    let doubled = match validated.checked_mul(2) {
        Some(v) => v,
        None => return Imperfect::Failure("overflow".to_string()),
    };

    eh.finish(doubled)
}

# let r = parse_and_validate("50");
# assert_eq!(r.ok(), Some(100));
```

The key insight: `Eh.eh()` returns `Result`, so you can match on it for early return. Loss accumulates only through `Eh.eh()` calls. Everything else is standard Rust error handling. If your function returns `Result` (not `Imperfect`), you can use `?` on `eh.eh()` directly.

## Example: payment verification

```rust
use terni::{Imperfect, Eh, ConvergenceLoss};

struct Payment { amount: u64, currency: String }
struct VerifiedPayment { amount: u64, currency: String, risk_score: f64 }

fn verify_amount(p: &Payment) -> Imperfect<u64, String, ConvergenceLoss> {
    if p.amount == 0 {
        Imperfect::Failure("zero amount".into())
    } else if p.amount > 10_000 {
        Imperfect::Partial(p.amount, ConvergenceLoss::new(2))  // needs review
    } else {
        Imperfect::Success(p.amount)
    }
}

fn verify_currency(c: &str) -> Imperfect<String, String, ConvergenceLoss> {
    match c {
        "USD" | "EUR" => Imperfect::Success(c.to_string()),
        "GBP" => Imperfect::Partial(c.to_string(), ConvergenceLoss::new(1)),  // supported but slower
        _ => Imperfect::Failure(format!("unsupported currency: {}", c)),
    }
}

fn verify_payment(p: Payment) -> Imperfect<VerifiedPayment, String, ConvergenceLoss> {
    let mut eh = Eh::new();

    let amount = match eh.eh(verify_amount(&p)) {
        Ok(v) => v,
        Err(e) => return Imperfect::Failure(e),
    };
    let currency = match eh.eh(verify_currency(&p.currency)) {
        Ok(v) => v,
        Err(e) => return Imperfect::Failure(e),
    };

    let risk_score = match eh.loss() {
        Some(loss) => 0.5 + (loss.steps() as f64 * 0.1),  // higher loss = higher risk
        None => 0.1,
    };

    eh.finish(VerifiedPayment { amount, currency, risk_score })
}

# let p = Payment { amount: 15_000, currency: "GBP".into() };
# let result = verify_payment(p);
# assert!(result.is_partial());
# assert_eq!(result.loss().steps(), 2);  // max(2, 1)
```

The loss tells downstream consumers how much confidence to place in this result. Zero loss = fully verified. Nonzero = verified with caveats. Failure = rejected.

[Back to README](../README.md) · [Terni-functor →](terni-functor.md)
