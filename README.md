# terni

> I wanna thank Brene Brown for her work.

Ternary error handling for Rust. Because computation is not binary.

[![crates.io](https://img.shields.io/crates/v/terni.svg)](https://crates.io/crates/terni)
[![docs.rs](https://docs.rs/terni/badge.svg)](https://docs.rs/terni)
[![license](https://img.shields.io/crates/l/terni.svg)](https://github.com/systemic-engineering/prism/blob/main/imperfect/LICENSE)

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

The question. Coming in a future release.

Block macro for implicit loss accumulation — `eh! { }` will do what `Eh` does without the boilerplate.

## More

- [Loss types](docs/loss-types.md) — the `Loss` trait, shipped types, custom implementations
- [Pipeline](docs/pipeline.md) — `.eh()` bind in depth, loss accumulation rules
- [Context](docs/context.md) — `Eh` struct, mixing `Imperfect` and `Result`
- [Terni-functor](docs/terni-functor.md) — the math behind `.eh()`
- [Migration](docs/migration.md) — moving from `Result<T, E>` to `Imperfect<T, E, L>`

## License

Apache-2.0
