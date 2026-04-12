# Context

The `Eh` struct is a composition context that accumulates loss across a sequence of `Imperfect` operations, converting each to `Result` so you can use `?`.

## Why

The `.eh()` pipeline is clean when every step returns `Imperfect`. But sometimes you need to interleave `Imperfect` and `Result` operations in the same function, or you need early return on failure. `Eh` bridges the two worlds.

## Basic usage

```rust
use imperfect::{Imperfect, Eh, ConvergenceLoss};

fn process() -> Imperfect<i32, String, ConvergenceLoss> {
    let mut eh = Eh::new();

    let a = eh.eh(Imperfect::<i32, String, ConvergenceLoss>::Success(10))
        .map_err(|e| e.to_string())?;

    let b = eh.eh(Imperfect::<_, String, _>::Partial(a + 5, ConvergenceLoss::new(3)))
        .map_err(|e| e.to_string())?;

    // If any step was Failure, ? already returned Err.
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
use imperfect::{Eh, ConvergenceLoss};

let eh: Eh<ConvergenceLoss> = Eh::new();
assert!(eh.loss().is_none());
```

### `.eh(imp) -> Result<T, E>`

Extracts the value from an `Imperfect`, accumulating any loss. Returns `Ok(T)` for Success and Partial, `Err(E)` for Failure.

This is where loss gets absorbed into the context. Success adds nothing. Partial adds its loss (via `combine` if loss already exists). Failure returns `Err` immediately.

### `.imperfect()` and `.tri()`

Aliases for `.eh()`, same as on `Imperfect` itself.

### `.loss() -> Option<&L>`

Inspect accumulated loss without consuming the context. Returns `None` if no loss has accumulated (all steps were Success).

```rust
use imperfect::{Imperfect, Eh, ConvergenceLoss};

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
use imperfect::{Imperfect, Eh, ConvergenceLoss};
use std::num::ParseIntError;

fn parse_and_validate(input: &str) -> Imperfect<i32, String, ConvergenceLoss> {
    let mut eh = Eh::new();

    // Result operation — parse the input
    let raw: i32 = input.parse()
        .map_err(|e: ParseIntError| e.to_string())?;

    // Imperfect operation — validate range
    let validated = eh.eh(if raw > 100 {
        Imperfect::Partial(100, ConvergenceLoss::new(1))  // clamped
    } else if raw < 0 {
        Imperfect::<_, String, _>::Failure("negative".into())
    } else {
        Imperfect::Success(raw)
    })?;

    // Another Result operation
    let doubled = validated.checked_mul(2)
        .ok_or_else(|| "overflow".to_string())?;

    eh.finish(doubled)
}

# let r = parse_and_validate("50");
# assert_eq!(r.ok(), Some(100));
```

The key insight: `Eh.eh()` returns `Result`, so `?` works on it. Regular `Result` operations use `?` directly. Loss accumulates only through `Eh.eh()` calls. Everything else is standard Rust error handling.

## Example: payment verification

```rust
use imperfect::{Imperfect, Eh, ConvergenceLoss};

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

    let amount = eh.eh(verify_amount(&p))?;
    let currency = eh.eh(verify_currency(&p.currency))?;

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
