# The `eh!` Macro

Block macro for implicit loss accumulation with `?`.

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

## How It Works

The `eh!` proc macro rewrites the block:

1. Creates a hidden `Eh` context (`__eh_ctx`)
2. Wraps the block body in a closure returning `Result`
3. Rewrites every `expr?` to `IntoEh::into_eh(expr, &mut __eh_ctx)?`
4. Wraps the final expression in `Ok(...)`
5. Matches the closure result: `Ok` calls `finish()`, `Err` calls `failure_with_loss()`

## Limitations

**`return` returns from the block, not the enclosing function.** The macro wraps your code in a closure. `return` exits that closure, not your function. Use `?` for early exit from `eh!` blocks.

**No `async` support.** The closure wrapper doesn't interact well with `.await`. This needs design work and will come in a future release.

**`?` in closures accumulates to the same context.** If you use `?` inside a closure within `eh!`, loss accumulates into the outer block's context. This is usually what you want.

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
