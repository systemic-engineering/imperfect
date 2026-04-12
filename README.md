# Imperfect

> I wanna thank Brene Brown for her work.

Ternary error handling for Rust. Because computation is not binary.

[![crates.io](https://img.shields.io/crates/v/imperfect.svg)](https://crates.io/crates/imperfect)
[![docs.rs](https://docs.rs/imperfect/badge.svg)](https://docs.rs/imperfect)
[![license](https://img.shields.io/crates/l/imperfect.svg)](https://github.com/systemic-engineering/prism/blob/main/imperfect/LICENSE)

## `eh`

The type. Three states instead of two.

```rust
use imperfect::{Imperfect, ConvergenceLoss};

let perfect: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Success(42);
let lossy = Imperfect::Partial(42, ConvergenceLoss::new(3));
let failed: Imperfect<u32, String, ConvergenceLoss> = Imperfect::Failure("gone".into());

assert!(perfect.is_ok());
assert!(lossy.is_partial());
assert!(failed.is_err());
```

[`Loss`](https://docs.rs/imperfect/latest/imperfect/trait.Loss.html) measures what didn't survive. It's a monoid: `zero()` identity, `combine` associative, `total()` absorbing.

Three loss types ship with the crate:
- **`ConvergenceLoss`** — distance to crystal. Combine: max.
- **`ApertureLoss`** — dark dimensions. Combine: union.
- **`RoutingLoss`** — decision entropy. Combine: max entropy, min gap.

[Loss types in depth →](docs/loss-types.md)

## `eh!`

The bind. Chain operations, accumulate loss.

```rust
use imperfect::{Imperfect, ConvergenceLoss};

let result = Imperfect::<i32, String, ConvergenceLoss>::Success(1)
    .eh(|x| Imperfect::Success(x * 2))
    .eh(|x| Imperfect::Partial(x + 1, ConvergenceLoss::new(3)));

assert_eq!(result.ok(), Some(3));
assert!(result.is_partial());
```

For explicit context with `?` propagation:

```rust
use imperfect::{Imperfect, Eh, ConvergenceLoss};

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
