//! Structured loss as substrate-located opacities. STUB — failing on purpose.
//!
//! Real implementation arrives in the paired 🟢 commit. The stubs compile so
//! the 🔴 test suite can run, but their bodies are deliberately wrong (or
//! `todo!()`) so every test asserting real semantics fails.

use std::collections::BTreeMap;

use crate::Loss;

/// Stub diagnostic — see 🟢 commit.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Diagnostic(String);

impl Diagnostic {
    /// Stub constructor — `todo!` in the 🔴 build.
    pub fn new(_msg: impl Into<String>) -> Self {
        todo!("Diagnostic::new — implement in 🟢")
    }

    /// Stub accessor — `todo!` in the 🔴 build.
    pub fn as_str(&self) -> &str {
        todo!("Diagnostic::as_str — implement in 🟢")
    }
}

/// Stub per-location verdict — see 🟢 commit.
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
    /// Stub merge — `todo!` in the 🔴 build.
    pub fn merge_with(&mut self, _other: &Self) {
        todo!("PropertyVerdict::merge_with — implement in 🟢")
    }
}

/// Stub map-union — drops `b` on purpose so tests fail.
pub fn verdict_union<P: Ord + Clone>(
    a: BTreeMap<P, PropertyVerdict>,
    _b: BTreeMap<P, PropertyVerdict>,
) -> BTreeMap<P, PropertyVerdict> {
    a
}

/// Stub structured loss — see 🟢 commit. Bodies are deliberately wrong.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Transparency<P: Ord + Clone> {
    /// Identity (no opacity).
    #[default]
    Clear,
    /// Accumulated opacities at substrate locations.
    Opaque(BTreeMap<P, PropertyVerdict>),
}

impl<P: Ord + Clone> Transparency<P> {
    /// Stub clear-constructor.
    pub fn clear() -> Self {
        Transparency::Clear
    }

    /// Stub catastrophic-constructor — wrong on purpose.
    pub fn catastrophic() -> Self {
        Transparency::Clear
    }

    /// Stub single-constructor — wrong on purpose.
    pub fn single(_path: P, _verdict: PropertyVerdict) -> Self {
        Transparency::Clear
    }

    /// Stub opacities accessor — wrong on purpose.
    pub fn opacities(&self) -> Option<&BTreeMap<P, PropertyVerdict>> {
        None
    }

    /// Stub catastrophic check — wrong on purpose.
    pub fn is_catastrophic(&self) -> bool {
        false
    }

    /// Stub opaque check — wrong on purpose.
    pub fn is_opaque(&self) -> bool {
        false
    }

    /// Stub per-path opaque check — wrong on purpose.
    pub fn is_opaque_at(&self, _path: &P) -> bool {
        false
    }
}

impl<P: Ord + Clone> Loss for Transparency<P> {
    fn zero() -> Self {
        Transparency::Clear
    }
    fn total() -> Self {
        // Wrong on purpose: return Clear so the absorbing-element tests fail.
        Transparency::Clear
    }
    fn is_zero(&self) -> bool {
        matches!(self, Transparency::Clear)
    }
    fn combine(self, _other: Self) -> Self {
        // Wrong on purpose: drop `other` so disjoint-union tests fail.
        self
    }
}
