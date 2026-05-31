//! Structured loss as substrate-located opacities.
//!
//! [`Transparency<P>`] is a [`Loss`] monoid whose values are either
//! [`Transparency::Clear`] (no opacity — empty light) or
//! [`Transparency::Opaque`] (a `BTreeMap` from substrate location `P` to a
//! per-location [`PropertyVerdict`]). The `combine` operation **unions** the
//! opacity maps at the same path via [`PropertyVerdict::merge_with`], which
//! is the structural realisation of Beer's "audit channel" (System 3*)
//! propagating *located* trouble through the system rather than collapsing
//! it to a scalar.
//!
//! ## Catastrophic absorption
//!
//! `Transparency::Opaque(BTreeMap::new())` — `Opaque` with no entries — is
//! the catastrophic sentinel returned by [`Loss::total`]. It is the
//! absorbing element under `combine`: anything combined with it returns it.
//! Public constructors hide this footgun:
//!
//! - [`Transparency::clear`] — the identity (no opacity).
//! - [`Transparency::single`] — a single located opacity.
//! - [`Transparency::opaque`] — a single located opacity (canonical
//!   constructor name; structurally identical to `single`).
//! - [`Transparency::catastrophic`] — the absorbing element.
//!
//! The `Opaque(BTreeMap)` variant is `pub(crate)`: outside the
//! `terni::transparency` module the variant cannot be named directly, so
//! `Transparency::Opaque(BTreeMap::new())` is not constructible from
//! downstream code. The catastrophic sentinel can only arise through
//! [`Loss::total`] or [`Transparency::catastrophic`] (explicit) or via
//! [`Loss::combine`]-driven absorption from within the crate (legit, by
//! composition). Per Seam I1 (pre-merge adversarial review, 2026-05-30):
//! the invariant moves from "doc comment" to type system.
//!
//! ## Why not `P: Default`
//!
//! There is **no** `P : Default` bound. The identity element is structural
//! (`Clear` carries nothing); the absorbing element is structural too
//! (`Opaque(empty)`). Downstream substrate-reference types — notably
//! `prism_core::Ref` — can't sensibly have a `Default` impl (an "empty"
//! `@`-prefixed ref is meaningless), and the Default bound on the path
//! type would force every consumer to invent one. The enum encoding
//! sidesteps that entirely.
//!
//! ## Citation
//!
//! The "located opacity" framing follows R. Reyes' 2024 reconstruction of
//! Beer's Viable System Model, which formalises the audit channel (VSM
//! System 3*) as a sheaf-of-troubles indexed by substrate locations rather
//! than a single scalar of "how broken." See the systemic.engineering
//! corpus, `cybernetics/beer-error-propagation.md`.

use std::collections::BTreeMap;

use crate::Loss;

// ---------------------------------------------------------------------------
// OpacityMap — newtype around the BTreeMap inside Transparency::Opaque.
// ---------------------------------------------------------------------------

/// The opacities map carried inside [`Transparency::Opaque`]. Newtype
/// over `BTreeMap<P, PropertyVerdict>` with a `pub(crate)` constructor:
/// outside the `terni::transparency` module nothing can construct an
/// `OpacityMap` directly, so the catastrophic-by-empty-map sentinel
/// cannot be forged from downstream code.
///
/// Per Seam I1 (pre-merge adversarial review, 2026-05-30): the
/// invariant moves from "doc comment" to type system.
#[derive(Clone, Debug, PartialEq)]
pub struct OpacityMap<P: Ord + Clone>(pub(crate) BTreeMap<P, PropertyVerdict>);

impl<P: Ord + Clone> OpacityMap<P> {
    /// Borrow the underlying map.
    pub fn as_btreemap(&self) -> &BTreeMap<P, PropertyVerdict> {
        &self.0
    }

    /// True iff this map has no entries (the catastrophic-sentinel shape).
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Number of opacities in the map.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// True iff there's a verdict at `path`.
    pub fn contains_key(&self, path: &P) -> bool {
        self.0.contains_key(path)
    }
}

impl<P: Ord + Clone> std::ops::Index<&P> for OpacityMap<P> {
    type Output = PropertyVerdict;
    fn index(&self, key: &P) -> &PropertyVerdict {
        &self.0[key]
    }
}

// ---------------------------------------------------------------------------
// Diagnostic — a small named message accompanying a verdict.
// ---------------------------------------------------------------------------

/// A small human-readable message accompanying a [`PropertyVerdict`].
/// Newtype over `String` to keep bare strings out of the verdict surface
/// (no-bare-types).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Diagnostic(String);

impl Diagnostic {
    /// Construct a diagnostic from any string-like value.
    pub fn new(msg: impl Into<String>) -> Self {
        Diagnostic(msg.into())
    }

    /// Borrow the inner message.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// PropertyVerdict — per-substrate-location structured verdict.
// ---------------------------------------------------------------------------

/// The verdict carried for one substrate location in a
/// [`Transparency::Opaque`] map.
///
/// Three shapes:
///
/// - `Pass` — this location passed. Doesn't usually appear inside an
///   `Opaque` map (those carry only the non-clear locations) but is a legal
///   value so external assembly code can use the same enum uniformly.
/// - `Partial { confidence, diagnostics }` — this location passed with
///   bounded confidence and one or more accumulated diagnostics.
/// - `Fail(Diagnostic)` — this location failed outright.
///
/// `merge_with` is the per-location combine: `Fail` dominates everything,
/// otherwise `Partial`s merge their diagnostics (and take the *minimum*
/// confidence — confidence only goes down through accumulation).
#[derive(Clone, Debug, PartialEq)]
pub enum PropertyVerdict {
    /// This property passed without qualification.
    Pass,
    /// This property passed with bounded confidence and accumulated
    /// diagnostics.
    Partial {
        /// In `[0.0, 1.0]`. Lower = less confident.
        confidence: f64,
        /// Accumulated diagnostics from the path through this location.
        diagnostics: Vec<Diagnostic>,
    },
    /// This property failed outright. `Fail` dominates under `merge_with`.
    Fail(Diagnostic),
}

impl PropertyVerdict {
    /// Merge `other` into `self` per the per-location combine semantics:
    ///
    /// - `Fail` dominates (a `Fail` on either side absorbs the other).
    /// - `Partial + Partial` combines diagnostics and takes the minimum
    ///   confidence (confidence only goes down through accumulation).
    /// - `Pass` is the neutral element on either side.
    pub fn merge_with(&mut self, other: &Self) {
        use PropertyVerdict::*;
        match (&*self, other) {
            (Fail(_), _) => { /* Fail dominates; no change */ }
            (_, Fail(d)) => {
                *self = Fail(d.clone());
            }
            (
                Partial {
                    confidence: c1,
                    diagnostics: ds1,
                },
                Partial {
                    confidence: c2,
                    diagnostics: ds2,
                },
            ) => {
                let mut combined = ds1.clone();
                combined.extend(ds2.iter().cloned());
                *self = Partial {
                    confidence: c1.min(*c2),
                    diagnostics: combined,
                };
            }
            // Pass cases — shouldn't usually arise in Opaque maps; the
            // non-Pass side wins.
            (Pass, other) => {
                *self = other.clone();
            }
            (_, Pass) => { /* no change */ }
        }
    }
}

// ---------------------------------------------------------------------------
// verdict_union — the BTreeMap-level combine helper.
// ---------------------------------------------------------------------------

/// Union two opacity maps, merging colliding verdicts via
/// [`PropertyVerdict::merge_with`].
pub fn verdict_union<P: Ord + Clone>(
    mut a: BTreeMap<P, PropertyVerdict>,
    b: BTreeMap<P, PropertyVerdict>,
) -> BTreeMap<P, PropertyVerdict> {
    for (path, verdict) in b {
        a.entry(path)
            .and_modify(|v| v.merge_with(&verdict))
            .or_insert(verdict);
    }
    a
}

// ---------------------------------------------------------------------------
// Transparency — the Loss monoid of structured opacities.
// ---------------------------------------------------------------------------

/// Structured loss: empty light (`Clear`) or accumulated opacities at
/// substrate locations (`Opaque`).
///
/// `Opaque(empty_map)` is the catastrophic sentinel returned by
/// [`Loss::total`]. Public constructors ([`Transparency::clear`],
/// [`Transparency::single`], [`Transparency::opaque`],
/// [`Transparency::catastrophic`]) hide this footgun — the inner
/// [`OpacityMap`] has a `pub(crate)` constructor, so
/// `Transparency::Opaque(OpacityMap(BTreeMap::new()))` is unreachable
/// from outside the crate. Forging the catastrophic sentinel from
/// downstream code is now structurally impossible.
///
/// `P` is the substrate-location type. There is **no** `P : Default`
/// bound — the identity element is structural (`Clear` carries nothing).
/// Co-discovery with R. Reyes 2024 on Beer's audit channel (System 3*);
/// see `cybernetics/beer-error-propagation.md` in the
/// systemic.engineering corpus.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Transparency<P: Ord + Clone> {
    /// No opacity. The Loss identity. Combining `Clear` with anything
    /// yields the other side unchanged.
    #[default]
    Clear,
    /// Accumulated opacities at substrate locations. An empty map is the
    /// catastrophic sentinel — see [`Loss::total`].
    ///
    /// Wraps [`OpacityMap`]: outside the crate the inner map is not
    /// constructible (the [`OpacityMap`] tuple field is `pub(crate)`),
    /// so downstream callers must go through
    /// [`Transparency::opaque`] / [`Transparency::single`] /
    /// [`Transparency::catastrophic`]. Per Seam I1.
    Opaque(OpacityMap<P>),
}

impl<P: Ord + Clone> Transparency<P> {
    /// The identity element. Same as `Loss::zero()` but inherent so
    /// callers don't need the `Loss` trait in scope.
    pub fn clear() -> Self {
        Transparency::Clear
    }

    /// The catastrophic sentinel — `Opaque` with no entries. Absorbing
    /// element under `combine`.
    pub fn catastrophic() -> Self {
        Transparency::Opaque(OpacityMap(BTreeMap::new()))
    }

    /// A single located opacity.
    pub fn single(path: P, verdict: PropertyVerdict) -> Self {
        Self::opaque(path, verdict)
    }

    /// A single located opacity — canonical constructor name.
    ///
    /// Per Seam I1 (2026-05-30): the inner [`OpacityMap`] is
    /// constructible only inside the `terni` crate, so outside callers
    /// cannot write `Transparency::Opaque(OpacityMap(BTreeMap))`
    /// directly. `opaque` is the named public constructor; it takes a
    /// required first `(path, verdict)` pair, making empty-map forge
    /// structurally impossible at the call site. For the legitimate
    /// catastrophic-by-composition case, [`Loss::combine`] still
    /// produces an empty `Opaque` via crate-internal absorption — that
    /// path is part of the monoid semantics and is intended.
    pub fn opaque(path: P, verdict: PropertyVerdict) -> Self {
        let mut m = BTreeMap::new();
        m.insert(path, verdict);
        Transparency::Opaque(OpacityMap(m))
    }

    /// Borrow the opacities map. Returns `None` for `Clear`.
    pub fn opacities(&self) -> Option<&BTreeMap<P, PropertyVerdict>> {
        match self {
            Transparency::Clear => None,
            Transparency::Opaque(m) => Some(&m.0),
        }
    }

    /// True iff this value is the catastrophic sentinel
    /// (`Opaque` with no entries).
    pub fn is_catastrophic(&self) -> bool {
        matches!(self, Transparency::Opaque(m) if m.is_empty())
    }

    /// True iff this value is `Opaque` (catastrophic or not).
    pub fn is_opaque(&self) -> bool {
        matches!(self, Transparency::Opaque(_))
    }

    /// True iff this value is `Opaque` and has a verdict at `path`.
    pub fn is_opaque_at(&self, path: &P) -> bool {
        match self {
            Transparency::Clear => false,
            Transparency::Opaque(m) => m.contains_key(path),
        }
    }
}

impl<P: Ord + Clone> Loss for Transparency<P> {
    fn zero() -> Self {
        Transparency::Clear
    }

    fn total() -> Self {
        // Catastrophic: opaque, no locatable structure. Absorbs under
        // combine.
        Transparency::Opaque(OpacityMap(BTreeMap::new()))
    }

    fn is_zero(&self) -> bool {
        matches!(self, Transparency::Clear)
    }

    fn combine(self, other: Self) -> Self {
        use Transparency::*;
        match (self, other) {
            (Clear, x) | (x, Clear) => x,
            // Either side empty-Opaque = catastrophic = absorbs.
            (Opaque(m), _) | (_, Opaque(m)) if m.is_empty() => {
                Opaque(OpacityMap(BTreeMap::new()))
            }
            (Opaque(m1), Opaque(m2)) => Opaque(OpacityMap(verdict_union(m1.0, m2.0))),
        }
    }
}
