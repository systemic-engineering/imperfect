# Benchmarks

## The line

**0.65 nanoseconds per honest step.** Only when there's something to be honest about. Otherwise: zero.

That's the cost of `Imperfect` over `Result`. Not the cost of the abstraction. The cost of the *measurement*. Result pays nothing because it records nothing. Imperfect pays 0.65 ns per step because it's accumulating the loss that Result threw away.

When there's no loss to accumulate, Imperfect pays nothing. The flight recorder has zero overhead when the flight is clean.

## Numbers

All benchmarks: Criterion, 10-step pipelines, stable x86_64, `--release`. Per-iteration times.

### Construction

| Variant | Time |
|---------|------|
| `Result::Ok` | 1.12 ns |
| `Imperfect::Success` | 1.33 ns |
| `Imperfect::Partial` | 1.35 ns |

210 ps overhead for Success. 230 ps for Partial. You will not find this in a profile.

### Pipeline (10-step `eh` chain)

| Pipeline | Time | Delta |
|----------|------|-------|
| `Result::and_then` (all Ok) | 569 ps | -- |
| `Imperfect::eh` (all Success) | 646 ps | +77 ps |
| `Result::and_then` (all Ok, baseline) | 621 ps | -- |
| `Imperfect::eh` (all Partial) | 7.18 ns | +6.56 ns |
| `Imperfect::eh` (mixed: 5 Success then 5 Partial) | 708 ps | +87 ps |

Three findings stacked:

**Success path is zero-cost.** 77 picoseconds over Result for ten chained operations. That's not overhead, that's CPU noise. The branch predictor sees straight-line Success and optimizes it identically to Result's Ok path.

**Partial path is the value.** 7.18 ns for 10 steps of loss accumulation. That's 0.65 ns per step where loss is actually being combined. You're paying for information that Result deletes. The cost is the measurement.

**Mixed pipeline: zero-cost until it fires.** `eh_mixed_at_5` runs five Success steps then five Partial steps and lands at 708 ps. Almost identical to ten clean steps. Loss accumulation only activates when loss exists. The branch predictor loves this shape -- long runs of one variant, clean transition, long run of the other.

### Recovery

| Operation | Time | Delta |
|-----------|------|-------|
| `Result::or_else` | 15.34 ns | -- |
| `Imperfect::recover` | 15.64 ns | +300 ps |

300 picoseconds. Within noise. Carrying loss through recovery adds nothing measurable. The loss is already there; `recover` just keeps it instead of dropping it.

### Map

| Operation | Time |
|-----------|------|
| `Result::map` | 562 ps |
| `Imperfect::map` (Success) | 647 ps |
| `Imperfect::map` (Partial) | 720 ps |

Same pattern. Success tracks Result. Partial adds the loss-preservation cost.

### Eh Context (10-step)

| Pipeline | Time |
|----------|------|
| All Success | 648 ps |
| All Partial | 6.06 ns |

The `Eh` struct adds zero overhead beyond the loss accumulation itself. Same performance characteristics as raw `eh` chains. The context is a zero-cost coordinator.

### Loss type combination

| Loss type | `combine` time | Notes |
|-----------|----------------|-------|
| `ConvergenceLoss` | 964 ps | `f64::max` -- trivial |
| `RoutingLoss` | 322 ps | `f64::max` on entropy -- trivial |
| `ApertureLoss` | 72.3 ns | `BTreeSet` union -- expected |

ConvergenceLoss and RoutingLoss combine in under 1 ns. They're scalar max operations. ApertureLoss is 72 ns because it unions sets of blocked dimensions -- that's allocation, and it's correct for the use case. You're tracking 16-dimensional apertures, not megabyte vectors.

## The shape

Result is a two-state type that optimizes for the fast path by *deleting the middle*. Imperfect is a three-state type that preserves it. The benchmarks say: preservation is free until there's something to preserve. Then it costs 0.65 ns per step.

That's not a tax. That's a price. And it buys you the flight recorder that Result said you couldn't have.

[Back to README](../README.md) · [Flight recorder →](flight-recorder.md) · [Pipeline →](pipeline.md)
