//! Tests for `Transparency<P>` — the Loss monoid of substrate-located
//! opacities. Tick: prism/transparency 🔴.

use std::collections::BTreeMap;

use terni::{verdict_union, Diagnostic, Loss, PropertyVerdict, Transparency};

// ---------------------------------------------------------------------------
// PropertyVerdict::merge_with
// ---------------------------------------------------------------------------

#[test]
fn merge_with_fail_dominates_partial() {
    let mut a = PropertyVerdict::Partial {
        confidence: 0.9,
        diagnostics: vec![Diagnostic::new("first")],
    };
    let b = PropertyVerdict::Fail(Diagnostic::new("boom"));
    a.merge_with(&b);
    match a {
        PropertyVerdict::Fail(d) => assert_eq!(d.as_str(), "boom"),
        other => panic!("expected Fail, got {:?}", other),
    }
}

#[test]
fn merge_with_fail_self_stays_fail() {
    let mut a = PropertyVerdict::Fail(Diagnostic::new("first-boom"));
    let b = PropertyVerdict::Partial {
        confidence: 0.5,
        diagnostics: vec![Diagnostic::new("second")],
    };
    a.merge_with(&b);
    match a {
        PropertyVerdict::Fail(d) => assert_eq!(d.as_str(), "first-boom"),
        other => panic!("expected Fail unchanged, got {:?}", other),
    }
}

#[test]
fn merge_with_partials_unions_diagnostics_min_confidence() {
    let mut a = PropertyVerdict::Partial {
        confidence: 0.8,
        diagnostics: vec![Diagnostic::new("d1")],
    };
    let b = PropertyVerdict::Partial {
        confidence: 0.5,
        diagnostics: vec![Diagnostic::new("d2"), Diagnostic::new("d3")],
    };
    a.merge_with(&b);
    match a {
        PropertyVerdict::Partial {
            confidence,
            diagnostics,
        } => {
            assert!((confidence - 0.5).abs() < 1e-12, "min wins, got {confidence}");
            let strs: Vec<&str> = diagnostics.iter().map(|d| d.as_str()).collect();
            assert_eq!(strs, vec!["d1", "d2", "d3"]);
        }
        other => panic!("expected Partial, got {:?}", other),
    }
}

#[test]
fn merge_with_pass_is_neutral() {
    // Pass + Partial → Partial unchanged on the other side.
    let mut a = PropertyVerdict::Pass;
    let b = PropertyVerdict::Partial {
        confidence: 0.7,
        diagnostics: vec![Diagnostic::new("d")],
    };
    a.merge_with(&b);
    match a {
        PropertyVerdict::Partial {
            confidence,
            diagnostics,
        } => {
            assert!((confidence - 0.7).abs() < 1e-12);
            assert_eq!(diagnostics.len(), 1);
        }
        other => panic!("expected Partial, got {:?}", other),
    }

    // Partial + Pass → Partial unchanged.
    let mut c = PropertyVerdict::Partial {
        confidence: 0.6,
        diagnostics: vec![Diagnostic::new("c")],
    };
    let d = PropertyVerdict::Pass;
    c.merge_with(&d);
    match c {
        PropertyVerdict::Partial {
            confidence,
            diagnostics,
        } => {
            assert!((confidence - 0.6).abs() < 1e-12);
            assert_eq!(diagnostics.len(), 1);
        }
        other => panic!("expected Partial unchanged, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// verdict_union
// ---------------------------------------------------------------------------

#[test]
fn verdict_union_disjoint_keys_merge() {
    let mut a: BTreeMap<String, PropertyVerdict> = BTreeMap::new();
    a.insert(
        "@a".into(),
        PropertyVerdict::Partial {
            confidence: 0.9,
            diagnostics: vec![Diagnostic::new("a")],
        },
    );
    let mut b: BTreeMap<String, PropertyVerdict> = BTreeMap::new();
    b.insert(
        "@b".into(),
        PropertyVerdict::Fail(Diagnostic::new("b-boom")),
    );
    let merged = verdict_union(a, b);
    assert_eq!(merged.len(), 2);
    assert!(matches!(merged["@a"], PropertyVerdict::Partial { .. }));
    assert!(matches!(merged["@b"], PropertyVerdict::Fail(_)));
}

#[test]
fn verdict_union_colliding_keys_merge_via_merge_with() {
    let mut a: BTreeMap<String, PropertyVerdict> = BTreeMap::new();
    a.insert(
        "@x".into(),
        PropertyVerdict::Partial {
            confidence: 0.8,
            diagnostics: vec![Diagnostic::new("d1")],
        },
    );
    let mut b: BTreeMap<String, PropertyVerdict> = BTreeMap::new();
    b.insert(
        "@x".into(),
        PropertyVerdict::Fail(Diagnostic::new("boom")),
    );
    let merged = verdict_union(a, b);
    assert_eq!(merged.len(), 1);
    match &merged["@x"] {
        PropertyVerdict::Fail(d) => assert_eq!(d.as_str(), "boom"),
        other => panic!("expected Fail to dominate, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Transparency<P> — Clear/Opaque enum + Loss laws
// ---------------------------------------------------------------------------

#[test]
fn clear_is_zero_is_default() {
    let z: Transparency<String> = Transparency::zero();
    assert!(z.is_zero());
    assert!(matches!(z, Transparency::Clear));
    let d: Transparency<String> = Transparency::default();
    assert!(d.is_zero());
}

#[test]
fn catastrophic_is_total_and_opaque() {
    let t: Transparency<String> = Transparency::total();
    assert!(!t.is_zero());
    assert!(t.is_catastrophic());
    assert!(t.is_opaque());
    let c: Transparency<String> = Transparency::catastrophic();
    assert!(c.is_catastrophic());
    assert_eq!(t, c);
}

#[test]
fn single_constructs_opaque_with_one_entry() {
    let t = Transparency::single(
        "@p".to_string(),
        PropertyVerdict::Fail(Diagnostic::new("nope")),
    );
    let map = t.opacities().expect("single should produce Opaque");
    assert_eq!(map.len(), 1);
    assert!(map.contains_key("@p"));
}

#[test]
fn combine_clear_neutral_left() {
    let z: Transparency<String> = Transparency::clear();
    let t = Transparency::single(
        "@p".to_string(),
        PropertyVerdict::Fail(Diagnostic::new("x")),
    );
    let combined = z.combine(t.clone());
    assert_eq!(combined, t);
}

#[test]
fn combine_clear_neutral_right() {
    let z: Transparency<String> = Transparency::clear();
    let t = Transparency::single(
        "@p".to_string(),
        PropertyVerdict::Fail(Diagnostic::new("x")),
    );
    let combined = t.clone().combine(z);
    assert_eq!(combined, t);
}

#[test]
fn combine_two_opaques_unions_maps() {
    let a = Transparency::single(
        "@a".to_string(),
        PropertyVerdict::Partial {
            confidence: 0.9,
            diagnostics: vec![Diagnostic::new("a")],
        },
    );
    let b = Transparency::single(
        "@b".to_string(),
        PropertyVerdict::Fail(Diagnostic::new("b-boom")),
    );
    let merged = a.combine(b);
    let map = merged
        .opacities()
        .expect("combined opaques should be Opaque");
    assert_eq!(map.len(), 2, "disjoint keys union");
    assert!(map.contains_key("@a"));
    assert!(map.contains_key("@b"));
}

#[test]
fn combine_catastrophic_left_absorbs() {
    let cat: Transparency<String> = Transparency::catastrophic();
    let t = Transparency::single(
        "@p".to_string(),
        PropertyVerdict::Partial {
            confidence: 0.5,
            diagnostics: vec![],
        },
    );
    let out = cat.combine(t);
    assert!(out.is_catastrophic(), "catastrophic absorbs from the left");
}

#[test]
fn combine_catastrophic_right_absorbs() {
    let cat: Transparency<String> = Transparency::catastrophic();
    let t = Transparency::single(
        "@p".to_string(),
        PropertyVerdict::Partial {
            confidence: 0.5,
            diagnostics: vec![],
        },
    );
    let out = t.combine(cat);
    assert!(out.is_catastrophic(), "catastrophic absorbs from the right");
}

#[test]
fn combine_colliding_paths_merges_verdicts() {
    let a = Transparency::single(
        "@x".to_string(),
        PropertyVerdict::Partial {
            confidence: 0.9,
            diagnostics: vec![Diagnostic::new("d1")],
        },
    );
    let b = Transparency::single(
        "@x".to_string(),
        PropertyVerdict::Fail(Diagnostic::new("d2-boom")),
    );
    let merged = a.combine(b);
    let map = merged
        .opacities()
        .expect("combined opaques should be Opaque");
    assert_eq!(map.len(), 1);
    match &map["@x"] {
        PropertyVerdict::Fail(d) => assert_eq!(d.as_str(), "d2-boom"),
        other => panic!("expected Fail to dominate at collision, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// P is not Default — instantiate Transparency over a non-Default type to
// witness that the bound is gone.
// ---------------------------------------------------------------------------

/// A path-shaped type with NO `Default` impl. If Transparency required
/// `P: Default`, this test wouldn't compile.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct NoDefaultPath(String);

#[test]
fn transparency_does_not_require_p_default() {
    let p = NoDefaultPath("@no-default".to_string());
    let t: Transparency<NoDefaultPath> = Transparency::single(
        p.clone(),
        PropertyVerdict::Partial {
            confidence: 0.5,
            diagnostics: vec![Diagnostic::new("hi")],
        },
    );
    let z: Transparency<NoDefaultPath> = Transparency::zero();
    let cat: Transparency<NoDefaultPath> = Transparency::total();
    assert!(z.is_zero());
    assert!(cat.is_catastrophic());
    assert!(t.is_opaque_at(&p));
}

// ---------------------------------------------------------------------------
// Loss laws sanity
// ---------------------------------------------------------------------------

#[test]
fn combine_associative_disjoint_paths() {
    // (a + b) + c == a + (b + c) on disjoint paths.
    let mk = |path: &str, msg: &str| {
        Transparency::single(
            path.to_string(),
            PropertyVerdict::Partial {
                confidence: 1.0,
                diagnostics: vec![Diagnostic::new(msg)],
            },
        )
    };
    let a = mk("@a", "a");
    let b = mk("@b", "b");
    let c = mk("@c", "c");
    let lhs = a.clone().combine(b.clone()).combine(c.clone());
    let rhs = a.combine(b.combine(c));
    assert_eq!(lhs, rhs);
}
