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

/// The recover branch: `recover |value, loss| { body }`.
/// Handles Partial results — the 7-9 move.
struct RecoverBranch {
    value_ident: Ident,
    loss_ident: Ident,
    body: Block,
}

/// The rescue branch: `rescue |error| { body }`.
/// Handles Failure — the 6- move.
struct RescueBranch {
    error_ident: Ident,
    body: Block,
}

/// Parsed input for the `eh!` macro.
struct EhInput {
    body: Vec<Stmt>,
    recover: Option<RecoverBranch>,
    rescue: Option<RescueBranch>,
}

impl Parse for EhInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Collect all tokens, then scan for top-level `recover` and `rescue` identifiers
        // followed by `|`. We need to split the token stream at those points.
        let all_tokens: TokenStream2 = input.parse()?;
        let tokens: Vec<proc_macro2::TokenTree> = all_tokens.into_iter().collect();

        // Find positions of `recover` and `rescue` keywords
        let recover_pos = find_keyword_position(&tokens, "recover");
        let rescue_pos = find_keyword_position(&tokens, "rescue");

        match (recover_pos, rescue_pos) {
            (None, None) => {
                // No recover or rescue branch — parse everything as statements
                let stmts = parse_stmts_from_tokens(&tokens)?;
                Ok(EhInput {
                    body: stmts,
                    recover: None,
                    rescue: None,
                })
            }
            (None, Some(resc_pos)) => {
                // Only rescue branch
                let body_tokens: TokenStream2 = tokens[..resc_pos].iter().cloned().collect();
                let rescue_tokens: TokenStream2 = tokens[resc_pos..].iter().cloned().collect();

                let body_stmts = parse_stmts_from_stream(body_tokens)?;
                let rescue_branch: RescueBranch = syn::parse2(rescue_tokens)?;

                Ok(EhInput {
                    body: body_stmts,
                    recover: None,
                    rescue: Some(rescue_branch),
                })
            }
            (Some(rec_pos), None) => {
                // Only recover branch
                let body_tokens: TokenStream2 = tokens[..rec_pos].iter().cloned().collect();
                let recover_tokens: TokenStream2 = tokens[rec_pos..].iter().cloned().collect();

                let body_stmts = parse_stmts_from_stream(body_tokens)?;
                let recover_branch: RecoverBranch = syn::parse2(recover_tokens)?;

                Ok(EhInput {
                    body: body_stmts,
                    recover: Some(recover_branch),
                    rescue: None,
                })
            }
            (Some(rec_pos), Some(resc_pos)) => {
                // Both branches: recover must come before rescue
                if rec_pos >= resc_pos {
                    return Err(syn::Error::new(
                        proc_macro2::Span::call_site(),
                        "`recover` must appear before `rescue`",
                    ));
                }

                let body_tokens: TokenStream2 = tokens[..rec_pos].iter().cloned().collect();
                let recover_tokens: TokenStream2 =
                    tokens[rec_pos..resc_pos].iter().cloned().collect();
                let rescue_tokens: TokenStream2 = tokens[resc_pos..].iter().cloned().collect();

                let body_stmts = parse_stmts_from_stream(body_tokens)?;
                let recover_branch: RecoverBranch = syn::parse2(recover_tokens)?;
                let rescue_branch: RescueBranch = syn::parse2(rescue_tokens)?;

                Ok(EhInput {
                    body: body_stmts,
                    recover: Some(recover_branch),
                    rescue: Some(rescue_branch),
                })
            }
        }
    }
}

impl Parse for RecoverBranch {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Expect: recover |value, loss| { body }
        let keyword: Ident = input.parse()?;
        if keyword != "recover" {
            return Err(syn::Error::new(keyword.span(), "expected `recover`"));
        }

        // Parse |value, loss|
        input.parse::<syn::Token![|]>()?;
        let value_ident: Ident = input.parse()?;
        input.parse::<syn::Token![,]>()?;
        let loss_ident: Ident = input.parse()?;
        input.parse::<syn::Token![|]>()?;

        // Parse { body }
        let body: Block = input.parse()?;

        Ok(RecoverBranch {
            value_ident,
            loss_ident,
            body,
        })
    }
}

impl Parse for RescueBranch {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Expect: rescue |ident| { body }
        let keyword: Ident = input.parse()?;
        if keyword != "rescue" {
            return Err(syn::Error::new(keyword.span(), "expected `rescue`"));
        }

        // Parse |ident|
        input.parse::<syn::Token![|]>()?;
        let error_ident: Ident = input.parse()?;
        input.parse::<syn::Token![|]>()?;

        // Parse { body }
        let body: Block = input.parse()?;

        Ok(RescueBranch { error_ident, body })
    }
}

/// Find the position of a top-level keyword identifier that is followed by `|`.
fn find_keyword_position(tokens: &[proc_macro2::TokenTree], keyword: &str) -> Option<usize> {
    for i in 0..tokens.len() {
        if let proc_macro2::TokenTree::Ident(ref ident) = tokens[i] {
            if ident == keyword {
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
        // Don't recurse into groups — keywords inside braces/parens are user code
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
/// Add a `recover |value, loss| { ... }` branch to handle partial results:
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
///         recover |value, loss| {
///             adjust(value, &loss)
///         }
///     }
/// }
/// ```
///
/// # Rescue
///
/// Add a `rescue |e| { ... }` branch to handle failures:
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
///         rescue |e| {
///             fallback(e)
///         }
///     }
/// }
/// ```
///
/// If the try body hits `Failure` (via `?`), the rescue closure runs with
/// the error. The accumulated loss carries into the rescue. The result is
/// always `Partial` — the failure happened. If no failure occurs, the rescue
/// branch is never executed.
///
/// # Full PbtA Block
///
/// ```ignore
/// eh! {
///     let a = step_one(input)?;
///     step_two(a)
///
///     // 7-9: you got it, it cost something
///     recover |value, loss| {
///         adjust(value, &loss)
///     }
///
///     // 6-: the MC makes a move
///     rescue |error| {
///         fallback(error)
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

    // Build the Ok branch: handles Success and Partial from finish()
    let ok_branch = match &eh_input.recover {
        None => {
            // No recover: finish() result passes through
            quote! {
                ::core::result::Result::Ok(__eh_val) => __eh_ctx.finish(__eh_val),
            }
        }
        Some(recover) => {
            let value_ident = &recover.value_ident;
            let loss_ident = &recover.loss_ident;
            let recover_stmts = &recover.body.stmts;
            // With recover: check accumulated loss, apply recover closure if Partial
            quote! {
                ::core::result::Result::Ok(__eh_val) => {
                    match __eh_ctx.into_loss() {
                        ::core::option::Option::None => {
                            ::terni::Imperfect::Success(__eh_val)
                        },
                        ::core::option::Option::Some(__eh_l) => {
                            let #value_ident = __eh_val;
                            let #loss_ident = __eh_l.clone();
                            let __eh_recovered = { #(#recover_stmts)* };
                            ::terni::Imperfect::Partial(__eh_recovered, __eh_l)
                        },
                    }
                },
            }
        }
    };

    // Build the Err branch: handles Failure
    let err_branch = match &eh_input.rescue {
        None => {
            // No rescue: failure path unchanged
            quote! {
                ::core::result::Result::Err(__eh_err) => {
                    ::terni::Imperfect::failure_with_loss(
                        __eh_err,
                        __eh_ctx.into_loss().unwrap_or_else(::terni::Loss::zero),
                    )
                }
            }
        }
        Some(rescue) => {
            let error_ident = &rescue.error_ident;
            let rescue_stmts = &rescue.body.stmts;
            quote! {
                ::core::result::Result::Err(__eh_err) => {
                    let __eh_loss = __eh_ctx.into_loss().unwrap_or_else(::terni::Loss::zero);
                    ::terni::Imperfect::failure_with_loss(__eh_err, __eh_loss)
                        .unwrap_or_else(|#error_ident| {
                            #(#rescue_stmts)*
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
            #ok_branch
            #err_branch
        }
    }};

    output.into()
}
