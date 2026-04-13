//! Proc macros for the `terni` crate.
//!
//! Provides the `eh!` block macro for implicit loss accumulation with `?`.

use proc_macro::TokenStream;

/// Accumulate loss implicitly through `?` on `Imperfect` values.
///
/// Rewrites every `expr?` inside the block to route through an [`IntoEh`]
/// trait call, which accumulates loss for `Imperfect` values and passes
/// `Result` values through unchanged.
///
/// # Limitations
///
/// - `return` inside an `eh!` block returns from the block, not the
///   enclosing function. Use `?` for early exit, not `return`.
#[proc_macro]
pub fn eh(input: TokenStream) -> TokenStream {
    let _ = input;
    // Scaffold — implementation in Task 3
    TokenStream::new()
}
