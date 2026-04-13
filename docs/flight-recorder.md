# Not a Stack Trace. A Flight Recorder.

`Result` gives you the error.
`Imperfect` gives you the error and the receipt.

## The difference

A stack trace is a post-mortem. It's reconstructed after the crash. It tells
you where the failure happened. It's a debug artifact — gated behind
`RUST_BACKTRACE=1`, stripped in release builds, untyped, uncomposable.

The loss on `Failure(E, L)` is a flight recorder. It was running before
anything went wrong. It tells you what every step cost — not just the failure
point. It's a production type — in the signature, enforced by the compiler,
typed, composable, always on.

|                          | `Result`             | `Imperfect`          |
|--------------------------|----------------------|----------------------|
| What failed              | ✓                    | ✓                    |
| Where it failed          | `RUST_BACKTRACE=1`   | in the type          |
| What it cost to get here | —                    | `L`                  |
| Debug or production      | debug                | production           |
| Typed                    | no (string)          | yes (`L: Loss`)      |
| Composable               | no                   | `combine()`          |
| Always on                | no (env var)         | yes (return type)    |

## What it looks like

```rust
use terni::{Imperfect, RoutingLoss};

// Result: you get the error. That's it.
let r: Result<i32, &str> = Err("validation failed");

// Imperfect: you get the error AND the receipt.
let i: Imperfect<i32, &str, RoutingLoss> = Imperfect::Failure(
    "validation failed",
    RoutingLoss::new(0.7, 0.1),
    // entropy 0.7: the routing was uncertain
    // gap 0.1: the decision was close
);
```

The loss on the Failure tells you not just what failed but what the system
was doing when it failed. How uncertain. How far from crystal. What it cost
to arrive here.

## The flight recorder is always running

In a pipeline:

```rust
use terni::{Imperfect, ConvergenceLoss};

fn step_one(x: i32) -> Imperfect<i32, String, ConvergenceLoss> {
    Imperfect::Partial(x + 1, ConvergenceLoss::new(3))
}

fn step_two(x: i32) -> Imperfect<i32, String, ConvergenceLoss> {
    Imperfect::Partial(x * 2, ConvergenceLoss::new(5))
}

fn step_three(x: i32) -> Imperfect<i32, String, ConvergenceLoss> {
    Imperfect::Failure("broke".into(), ConvergenceLoss::new(2))
}

let result = step_one(1)
    .eh(step_two)
    .eh(step_three);

// result is Failure("broke", ConvergenceLoss(5))
// The loss is max(3, 5, 2) = 5 — the worst convergence across the pipeline.
// Steps 1 and 2 succeeded partially. Step 3 failed.
// The flight recorder captured all of it.
```

With `Result`, `step_three` returning `Err("broke")` throws away everything
steps 1 and 2 learned. With `Imperfect`, the failure carries the accumulated
loss from the entire journey.

## Zero cost on the success path

When every step returns `Success`, no loss is allocated. The `.eh()` bind
on `Success` is a direct function application — same as `Result::and_then`
on `Ok`. The flight recorder has zero overhead when there's nothing to record.

The cost only appears when loss accumulates — and that cost IS the value.
You're paying for information that `Result` throws away.

## Compose your flight recorder

Loss types compose via tuples:

```rust
use terni::{Imperfect, ConvergenceLoss, RoutingLoss};

// Track two independent loss dimensions simultaneously
type PipelineLoss = (ConvergenceLoss, RoutingLoss);

let result: Imperfect<i32, String, PipelineLoss> = Imperfect::Partial(
    42,
    (ConvergenceLoss::new(3), RoutingLoss::new(0.5, 0.2)),
);

// The flight recorder tracks convergence AND routing uncertainty
// in the same value. Both compose independently via combine().
```

Use `Vec<String>` as a labeled loss log:

```rust
use terni::Imperfect;

let result: Imperfect<i32, String, Vec<String>> = Imperfect::Partial(
    42,
    vec!["step 1: cache miss".into(), "step 3: fallback used".into()],
);

// The loss IS the log. Human-readable. Composable. In the type.
```

## The two empty cells

The table has two empty cells in the Result column. "What it cost to get here"
and "Always on." Result doesn't have answers for these because Result can't
represent them. The type doesn't have a place to put the information.

Those empty cells are why this crate exists.

## Real-world: prism-core's ScalarLoss

prism-core implements its own `Loss` type — [`ScalarLoss`](https://github.com/systemic-engineering/prism/blob/main/core/src/scalar_loss.rs) — for eigenvalue decomposition. When a spectral projection zeroes out eigenvalues below a precision threshold, the magnitude of what was discarded IS the loss. This is the flight recorder in practice: the loss tells you how much signal was thrown away at each stage of a spectral pipeline.

From `core/src/scalar_loss.rs`:

```rust
/// A scalar information loss measured in bits.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ScalarLoss(pub f64);

impl Loss for ScalarLoss {
    fn zero() -> Self { ScalarLoss(0.0) }
    fn total() -> Self { ScalarLoss(f64::INFINITY) }
    fn is_zero(&self) -> bool { self.0 == 0.0 }
    fn combine(self, other: Self) -> Self { ScalarLoss(self.0 + other.0) }
}
```

The `combine` is addition — scalar losses accumulate linearly. `total()` is infinity, acting as a proper absorbing element.

In the `Transport` trait (the bundle tower's parallel transport operation), the holonomy IS the loss — comprehension always costs something:

```rust
impl Transport for TestBundle {
    type Holonomy = ScalarLoss;
    fn transport(&self, state: &[f64; 4]) -> Imperfect<[f64; 4], Infallible, ScalarLoss> {
        let compressed = [state[0], state[1], 0.0, 0.0];
        let loss = state[2].abs() + state[3].abs();
        if loss == 0.0 {
            Imperfect::success(compressed)
        } else {
            Imperfect::partial(compressed, ScalarLoss::new(loss))
        }
    }
}
```

The two zeroed-out dimensions are measured. The flight recorder captures exactly what was discarded, in bits. Downstream consumers see `Partial` and know: a value exists, but it cost something. The loss says how much.

[Back to README](../README.md) · [Loss types →](loss-types.md) · [Benchmarks →](benchmarks.md)
