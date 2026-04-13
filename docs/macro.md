# The `eh!` Macro

The block that tries extra hard. Roll+Loss.

**10+** Success — zero loss, clean hit.
**7-9** Partial — you get the value, but something was lost. The loss is measured.
**6-** Failure — the MC makes a move. The cost carries.

The design descends from PbtA (Powered by the Apocalypse). The 7-9 result — success with complications — is the design innovation that PbtA contributed to game design. `eh!` encodes that structure in a proc macro.

`eh` — the shrug. For the engineer who's been here before.
`eh` — extra hard. For the engineer reading the docs.
`eh!` — the proc macro. For the compiler.

## Usage

```rust
use terni::{eh, Imperfect, ConvergenceLoss};

fn pipeline(input: i32) -> Imperfect<i32, String, ConvergenceLoss> {
    eh! {
        let a = step_one(input)?;
        let b = step_two(a)?;
        b + 1
    }
}
```

Every `expr?` inside `eh!` routes through the `IntoEh` trait:

- **`Imperfect` values**: extracted via `Eh::eh()`, loss accumulated into a hidden context.
- **`Result` values**: passed through unchanged, no loss.

The final expression is wrapped with accumulated loss:
- All success, no loss: returns `Success(value)`
- Any loss accumulated: returns `Partial(value, accumulated_loss)`
- Any `?` hits `Failure` or `Err`: returns `Failure(error, accumulated_loss)`

## Mixing `Imperfect` and `Result`

Both types work inside `eh!`:

```rust
use terni::{eh, Imperfect, ConvergenceLoss};

fn mixed() -> Imperfect<String, String, ConvergenceLoss> {
    eh! {
        let data = Imperfect::<Vec<u8>, String, ConvergenceLoss>::Success(vec![72, 105])?;
        let text: String = Ok::<String, String>(String::from_utf8_lossy(&data).into())?;
        text
    }
}
```

The `IntoEh` trait handles dispatch at compile time. Zero-cost: monomorphized away.

## Nested Blocks

Each `eh!` block gets its own accumulation context:

```rust
use terni::{eh, Imperfect, ConvergenceLoss};

fn outer() -> Imperfect<i32, String, ConvergenceLoss> {
    eh! {
        let inner: Imperfect<i32, String, ConvergenceLoss> = eh! {
            let x = Imperfect::<i32, String, ConvergenceLoss>::Partial(10, ConvergenceLoss::new(2))?;
            x + 1
        };
        let v = inner?;
        v + 5
    }
}
```

The inner block produces `Partial(11, ConvergenceLoss(2))`. The outer block's `?` extracts the value and accumulates the inner loss.

## Recovery

Add a `recover` branch to handle partial results. If the body completes with accumulated loss (Partial), the recovery closure runs with the value and loss. The closure transforms the value; the loss stays unchanged. Success passes through untouched.

```rust
use terni::{eh, Imperfect, ConvergenceLoss};

fn adjusted(input: i32) -> Imperfect<i32, String, ConvergenceLoss> {
    eh! {
        let a = step_one(input)?;
        let b = step_two(a)?;
        b + 1

        recover |value, loss| {
            // 7-9: you got it, it cost something
            eprintln!("lost {} steps", loss.steps());
            value * 2  // adjust the value based on what was lost
        }
    }
}
```

The `recover` closure receives `(value, loss)` — the value from the body and the accumulated loss. It returns a new value. The loss itself is fact, not something you edit.

If no loss accumulated (Success), the `recover` branch is never executed.

## Rescue

Add a `rescue` branch to handle failures. If any `?` in the body hits `Failure`, the rescue closure runs with the error. The accumulated loss from the try body carries into the rescue. The result is always `Partial` — the failure happened.

```rust
use terni::{eh, Imperfect, ConvergenceLoss};

fn resilient(input: i32) -> Imperfect<i32, String, ConvergenceLoss> {
    eh! {
        let a = step_one(input)?;
        let b = step_two(a)?;
        b + 1

        rescue |e| {
            // 6-: the MC makes a move
            eprintln!("failed: {}", e);
            0  // fallback value
        }
    }
}
```

Without `rescue`, a `Failure` propagates as `Failure(error, accumulated_loss)`. With `rescue`, it becomes `Partial(rescued_value, accumulated_loss)`.

The rescue closure receives the error value. Use it or ignore it:

```rust
use terni::{eh, Imperfect, ConvergenceLoss};

fn with_error_info() -> Imperfect<String, String, ConvergenceLoss> {
    eh! {
        let val = might_fail()?;
        format!("got: {}", val)

        rescue |e| {
            format!("rescued from: {}", e)
        }
    }
}
```

If no failure occurs, the `rescue` branch is never executed.

## Full PbtA Block

Both branches are optional and independent. Use one, both, or neither. When both are present, `recover` comes before `rescue`:

```rust
use terni::{eh, Imperfect, ConvergenceLoss};

fn full_pbta(input: i32) -> Imperfect<i32, String, ConvergenceLoss> {
    eh! {
        let a = step_one(input)?;
        step_two(a)

        // 7-9: you got it, it cost something
        recover |value, loss| {
            adjust(value, &loss)
        }

        // 6-: the MC makes a move
        rescue |error| {
            fallback(error)
        }
    }
}

// 10+: clean hit. no handler needed.
```

## How It Works

The `eh!` proc macro rewrites the block:

1. Creates a hidden `Eh` context (`__eh_ctx`)
2. Wraps the block body in a closure returning `Result`
3. Rewrites every `expr?` to `IntoEh::into_eh(expr, &mut __eh_ctx)?`
4. Wraps the final expression in `Ok(...)`
5. Matches the closure result: `Ok` checks accumulated loss, `Err` calls `failure_with_loss()`
6. With `recover`: `Ok` with loss runs the recover closure with `(value, loss)`, returns `Partial(new_value, loss)`
7. With `rescue`: `Err` builds a `Failure`, then calls `unwrap_or_else()` with the rescue closure — always producing `Partial`

## Limitations

**`return` returns from the block, not the enclosing function.** The macro wraps your code in a closure. `return` exits that closure, not your function. Use `?` for early exit from `eh!` blocks.

**No `async` support.** The closure wrapper doesn't interact well with `.await`. This needs design work and will come in a future release.

**`?` in closures accumulates to the same context.** If you use `?` inside a closure within `eh!`, loss accumulates into the outer block's context. This is usually what you want.

**`__eh_ctx` name collision.** The macro generates an internal variable named `__eh_ctx`. If your code uses a variable with this exact name, it will collide. The double-underscore prefix makes this unlikely in practice, but it is not hygienically scoped (proc macros operate at call-site span).

**`recover` / `rescue` as identifiers.** The parser detects `recover` and `rescue` keywords by scanning for a top-level identifier followed by `|`. If you have a variable named `recover` or `rescue` followed by a bitwise OR (`|`), the parser will misidentify it as a branch keyword. Rename the variable to avoid ambiguity.

**`needless_question_mark` clippy lint.** When the tail expression of an `eh!` block is `expr?`, the macro expansion produces `Ok(IntoEh::into_eh(expr, ctx)?)` which clippy flags as redundant. This is inherent to the rewriting strategy and does not affect correctness. Suppress with `#[allow(clippy::needless_question_mark)]` on the enclosing function if needed.

## The `IntoEh` Trait

The trait that makes `eh!` work:

```rust
pub trait IntoEh<T, E, L: Loss> {
    fn into_eh(self, ctx: &mut Eh<L>) -> Result<T, E>;
}
```

Implemented for:
- `Imperfect<T, E, L>` -- extracts value, accumulates loss into context
- `Result<T, E>` -- passes through unchanged

You can implement `IntoEh` for your own types to make them work with `eh!`.

## Feature Flag

The macro is behind the `macros` feature, which is on by default:

```toml
[dependencies]
terni = "0.5"  # macros included

# or explicitly:
terni = { version = "0.5", features = ["macros"] }

# without macros:
terni = { version = "0.5", default-features = false }
```
