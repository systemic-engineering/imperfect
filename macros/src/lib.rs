//! Proc macros for the `terni` crate.
//!
//! Provides the `eh!` block macro for implicit loss accumulation with `?`.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::visit_mut::{self, VisitMut};
use syn::{parse_macro_input, Block, ExprTry, Ident, Stmt};

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

/// The recovery branch: `recover |ident| { body }`.
struct RecoverBranch {
    error_ident: Ident,
    body: Block,
}

/// Parsed input for the `eh!` macro.
struct EhInput {
    body: Vec<Stmt>,
    recover: Option<RecoverBranch>,
}

impl Parse for EhInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Collect all tokens, then scan for a top-level `recover` identifier
        // followed by `|`. We need to split the token stream at that point.
        let all_tokens: TokenStream2 = input.parse()?;
        let tokens: Vec<proc_macro2::TokenTree> = all_tokens.into_iter().collect();

        // Find the position of `recover` followed by `|`
        let recover_pos = find_recover_position(&tokens);

        match recover_pos {
            None => {
                // No recover branch — parse everything as statements
                let stmts = parse_stmts_from_tokens(&tokens)?;
                Ok(EhInput {
                    body: stmts,
                    recover: None,
                })
            }
            Some(pos) => {
                // Split at recover position
                let body_tokens: TokenStream2 = tokens[..pos].iter().cloned().collect();
                let recover_tokens: TokenStream2 = tokens[pos..].iter().cloned().collect();

                let body_stmts = parse_stmts_from_stream(body_tokens)?;
                let recover_branch: RecoverBranch = syn::parse2(recover_tokens)?;

                Ok(EhInput {
                    body: body_stmts,
                    recover: Some(recover_branch),
                })
            }
        }
    }
}

impl Parse for RecoverBranch {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Expect: recover |ident| { body }
        let keyword: Ident = input.parse()?;
        if keyword != "recover" {
            return Err(syn::Error::new(keyword.span(), "expected `recover`"));
        }

        // Parse |ident|
        input.parse::<syn::Token![|]>()?;
        let error_ident: Ident = input.parse()?;
        input.parse::<syn::Token![|]>()?;

        // Parse { body }
        let body: Block = input.parse()?;

        Ok(RecoverBranch { error_ident, body })
    }
}

/// Find the position of a top-level `recover` identifier that is followed by `|`.
fn find_recover_position(tokens: &[proc_macro2::TokenTree]) -> Option<usize> {
    for i in 0..tokens.len() {
        if let proc_macro2::TokenTree::Ident(ref ident) = tokens[i] {
            if ident == "recover" {
                // Check that next token is `|`
                if i + 1 < tokens.len() {
                    if let proc_macro2::TokenTree::Punct(ref p) = tokens[i + 1] {
                        if p.as_char() == '|' {
                            return Some(i);
                        }
                    }
                }
            }
        }
        // Don't recurse into groups — `recover` inside braces/parens is user code
    }
    None
}

fn parse_stmts_from_tokens(tokens: &[proc_macro2::TokenTree]) -> syn::Result<Vec<Stmt>> {
    let stream: TokenStream2 = tokens.iter().cloned().collect();
    parse_stmts_from_stream(stream)
}

fn parse_stmts_from_stream(stream: TokenStream2) -> syn::Result<Vec<Stmt>> {
    let wrapped: TokenStream2 = quote! { { #stream } };
    let block: Block = syn::parse2(wrapped)?;
    Ok(block.stmts)
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
/// # Recovery
///
/// Add a `recover |e| { ... }` branch to handle failures:
///
/// ```ignore
/// use terni::{eh, Imperfect, ConvergenceLoss};
///
/// fn process(input: i32) -> Imperfect<i32, String, ConvergenceLoss> {
///     eh! {
///         let a = step_one(input)?;
///         let b = step_two(a)?;
///         b + 1
///
///         recover |e| {
///             fallback(e)
///         }
///     }
/// }
/// ```
///
/// If the try body hits `Failure` (via `?`), the recovery closure runs with
/// the error. The accumulated loss carries into the recovery. The result is
/// always `Partial` — the failure happened. If no failure occurs, the recover
/// branch is never executed.
///
/// # Limitations
///
/// - `return` inside an `eh!` block returns from the block, not the
///   enclosing function. Use `?` for early exit, not `return`.
#[proc_macro]
pub fn eh(input: TokenStream) -> TokenStream {
    let eh_input = parse_macro_input!(input as EhInput);
    let mut stmts = eh_input.body;

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

    let err_branch = match eh_input.recover {
        None => {
            // No recover: failure path unchanged
            quote! {
                ::core::result::Result::Err(__eh_err) => {
                    ::terni::Imperfect::failure_with_loss(
                        __eh_err,
                        __eh_ctx.into_loss().unwrap_or_else(::terni::Loss::zero),
                    )
                }
            }
        }
        Some(recover) => {
            let error_ident = &recover.error_ident;
            let recover_stmts = &recover.body.stmts;
            // Build a Failure with accumulated loss, then use unwrap_or_else to recover.
            // This constrains E through failure_with_loss and unwrap_or_else signatures,
            // and preserves accumulated loss from the try body into the Partial result.
            quote! {
                ::core::result::Result::Err(__eh_err) => {
                    let __eh_loss = __eh_ctx.into_loss().unwrap_or_else(::terni::Loss::zero);
                    ::terni::Imperfect::failure_with_loss(__eh_err, __eh_loss)
                        .unwrap_or_else(|#error_ident| {
                            #(#recover_stmts)*
                        })
                }
            }
        }
    };

    let output = quote! {{
        let mut __eh_ctx = ::terni::Eh::new();
        let __eh_result: ::core::result::Result<_, _> = (|| {
            #(#prefix)*
            ::core::result::Result::Ok(#tail)
        })();
        match __eh_result {
            ::core::result::Result::Ok(__eh_val) => __eh_ctx.finish(__eh_val),
            #err_branch
        }
    }};

    output.into()
}
