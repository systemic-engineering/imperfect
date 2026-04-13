//! Proc macros for the `terni` crate.
//!
//! Provides the `eh!` block macro for implicit loss accumulation with `?`.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::visit_mut::{self, VisitMut};
use syn::{parse_macro_input, ExprTry, Stmt};

struct EhRewriter;

impl VisitMut for EhRewriter {
    fn visit_expr_try_mut(&mut self, node: &mut ExprTry) {
        // First, visit nested expressions (handles chained ?)
        visit_mut::visit_expr_try_mut(self, node);

        // Rewrite: expr? → IntoEh::into_eh(expr, &mut __eh_ctx)?
        let inner = &node.expr;
        node.expr = Box::new(syn::parse_quote! {
            ::terni::IntoEh::into_eh(#inner, &mut __eh_ctx)
        });
    }
}

/// Accumulate loss implicitly through `?` on `Imperfect` values.
///
/// Rewrites every `expr?` inside the block to route through an [`IntoEh`]
/// trait call, which accumulates loss for `Imperfect` values and passes
/// `Result` values through unchanged.
///
/// # Example
///
/// ```ignore
/// use terni::{eh, Imperfect, ConvergenceLoss};
///
/// fn process(input: i32) -> Imperfect<i32, String, ConvergenceLoss> {
///     eh! {
///         let a = Imperfect::<i32, String, ConvergenceLoss>::Success(input)?;
///         let b = Imperfect::Partial(a + 1, ConvergenceLoss::new(3))?;
///         b + 1
///     }
/// }
/// ```
///
/// # Limitations
///
/// - `return` inside an `eh!` block returns from the block, not the
///   enclosing function. Use `?` for early exit, not `return`.
#[proc_macro]
pub fn eh(input: TokenStream) -> TokenStream {
    let stmts = parse_macro_input!(input with parse_block_body);
    let mut stmts = stmts;

    // Rewrite all ? operators in each statement
    let mut rewriter = EhRewriter;
    for stmt in &mut stmts {
        syn::visit_mut::visit_stmt_mut(&mut rewriter, stmt);
    }

    // Split: all but last are prefix statements, last is the tail expression
    // that gets wrapped in Ok(...)
    let (prefix, tail) = if stmts.is_empty() {
        (vec![], quote! { () })
    } else {
        let last = stmts.pop().unwrap();
        let tail = match last {
            Stmt::Expr(expr, None) => quote! { #expr },
            other => {
                // Last statement is not a tail expression (e.g. `let x = ...;`)
                // Put it back and use unit as tail
                stmts.push(other);
                quote! { () }
            }
        };
        (stmts, tail)
    };

    // DELIBERATELY BROKEN: always returns Success, ignoring accumulated loss
    let output = quote! {{
        let mut __eh_ctx = ::terni::Eh::new();
        let __eh_result: ::core::result::Result<_, _> = (|| {
            #(#prefix)*
            ::core::result::Result::Ok(#tail)
        })();
        match __eh_result {
            ::core::result::Result::Ok(__eh_val) => ::terni::Imperfect::Success(__eh_val),
            ::core::result::Result::Err(__eh_err) => {
                ::terni::Imperfect::failure_with_loss(
                    __eh_err,
                    __eh_ctx.into_loss().unwrap_or_else(::terni::Loss::zero),
                )
            }
        }
    }};

    output.into()
}

/// Parse the body of a block (sequence of statements) without requiring braces.
///
/// Uses syn's Block parser by wrapping the input in braces, which correctly
/// handles the tail expression (last expression without semicolon).
fn parse_block_body(input: syn::parse::ParseStream) -> syn::Result<Vec<Stmt>> {
    // Collect all remaining tokens
    let body: TokenStream2 = input.parse()?;
    // Wrap in braces so syn::Block can parse it (it expects { ... })
    let wrapped: TokenStream2 = quote! { { #body } };
    let block: syn::Block = syn::parse2(wrapped)?;
    Ok(block.stmts)
}
