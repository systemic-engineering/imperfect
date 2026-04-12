# Loss Types

The `Loss` trait is what makes `Imperfect` more than a three-variant enum. It measures what didn't survive a transformation and accumulates that measurement through a pipeline.

## The `Loss` trait

```rust
pub trait Loss: Clone + Default {
    fn zero() -> Self;
    fn total() -> Self;
    fn is_zero(&self) -> bool;
    fn combine(self, other: Self) -> Self;
}
```

`Loss` is a monoid with an absorbing element:

- **`zero()`** — the identity. No loss occurred. `combine(zero(), x) == x`.
- **`total()`** — the annihilator. The transformation destroyed everything. `Failure` reports this.
- **`is_zero()`** — test whether this loss is lossless.
- **`combine()`** — accumulate two losses. Must be associative: `a.combine(b).combine(c) == a.combine(b.combine(c))`.

The semantics of `combine` are domain-specific. That's the point. "What does it mean to lose more?" depends on what you're measuring.

## `ConvergenceLoss`

Distance to crystal. How many steps remain before the result is fully converged.

**Domain:** Iterative refinement — optimization loops, numerical solvers, consensus protocols.

**Combine semantics:** `max`. The furthest-from-crystal step dominates. If one step is 3 iterations away and another is 7, the pipeline is 7 iterations away.

```rust
use imperfect::{Imperfect, ConvergenceLoss};

// Solver ran but didn't fully converge
let step1 = Imperfect::<f64, String, ConvergenceLoss>::Partial(3.14, ConvergenceLoss::new(5));

// Another step, closer to convergence
let result = step1.eh(|v| Imperfect::Partial(v * 2.0, ConvergenceLoss::new(2)));

// Loss is max(5, 2) = 5
assert_eq!(result.loss().steps(), 5);
```

**`zero()`** — 0 steps. Fully converged.
**`total()`** — `usize::MAX` steps. Infinite distance from crystal.

## `ApertureLoss`

Which dimensions were dark during observation. Tracks both which specific dimensions were unobserved and the fraction of total dimensions missed.

**Domain:** Partial observation — sensor arrays, feature extraction, dimensionality reduction.

**Combine semantics:** Union of dark dimensions, max of aperture fraction. If step A missed dims [1, 3] and step B missed dims [2, 3], the pipeline missed [1, 2, 3].

```rust
use imperfect::{Imperfect, ApertureLoss};

// Observed 8 of 10 dimensions — dims 2 and 7 were dark
let step1 = Imperfect::<Vec<f64>, String, ApertureLoss>::Partial(
    vec![1.0; 8],
    ApertureLoss::new(vec![2, 7], 10),
);

// Another observation missed dim 4
let result = step1.eh(|v| Imperfect::Partial(v, ApertureLoss::new(vec![4], 10)));

// Union: dims [2, 4, 7] now dark
assert_eq!(result.loss().dark_dims(), &[2, 4, 7]);
```

**`zero()`** — no dark dims, aperture 0.0. Full observation.
**`total()`** — aperture 1.0. Everything was dark.

## `RoutingLoss`

Decision uncertainty at a routing point. Measured as Shannon entropy of the routing distribution plus the probability gap between the top pick and the runner-up.

**Domain:** Model routing, classifier ensembles, A/B decisions — anywhere a choice was made under uncertainty.

**Combine semantics:** Max entropy (most uncertain routing dominates), min gap (tightest race dominates).

```rust
use imperfect::{Imperfect, RoutingLoss};

// Routed to model A with moderate confidence
let step1 = Imperfect::<String, String, RoutingLoss>::Partial(
    "response_a".into(),
    RoutingLoss::new(0.8, 0.3),  // 0.8 bits entropy, 30% gap
);

// Second routing, higher confidence
let result = step1.eh(|v| Imperfect::Partial(v, RoutingLoss::new(0.2, 0.7)));

// Entropy: max(0.8, 0.2) = 0.8. Gap: min(0.3, 0.7) = 0.3
assert_eq!(result.loss().entropy(), 0.8);
assert_eq!(result.loss().runner_up_gap(), 0.3);
```

**`zero()`** — 0.0 entropy, 1.0 gap. One model at 100%.
**`total()`** — infinite entropy, 0.0 gap. Maximum uncertainty.

## Implementing your own Loss type

Implement `Loss` for any domain-specific measurement. The only requirements: `Clone + Default`, the four trait methods, and `combine` must be associative.

```rust
use imperfect::{Loss, Imperfect};

/// Tracks accumulated latency as loss.
#[derive(Clone, Debug, PartialEq, Default)]
struct LatencyLoss(std::time::Duration);

impl Loss for LatencyLoss {
    fn zero() -> Self {
        LatencyLoss(std::time::Duration::ZERO)
    }

    fn total() -> Self {
        LatencyLoss(std::time::Duration::MAX)
    }

    fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    fn combine(self, other: Self) -> Self {
        LatencyLoss(self.0.saturating_add(other.0))
    }
}

// Now use it
let result = Imperfect::<String, String, LatencyLoss>::Partial(
    "data".into(),
    LatencyLoss(std::time::Duration::from_millis(50)),
).eh(|v| Imperfect::Partial(
    v,
    LatencyLoss(std::time::Duration::from_millis(30)),
));

assert_eq!(result.loss().0, std::time::Duration::from_millis(80));
```

Choose your `combine` semantics carefully. Latency adds. Convergence distance maxes. Aperture unions. The semantics encode what "more loss" means in your domain.

[Back to README](../README.md) · [Pipeline →](pipeline.md)
