# terni

> I wanna thank Brene Brown for her work.

Ternary error handling for Rust. Because computation is not binary.

[![crates.io](https://img.shields.io/crates/v/terni.svg)](https://crates.io/crates/terni)
[![docs.rs](https://docs.rs/terni/badge.svg)](https://docs.rs/terni)
[![license](https://img.shields.io/crates/l/terni.svg)](https://github.com/systemic-engineering/prism/blob/main/imperfect/LICENSE)

**The cost of honesty is 0.65 nanoseconds per step, only when there's something to be honest about. Otherwise: zero.**

## `eh`

The type. Three states instead of two.

```rust
use terni::{Imperfect, ConvergenceLoss};

let perfect: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Success(42);
let lossy = Imperfect::Partial(42, ConvergenceLoss::new(3));
let failed: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Failure("gone".into(), ConvergenceLoss::new(0));
let costly_failure: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Failure("gone".into(), ConvergenceLoss::new(5));

assert!(perfect.is_ok());
assert!(lossy.is_partial());
assert!(failed.is_err());
// Failure carries accumulated loss — the cost of getting here:
assert_eq!(costly_failure.loss().steps(), 5);
```

`Failure(E, L)` carries the accumulated loss from before the failure. The loss tells you what it cost to arrive here. If you need to distinguish "failed immediately" from "failed after expensive work," the carried loss is that information.

[`Loss`](https://docs.rs/terni/latest/terni/trait.Loss.html) measures what didn't survive. It's a monoid: `zero()` identity, `combine` associative, `total()` absorbing.

Three loss types ship with the crate:
- **`ConvergenceLoss`** — distance to crystal. Combine: max.
- **`ApertureLoss`** — dark dimensions. Combine: union.
- **`RoutingLoss`** — decision entropy. Combine: max entropy, min gap.

### Migration from `Result`

| Result           | terni                       |                |
|------------------|-----------------------------|----------------|
| `Ok(v)`          | `Imperfect::Success(v)`     | same           |
| `Err(e)`         | `Imperfect::Failure(e, l)`  | same           |
|                  | `Imperfect::Partial(v, l)`  | **new**        |
|                  | `Imperfect::Failure(e, l)`  | **honest**     |

The two empty cells on the left are the argument. `Result` doesn't have a row for partial success or honest failure. That's why terni exists.

### Constructors

Four ways to build an `Imperfect`:

```rust
use terni::{Imperfect, ConvergenceLoss};

let a = Imperfect::<i32, String, ConvergenceLoss>::success(42);
let b = Imperfect::<i32, String, ConvergenceLoss>::partial(42, ConvergenceLoss::new(3));
let c = Imperfect::<i32, String, ConvergenceLoss>::failure("gone".into());
let d = Imperfect::<i32, String, ConvergenceLoss>::failure_with_loss("gone".into(), ConvergenceLoss::new(5));
```

`.failure()` carries zero loss. `.failure_with_loss()` carries accumulated loss from prior steps.

[Loss types in depth →](docs/loss-types.md) · [Full migration guide →](docs/migration.md)

## `eh!`

The bind. Chain operations, accumulate loss.

```rust
use terni::{Imperfect, ConvergenceLoss};

let result = Imperfect::<i32, String, ConvergenceLoss>::Success(1)
    .eh(|x| Imperfect::Success(x * 2))
    .eh(|x| Imperfect::Partial(x + 1, ConvergenceLoss::new(3)));

assert_eq!(result.ok(), Some(3));
assert!(result.is_partial());
```

Recovery from failure carries the cost:

```rust
use terni::{Imperfect, ConvergenceLoss};

let result = Imperfect::<i32, String, ConvergenceLoss>::Success(1)
    .eh(|x| Imperfect::Partial(x * 2, ConvergenceLoss::new(3)))
    .eh(|_| Imperfect::<i32, String, ConvergenceLoss>::Failure("broke".into(), ConvergenceLoss::new(2)))
    .recover(|_e| Imperfect::Success(0));

// Recovery from Failure always produces Partial — the failure was real
assert!(result.is_partial());
assert_eq!(result.ok(), Some(0));
```

For explicit context with loss accumulation:

```rust
use terni::{Imperfect, Eh, ConvergenceLoss};

let mut eh = Eh::new();
let a = eh.eh(Imperfect::<i32, String, ConvergenceLoss>::Success(1)).unwrap();
let b = eh.eh(Imperfect::<_, String, _>::Partial(a + 1, ConvergenceLoss::new(5))).unwrap();
let result: Imperfect<i32, String, ConvergenceLoss> = eh.finish(b);

assert!(result.is_partial());
```

`.imp()` and `.tri()` are aliases for `.eh()` — same bind, different name. Use whichever reads best in your code.

[Pipeline guide →](docs/pipeline.md) · [Context guide →](docs/context.md)

## `eh?`

The `eh!` macro tries extra hard to recover meaning at the boundary. Roll+Loss.

10+ **Success.** 7-9 **Partial** with a cost. 6- **Failure.**

For real this time.

```rust
use terni::{eh, Imperfect, ConvergenceLoss};

fn process(input: i32) -> Imperfect<i32, String, ConvergenceLoss> {
    eh! {
        let a = step_one(input)?;
        let b = step_two(a)?;
        b + 1
    }
}

fn step_one(x: i32) -> Imperfect<i32, String, ConvergenceLoss> {
    Imperfect::Partial(x * 2, ConvergenceLoss::new(1))
}

fn step_two(x: i32) -> Imperfect<i32, String, ConvergenceLoss> {
    Imperfect::Success(x + 10)
}

let result = process(5);
assert!(result.is_partial());
assert_eq!(result.loss().steps(), 1);
assert_eq!(result.ok(), Some(21));
```

Plain `?` on `Imperfect`. Loss accumulates implicitly. No context variable. No `.eh()` calls.

The macro rewrites `expr?` to route through an `IntoEh` trait, which handles both `Imperfect` (accumulates loss) and `Result` (passes through). Mix them freely inside an `eh!` block.

`return` inside `eh!` returns from the block, not the enclosing function. Use `?` for early exit.

[Macro guide ->](docs/macro.md)

## More

- [Macro](docs/macro.md) — `eh!` block macro, `IntoEh` trait, how it works
- [Loss types](docs/loss-types.md) — the `Loss` trait, shipped types, stdlib impls, custom implementations
- [Pipeline](docs/pipeline.md) — `.eh()` bind in depth, loss accumulation rules
- [Context](docs/context.md) — `Eh` struct, mixing `Imperfect` and `Result`
- [Terni-functor](docs/terni-functor.md) — the math behind `.eh()`
- [Migration](docs/migration.md) — moving from `Result<T, E>` to `Imperfect<T, E, L>`
- [Flight recorder](docs/flight-recorder.md) — `Failure(E, L)` as production telemetry, not stack traces
- [Benchmarks](docs/benchmarks.md) — 0.65 ns per honest step, zero on the success path

## See it in action

- **[prism-core](https://github.com/systemic-engineering/prism/tree/main/core)** — spectral optics pipeline. Uses `Imperfect` as the value carrier inside `Beam`, with a custom `ScalarLoss` type for eigenvalue decomposition. 182 tests.

## License

Apache-2.0
