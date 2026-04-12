# Terni-Functor

For the mathematically curious. Not required reading — you can use `.eh()` productively without any of this.

## What it is

A terni-functor is a three-state composition that carries a monoidal annotation through the middle state. `Imperfect<T, E, L>` is one:

- **Success(T)** — pure value, zero annotation
- **Partial(T, L)** — value with annotation
- **Failure(E, L)** — no value, but the cost of getting here is measured

The bind operator (`.eh()`) composes these while accumulating the annotation via the `Loss` monoid.

## Relation to bifunctors

`Result<T, E>` is a bifunctor — it has two type parameters that can be mapped independently (`map` for `T`, `map_err` for `E`). `Imperfect` extends this to three parameters. It's a trifunctor in the same sense: `map` for `T`, `map_err` for `E`, and the `Loss` parameter is accumulated rather than mapped.

The key difference: `Result` is a coproduct (either/or). `Imperfect` has a middle state that carries both a value *and* metadata about that value. This middle state is where the interesting composition happens.

## Relation to Writer

Haskell's `Writer w a` carries a monoidal log alongside a value. `Partial(T, L)` looks similar — value plus monoid. But `Writer` always carries the log. `Imperfect` has three states:

- `Success` carries no log (it's structurally absent, not zero)
- `Partial` carries the log
- `Failure` has no value, but carries the accumulated loss from before the failure

This is not `Writer`. `Writer` is `(a, w)`. `Imperfect` is `Success a | Partial a w | Failure e w`. The failure path and the "genuinely zero loss" path both exist as distinct states, not as special values of the monoid. Failure carries loss to preserve the cost of computation that preceded it.

## The monad laws

For `.eh()` to be a genuine bind, it must satisfy three laws:

### Left identity

`return a >>= f  ==  f a`

```rust
use terni::{Imperfect, ConvergenceLoss};

fn f(x: i32) -> Imperfect<i32, String, ConvergenceLoss> {
    Imperfect::Partial(x * 2, ConvergenceLoss::new(1))
}

let left = Imperfect::<i32, String, ConvergenceLoss>::Success(5).eh(f);
let right = f(5);

assert_eq!(left, right);
```

`Success` is `return`. Binding `Success(a)` through `f` gives exactly `f(a)`.

### Right identity

`m >>= return  ==  m`

```rust
use terni::{Imperfect, ConvergenceLoss};

let m = Imperfect::<i32, String, ConvergenceLoss>::Partial(5, ConvergenceLoss::new(3));

let result = m.clone().eh(|x| Imperfect::Success(x));

assert_eq!(result, m);
```

Binding through `Success` (return) preserves the original value and loss.

### Associativity

`(m >>= f) >>= g  ==  m >>= (|x| f(x) >>= g)`

```rust
use terni::{Imperfect, ConvergenceLoss};

fn f(x: i32) -> Imperfect<i32, String, ConvergenceLoss> {
    Imperfect::Partial(x + 1, ConvergenceLoss::new(2))
}

fn g(x: i32) -> Imperfect<i32, String, ConvergenceLoss> {
    Imperfect::Partial(x * 2, ConvergenceLoss::new(3))
}

let m = Imperfect::<i32, String, ConvergenceLoss>::Partial(1, ConvergenceLoss::new(1));

let left = m.clone().eh(f).eh(g);
let right = m.eh(|x| f(x).eh(g));

assert_eq!(left, right);
```

The order of binding doesn't matter. Loss accumulation is associative because `Loss::combine` is associative.

## Why this works

The monad laws hold because:

1. `Success` acts as a genuine unit — no loss to combine, value passes through.
2. `Failure` acts as a zero — short-circuits, `f` never called.
3. `Partial`'s loss accumulation delegates to `Loss::combine`, which is required to be associative.

The `Loss` monoid does the heavy lifting. Any associative `combine` with an identity `zero` gives you a lawful bind for free. The three-state structure just adds the failure short-circuit that `Writer` lacks.

## Why this has no precedent

Every mainstream language handles errors in two states: success or failure. Some languages carry metadata (Java's checked exceptions carry types, Go returns `(T, error)` tuples), but none reify the middle state as a first-class composition target.

`Either`/`Result` is the closest: two states, one bind operator. Haskell's `Writer` carries metadata but has no failure state. `ExceptT (WriterT ...)` monad transformer stacks come close, but they compose loss even through the success path — there's no "genuinely lossless" state distinct from "zero loss."

`Imperfect` is the first type (that we know of) to combine all three: success, annotated success, and failure in a single bind operator with lawful monad composition.

The design came from tabletop games, not category theory. PbtA's three-tier outcome structure (full success / success with cost / failure) was the insight. The math just confirmed it was sound.

[Back to README](../README.md) · [Migration →](migration.md)
