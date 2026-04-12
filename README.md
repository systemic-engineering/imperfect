# imperfect
[![crates.io](https://img.shields.io/crates/v/imperfect.svg)](https://crates.io/crates/imperfect)
[![docs.rs](https://docs.rs/imperfect/badge.svg)](https://docs.rs/imperfect)
[![no_std](https://img.shields.io/badge/no__std-compatible-blue)](https://github.com/systemic-engineering/prism/tree/main/imperfect)

I wanna thank Brene Brown for her work.


`Result` extended with partial success. Three states:

- **Success(T)** -- the transformation preserved everything. Zero loss.
- **Partial(T, L)** -- a value came through, but something was lost. The loss is measured and carried forward.
- **Failure(E)** -- no value survived.

Most real transformations are not perfect and not failed. They are partial: a value exists, and it cost something. Collapsing that into `Ok` or `Err` destroys the information about what was lost.

## Usage

```rust
use imperfect::{Imperfect, ShannonLoss, Loss};

// Three states
let perfect: Imperfect<u32, String> = Imperfect::Success(42);
let lossy: Imperfect<u32, String> = Imperfect::Partial(42, ShannonLoss::new(1.5));
let failed: Imperfect<u32, String> = Imperfect::Failure("gone".into());

// Check state -- is_ok() returns true for Success and Partial
assert!(perfect.is_ok());
assert!(lossy.is_ok());
assert!(failed.is_err());

// Extract
assert_eq!(perfect.ok(), Some(42));
assert_eq!(lossy.ok(), Some(42));   // value survives, loss is metadata
assert_eq!(failed.ok(), None);

// Measure loss
assert!(perfect.loss().is_zero());
assert_eq!(lossy.loss().as_f64(), 1.5);
assert!(failed.loss().as_f64().is_infinite());  // total loss
```

## Composition

`compose` propagates accumulated loss through a chain of results:

```rust
use imperfect::{Imperfect, ShannonLoss};

let step1: Imperfect<u32, String> = Imperfect::Partial(10, ShannonLoss::new(1.0));
let step2: Imperfect<u32, String> = Imperfect::Partial(20, ShannonLoss::new(0.5));

let result = step1.compose(step2);
assert_eq!(result.ok(), Some(20));
assert_eq!(result.loss().as_f64(), 1.5);  // losses accumulate
```

## The Loss trait

`Loss` is a monoid: `zero()` is the identity, `combine` is associative, `total()` is the absorbing element.

```rust
use imperfect::Loss;

pub trait Loss: Clone + Default {
    fn zero() -> Self;       // no loss occurred
    fn total() -> Self;      // everything was destroyed
    fn is_zero(&self) -> bool;
    fn combine(self, other: Self) -> Self;  // accumulate
}
```

`ShannonLoss` is the default implementation. Information loss measured in bits (base-2 logarithm). Losses combine by addition -- this is Shannon's channel coding theorem: information lost through sequential channels sums.

Implement `Loss` for domain-specific loss types. The `Imperfect` type is parameterized over any `L: Loss`.

## PbtA lineage

The three-state design descends from Powered by the Apocalypse tabletop games: 10+ is full success, 7-9 is success with complications, 6- is failure. The middle tier -- success with cost -- is the design innovation that PbtA contributed to game design. This crate encodes that structure in types.

## Compatibility

- `no_std` compatible: core type + `Loss` trait require no allocator.
- `std` interop (default feature): `From<Result<T, E>>`, `From<Option<T>>`, `Into<Result<T, E>>`.
- Zero dependencies.
