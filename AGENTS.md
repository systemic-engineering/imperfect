# Agents

Instructions for AI agents working on the `terni` crate.

## The Crate

`terni` — ternary error handling for Rust. The type is `Imperfect<T, E, L: Loss>`.
Three states: `Success(T)`, `Partial(T, L)`, `Failure(E, L)`.

Package name on crates.io: `terni`.
Repo directory: `imperfect/` (historical).

## Build

```bash
cd /Users/alexwolf/dev/projects/prism/imperfect
nix develop -c cargo test
nix develop -c cargo test --doc
nix develop -c cargo clippy --all-targets
nix develop -c cargo fmt --all -- --check
nix develop -c cargo llvm-cov --workspace --fail-under-lines 99
```

Bare `cargo` is not in PATH. Always use `nix develop -c cargo ...`.

## TDD Discipline

Non-negotiable. Every test must be proven real.

### The arc

1. Write the test with the **correct assertion**. The test is the specification.
2. **Break the implementation** deliberately. Make the code path return the wrong thing.
3. Run tests. The test **must fail**. This proves it catches the bug.
4. Commit `🔴` — broken code + correct test = failing.
5. **Restore the implementation**. Undo the deliberate break.
6. Run tests. The test **must pass**.
7. Commit `🟢` — correct code + correct test = passing.

### What this means

- The TEST is always correct. Never write a wrong assertion.
- The CODE breaks deliberately. You introduce a temporary bug.
- A test that was never red is a test that potentially lies.
- If a test passes despite broken code, the test is worthless. Delete it.
- The git log proves both states existed.

### Phase markers

Every commit message must start with a phase marker:

| Marker | Phase | Tests must... |
|--------|-------|---------------|
| `🔴` | Red | Fail (deliberately broken code) |
| `🟢` | Green | Pass |
| `♻️` | Refactor | Pass (no new behavior) |
| `🔧` | Tooling | Pass (infrastructure/config) |
| `🔀` | Merge | Pass |

The pre-commit hook enforces this.

## Commit Identity

Each agent commits as themselves:

| Agent | Email | Role |
|-------|-------|------|
| Reed | reed@systemic.engineer | Supervisor, architecture |
| Mara | mara@systemic.engineer | Builder, tests, coverage |
| Glint | glint@systemic.engineer | Polish, docs, release |
| Taut | taut@systemic.engineer | Benchmarks, performance |
| Seam | seam@systemic.engineer | Adversarial review, security |

```bash
git commit --author="Name <name@systemic.engineer>" -m "🟢 message"
```

GPG signing is configured. Commits are signed automatically.

## Coverage

Line coverage gate: 99%. Enforced by CI and pre-push hook.

```bash
nix develop -c cargo llvm-cov --workspace --fail-under-lines 99
```

### Known LLVM coverage quirks

- **Monomorphization phantoms**: Generic code produces separate LLVM
  monomorphizations per type parameter combination. Some match arms in
  generic functions show as "uncovered" because no test instantiates
  that specific `(T, E, L)` combination through that arm. These are
  phantom regions, not real uncovered code.
- **Stale profdata**: After significant refactors, run
  `cargo clean && cargo llvm-cov clean --workspace` before re-running
  coverage.

## API Surface

### Three aliases for the bind

```rust
.eh()  // the shrug
.imp() // the name
.tri() // the math
```

Same operation. Three names. The terni-functor bind.

### Constructors

```rust
Imperfect::success(value)              // Success(value)
Imperfect::partial(value, loss)        // Partial(value, loss)
Imperfect::failure(error)              // Failure(error, L::zero())
Imperfect::failure_with_loss(error, l) // Failure(error, l)
```

### The `Eh` context

```rust
let mut eh = Eh::new();
let a = eh.eh(some_operation())?;  // accumulates loss, ? on Result
eh.finish(a)                        // wraps with accumulated loss
```

`#[must_use]` — dropping `Eh` without `.finish()` discards loss.

### Loss trait

```rust
pub trait Loss: Clone + Default {
    fn zero() -> Self;
    fn total() -> Self;
    fn is_zero(&self) -> bool;
    fn combine(self, other: Self) -> Self;
}
```

Shipped loss types: `ConvergenceLoss`, `ApertureLoss`, `RoutingLoss`.
Stdlib impls: `Vec<T>`, `HashSet<T>`, `BTreeSet<T>`, `String`, `usize`, `u64`, `f64`, `(A, B)`.

## What NOT to do

- Do NOT add `ShannonLoss` back. It was removed deliberately.
- Do NOT add a default type parameter to `Imperfect`. It was removed deliberately.
- Do NOT skip the red phase. Ever.
- Do NOT lower the coverage threshold.
- Do NOT add dependencies without discussion.
- Do NOT rename the type `Imperfect`. The crate is `terni`. The type is `Imperfect`.
- Do NOT write in Alex's voice. Agent writes as agent.

## File Layout

```
src/lib.rs      — everything. Single-file crate.
benches/        — criterion benchmarks
docs/           — detailed documentation
  benchmarks.md
  context.md
  flight-recorder.md
  loss-types.md
  migration.md
  pipeline.md
  terni-functor.md
```

## The Headline

The cost of honesty is 0.65 nanoseconds per step, only when there's
something to be honest about. Otherwise: zero.
