//! Membership names its own arms (`In | Out | Undecided`) and maps into
//! `Imperfect` deliberately. Mandelbrot is the first instance, with exact
//! certificates only: exact rational recurrence, an exact real invariant
//! interval, and escape proved by exact |z_n|² > 4.
#![cfg(feature = "fractal")]

use terni::fractal::mandelbrot::{
    membership, of_f64, Complex, Escape, InWitness, Open, Rational, Refused, Stop,
};
use terni::fractal::{join, Darkness, Membership};
use terni::{ConvergenceLoss, Imperfect, Loss};

fn q(num: i128, den: i128) -> Rational {
    Rational::new(num, den).expect("nonzero denominator")
}

fn re(r: Rational) -> Complex {
    Complex::new(r, q(0, 1))
}

fn int(n: i128) -> Complex {
    re(q(n, 1))
}

const BUDGET: usize = 64;

type M = Membership<InWitness, Escape, Open>;

fn in_witness(c: Complex) -> InWitness {
    match membership(c, BUDGET) {
        Membership::In(w) => {
            assert!(w.verifies(&c), "certificate must verify exactly");
            w
        }
        other => panic!("expected In for {c:?}, got {other:?}"),
    }
}

#[test]
fn zero_is_in_by_an_exact_fixed_point() {
    assert_eq!(
        in_witness(int(0)),
        InWitness::Cycle {
            start: 0,
            period: 1,
            cycle: vec![int(0)],
        }
    );
}

#[test]
fn minus_one_is_in_by_an_exact_two_cycle() {
    assert_eq!(
        in_witness(int(-1)),
        InWitness::Cycle {
            start: 0,
            period: 2,
            cycle: vec![int(0), int(-1)],
        }
    );
}

#[test]
fn minus_two_is_in_exactly_on_the_boundary() {
    // 0 -> -2 -> 2 -> 2
    assert_eq!(
        in_witness(int(-2)),
        InWitness::Cycle {
            start: 2,
            period: 1,
            cycle: vec![int(2)],
        }
    );
}

#[test]
fn i_is_in_by_an_exact_preperiodic_cycle() {
    // 0 -> i -> -1+i -> -i -> -1+i
    let i = Complex::new(q(0, 1), q(1, 1));
    assert_eq!(
        in_witness(i),
        InWitness::Cycle {
            start: 2,
            period: 2,
            cycle: vec![
                Complex::new(q(-1, 1), q(1, 1)),
                Complex::new(q(0, 1), q(-1, 1))
            ],
        }
    );
}

#[test]
fn the_cusp_quarter_is_in_by_an_exact_invariant_interval() {
    // f([-1/2, 1/2]) = [1/4, 1/2], inside [-1/2, 1/2], and 0 is in it.
    assert_eq!(
        in_witness(re(q(1, 4))),
        InWitness::Interval {
            lo: q(-1, 2),
            hi: q(1, 2),
        }
    );
}

#[test]
fn minus_three_quarters_is_in_by_an_exact_invariant_interval() {
    assert_eq!(
        in_witness(re(q(-3, 4))),
        InWitness::Interval {
            lo: q(-3, 2),
            hi: q(3, 2),
        }
    );
}

#[test]
fn one_is_out_with_an_exact_escape_witness() {
    // 0 -> 1 -> 2 -> 5, and |5|² = 25 > 4.
    let c = int(1);
    match membership(c, BUDGET) {
        Membership::Out(w) => {
            assert_eq!(w, Escape { n: 3, z: int(5) });
            assert!(w.verifies(&c));
        }
        other => panic!("expected Out, got {other:?}"),
    }
}

#[test]
fn far_outside_escapes_at_the_first_step() {
    let c = Complex::new(q(3, 1), q(-1, 2));
    assert_eq!(
        membership(c, BUDGET),
        Membership::Out(Escape { n: 1, z: c })
    );
}

#[test]
fn a_small_budget_on_a_slow_point_is_undecided() {
    let got = of_f64(1.0, 0.0, 2);
    assert_eq!(
        got,
        Imperfect::Partial(
            Membership::Undecided(Open {
                iterations: 2,
                last: int(2),
                stop: Stop::Budget,
            }),
            ConvergenceLoss::new(2),
        )
    );
}

#[test]
fn a_float_cycle_is_never_in() {
    // -1/2 is inside M, but its exact orbit is not periodic: it converges to
    // the irrational fixed point (1 - √3)/2. A float orbit stalls on a
    // false fixed point; the exact orbit runs out of precision instead.
    let got = membership(re(q(-1, 2)), 1000);
    assert!(
        matches!(
            got,
            Membership::Undecided(Open {
                stop: Stop::Precision,
                ..
            })
        ),
        "got {got:?}"
    );
}

#[test]
fn forged_certificates_do_not_verify() {
    let c = re(q(-1, 2));
    let fake_cycle = InWitness::Cycle {
        start: 0,
        period: 1,
        cycle: vec![int(0)],
    };
    assert!(!fake_cycle.verifies(&c));
    let fake_interval = InWitness::Interval {
        lo: q(-1, 4),
        hi: q(1, 4),
    };
    assert!(!fake_interval.verifies(&c));
    // An interval for a non-real c certifies nothing.
    let i = Complex::new(q(0, 1), q(1, 1));
    let real_only = InWitness::Interval {
        lo: q(-2, 1),
        hi: q(2, 1),
    };
    assert!(!real_only.verifies(&i));
    // An escape witness at the wrong step, or of a bounded orbit, is false.
    assert!(!Escape { n: 2, z: int(5) }.verifies(&int(1)));
    assert!(!Escape { n: 1, z: int(-2) }.verifies(&int(-2)));
    // A wrong cycle length is false.
    let short = InWitness::Cycle {
        start: 0,
        period: 2,
        cycle: vec![int(0)],
    };
    assert!(!short.verifies(&int(-1)));
    // The right length with a wrong member is false.
    let wrong = InWitness::Cycle {
        start: 0,
        period: 2,
        cycle: vec![int(0), int(1)],
    };
    assert!(!wrong.verifies(&int(-1)));
    // An interval that misses 0 is false.
    let off_zero = InWitness::Interval {
        lo: q(1, 4),
        hi: q(1, 2),
    };
    assert!(!off_zero.verifies(&re(q(1, 4))));
    // A lopsided interval whose image overshoots is false.
    let lopsided = InWitness::Interval {
        lo: q(-3, 1),
        hi: q(2, 1),
    };
    assert!(!lopsided.verifies(&int(-2)));
}

#[test]
fn a_non_real_point_without_a_cycle_is_undecided() {
    // Inside the main cardioid, but no exact cycle and no real interval.
    let c = Complex::new(q(-1, 2), q(1, 4));
    assert!(matches!(membership(c, 1000), Membership::Undecided(_)));
}

#[test]
fn decided_arms_map_to_success() {
    assert!(matches!(
        of_f64(0.0, 0.0, BUDGET),
        Imperfect::Success(Membership::In(_))
    ));
    assert!(matches!(
        of_f64(1.0, 0.0, BUDGET),
        Imperfect::Success(Membership::Out(_))
    ));
    assert!(matches!(
        of_f64(0.25, 0.0, BUDGET),
        Imperfect::Success(Membership::In(InWitness::Interval { .. }))
    ));
}

#[test]
fn nonsense_input_maps_to_failure() {
    for (x, y) in [
        (f64::NAN, 0.0),
        (0.0, f64::NAN),
        (f64::INFINITY, 0.0),
        (0.0, f64::NEG_INFINITY),
    ] {
        match of_f64(x, y, BUDGET) {
            Imperfect::Failure(refused, loss) => {
                assert_eq!(refused.input_bits(), (x.to_bits(), y.to_bits()));
                assert!(loss.is_zero());
            }
            other => panic!("expected Failure, got {other:?}"),
        }
    }
}

#[test]
fn finite_but_unrepresentable_input_is_undecided_not_failure() {
    assert!(matches!(
        of_f64(1e300, 0.0, BUDGET),
        Imperfect::Partial(
            Membership::Undecided(Open {
                iterations: 0,
                stop: Stop::Precision,
                ..
            }),
            _
        )
    ));
}

#[test]
fn out_sits_among_the_values_never_among_the_darknesses() {
    let joined = join(vec![
        of_f64(1.0, 0.0, BUDGET),
        of_f64(0.0, 0.0, BUDGET),
        of_f64(f64::NAN, 0.0, BUDGET),
    ]);
    let outs: Vec<&M> = joined
        .values()
        .iter()
        .filter_map(|v| match v {
            Imperfect::Success(m @ Membership::Out(_)) => Some(m),
            _ => None,
        })
        .collect();
    assert_eq!(outs, vec![&Membership::Out(Escape { n: 3, z: int(5) })]);
    assert_eq!(joined.values().len(), 2);
    // The only darkness is the refused input.
    assert_eq!(joined.darkness_count(), 1);
    let only: Vec<&Refused> = joined.zoom().iter().collect();
    assert_eq!(only[0].address(), (f64::NAN.to_bits(), 0.0f64.to_bits()));
}

#[test]
fn the_membership_map_is_generic_and_deliberate() {
    let undecided: Membership<&str, &str, usize> = Membership::Undecided(3);
    assert_eq!(
        undecided.into_imperfect::<(), usize>(|steps| *steps),
        Imperfect::Partial(Membership::Undecided(3), 3)
    );
    let proved_out: Membership<&str, &str, usize> = Membership::Out("escaped");
    assert_eq!(
        proved_out.into_imperfect::<(), usize>(|_| unreachable!("decided arms are not measured")),
        Imperfect::Success(Membership::Out("escaped"))
    );
    let proved_in: Membership<&str, &str, usize> = Membership::In("cycle");
    assert!(proved_in.is_decided());
    assert!(!Membership::<(), (), ()>::Undecided(()).is_decided());
}

#[test]
fn rationals_are_exact_and_normalized() {
    assert_eq!(q(2, -4), q(-1, 2));
    assert_eq!((q(-1, 2).num(), q(-1, 2).den()), (-1, 2));
    assert_eq!(Rational::new(1, 0), None);
    assert_eq!(Rational::from_f64(0.25), Some(q(1, 4)));
    assert_eq!(Rational::from_f64(-0.0), Some(q(0, 1)));
    assert_eq!(Rational::from_f64(-3.0), Some(q(-3, 1)));
    assert_eq!(Rational::from_f64(f64::NAN), None);
    assert_eq!(Rational::from_f64(1e300), None);
    assert_eq!(Rational::from_f64(f64::MIN_POSITIVE), None);
    // Subnormal: exact, but too small for i128.
    assert_eq!(Rational::from_f64(5e-324), None);
    assert_eq!(Rational::from_f64(-0.375), Some(q(-3, 8)));
    assert_eq!(Rational::new(i128::MIN, -1), None);
}
