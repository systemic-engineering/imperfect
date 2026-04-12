# Migration

Moving from `Result<T, E>` to `Imperfect<T, E, L>`. You don't have to convert everything at once.

## Step 1: Choose a Loss type

What does "partial success" mean in your domain?

| If your code does... | Use |
|---|---|
| Iterative refinement, convergence loops | `ConvergenceLoss` |
| Partial observation, missing dimensions | `ApertureLoss` |
| Routing decisions, classifier selection | `RoutingLoss` |
| Something else | [Implement your own](loss-types.md) |

## Step 2: Convert return types

Start with one function. Replace `Result<T, E>` with `Imperfect<T, E, L>`.

Before:

```rust
fn process(input: &str) -> Result<i32, String> {
    let n: i32 = input.parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
    if n > 100 {
        Ok(100)  // silently clamped — information lost
    } else {
        Ok(n)
    }
}
```

After:

```rust
use terni::{Imperfect, ConvergenceLoss};

fn process(input: &str) -> Imperfect<i32, String, ConvergenceLoss> {
    let n: i32 = match input.parse() {
        Ok(n) => n,
        Err(e) => return Imperfect::Failure(e.to_string(), ConvergenceLoss::zero()),
    };
    if n > 100 {
        Imperfect::Partial(100, ConvergenceLoss::new(1))  // clamped — loss recorded
    } else {
        Imperfect::Success(n)
    }
}
```

The information that was silently discarded is now measured and carried.

## Step 3: Convert callers

Callers that used `?` on `Result` can use `Eh` to work with `Imperfect`:

Before:

```rust
# fn process(_: &str) -> Result<i32, String> { Ok(1) }
fn run(a: &str, b: &str) -> Result<i32, String> {
    let x = process(a)?;
    let y = process(b)?;
    Ok(x + y)
}
```

After:

```rust
use terni::{Imperfect, Eh, ConvergenceLoss};

# fn process(_: &str) -> Imperfect<i32, String, ConvergenceLoss> {
#     Imperfect::Success(1)
# }
fn run(a: &str, b: &str) -> Imperfect<i32, String, ConvergenceLoss> {
    let mut eh = Eh::new();
    let x = eh.eh(process(a)).unwrap_or_else(|e| panic!("{}", e));
    let y = eh.eh(process(b)).unwrap_or_else(|e| panic!("{}", e));
    eh.finish(x + y)
}
```

Or use the pipeline directly:

```rust
use terni::{Imperfect, ConvergenceLoss};

# fn process(_: &str) -> Imperfect<i32, String, ConvergenceLoss> {
#     Imperfect::Success(1)
# }
fn run(a: &str, b: &str) -> Imperfect<i32, String, ConvergenceLoss> {
    process(a).eh(|x| process(b).map(|y| x + y))
}
```

## Step 4: Gradual adoption

`From` conversions let `Imperfect` and `Result` coexist:

```rust
use terni::{Imperfect, ConvergenceLoss};

// Result → Imperfect (Ok becomes Success, Err becomes Failure)
let from_result: Imperfect<i32, String, ConvergenceLoss> =
    Ok::<i32, String>(42).into();
assert!(from_result.is_ok());

// Imperfect → Result (Success and Partial both become Ok, loss is discarded)
let back: Result<i32, String> =
    Imperfect::<i32, String, ConvergenceLoss>::Partial(42, ConvergenceLoss::new(3)).into();
assert_eq!(back, Ok(42));

// Option → Imperfect (Some becomes Success, None becomes Failure(()))
let from_option: Imperfect<i32, (), ConvergenceLoss> = Some(42).into();
assert!(from_option.is_ok());
```

Convert at the boundaries. Functions that return `Imperfect` can be called by code that only understands `Result` — just `.into()` or use `Result::from()`. Loss is discarded on that conversion, but it's explicit.

You don't need to convert your entire codebase. Convert the functions where partial success matters — where you're currently discarding information by collapsing to `Ok` or `Err`. The rest can stay as `Result`.

[Back to README](../README.md) · [Loss types →](loss-types.md)
