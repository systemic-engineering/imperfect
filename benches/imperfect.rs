#![allow(clippy::bind_instead_of_map)]
//! Benchmarks for terni — the cost of honesty, measured in nanoseconds.
//!
//! The thesis: `.eh()` on the Success path is zero-cost compared to Result's
//! `.and_then()`. The overhead only appears when loss accumulates through
//! Partial — and that overhead IS the value. You're paying for information
//! that Result throws away.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use terni::{ApertureLoss, ConvergenceLoss, Eh, Imperfect, Loss, RoutingLoss};

// ---------------------------------------------------------------------------
// 1. Construction
// ---------------------------------------------------------------------------

fn bench_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("construction");

    group.bench_function("Result::Ok", |b| {
        b.iter(|| -> Result<i32, String> { black_box(Ok(black_box(42))) })
    });

    group.bench_function("Result::Err", |b| {
        b.iter(|| -> Result<i32, String> { black_box(Err(black_box(String::from("gone")))) })
    });

    group.bench_function("Imperfect::Success", |b| {
        b.iter(|| -> Imperfect<i32, String, ConvergenceLoss> {
            black_box(Imperfect::Success(black_box(42)))
        })
    });

    group.bench_function("Imperfect::Partial", |b| {
        b.iter(|| -> Imperfect<i32, String, ConvergenceLoss> {
            black_box(Imperfect::Partial(black_box(42), ConvergenceLoss::new(3)))
        })
    });

    group.bench_function("Imperfect::Failure", |b| {
        b.iter(|| -> Imperfect<i32, String, ConvergenceLoss> {
            black_box(Imperfect::Failure(
                black_box(String::from("gone")),
                ConvergenceLoss::new(5),
            ))
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// 2. .eh() pipeline — Success path vs Result::and_then
// ---------------------------------------------------------------------------

fn bench_eh_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("eh_pipeline_x10");

    // 10 chained and_then on Result — the baseline
    group.bench_function("Result::and_then_success", |b| {
        b.iter(|| {
            let r: Result<i32, String> = Ok(black_box(1));
            r.and_then(|x| Ok(x + 1))
                .and_then(|x| Ok(x + 1))
                .and_then(|x| Ok(x + 1))
                .and_then(|x| Ok(x + 1))
                .and_then(|x| Ok(x + 1))
                .and_then(|x| Ok(x + 1))
                .and_then(|x| Ok(x + 1))
                .and_then(|x| Ok(x + 1))
                .and_then(|x| Ok(x + 1))
                .and_then(|x| Ok(x + 1))
        })
    });

    // 10 chained .eh() on Success — should be within noise of Result
    group.bench_function("Imperfect::eh_success", |b| {
        b.iter(|| {
            let i: Imperfect<i32, String, ConvergenceLoss> = Imperfect::Success(black_box(1));
            i.eh(|x| Imperfect::Success(x + 1))
                .eh(|x| Imperfect::Success(x + 1))
                .eh(|x| Imperfect::Success(x + 1))
                .eh(|x| Imperfect::Success(x + 1))
                .eh(|x| Imperfect::Success(x + 1))
                .eh(|x| Imperfect::Success(x + 1))
                .eh(|x| Imperfect::Success(x + 1))
                .eh(|x| Imperfect::Success(x + 1))
                .eh(|x| Imperfect::Success(x + 1))
                .eh(|x| Imperfect::Success(x + 1))
        })
    });

    // 10 chained and_then on Err — short-circuit baseline
    group.bench_function("Result::and_then_err", |b| {
        b.iter(|| {
            let r: Result<i32, String> = Err(black_box(String::from("nope")));
            r.and_then(|x| Ok(x + 1))
                .and_then(|x| Ok(x + 1))
                .and_then(|x| Ok(x + 1))
                .and_then(|x| Ok(x + 1))
                .and_then(|x| Ok(x + 1))
                .and_then(|x| Ok(x + 1))
                .and_then(|x| Ok(x + 1))
                .and_then(|x| Ok(x + 1))
                .and_then(|x| Ok(x + 1))
                .and_then(|x| Ok(x + 1))
        })
    });

    // 10 chained .eh() on Failure — short-circuit, should match Result::Err
    group.bench_function("Imperfect::eh_failure", |b| {
        b.iter(|| {
            let i: Imperfect<i32, String, ConvergenceLoss> =
                Imperfect::Failure(black_box(String::from("nope")), ConvergenceLoss::new(0));
            i.eh(|x| Imperfect::Success(x + 1))
                .eh(|x| Imperfect::Success(x + 1))
                .eh(|x| Imperfect::Success(x + 1))
                .eh(|x| Imperfect::Success(x + 1))
                .eh(|x| Imperfect::Success(x + 1))
                .eh(|x| Imperfect::Success(x + 1))
                .eh(|x| Imperfect::Success(x + 1))
                .eh(|x| Imperfect::Success(x + 1))
                .eh(|x| Imperfect::Success(x + 1))
                .eh(|x| Imperfect::Success(x + 1))
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// 3. .eh() with loss accumulation — the cost of honesty
// ---------------------------------------------------------------------------

fn bench_eh_partial(c: &mut Criterion) {
    let mut group = c.benchmark_group("eh_partial_x10");

    // Result has no equivalent — it collapses to Ok. This is the control.
    group.bench_function("Result::and_then_always_ok", |b| {
        b.iter(|| {
            let r: Result<i32, String> = Ok(black_box(1));
            r.and_then(|x| Ok(x + 1))
                .and_then(|x| Ok(x + 1))
                .and_then(|x| Ok(x + 1))
                .and_then(|x| Ok(x + 1))
                .and_then(|x| Ok(x + 1))
                .and_then(|x| Ok(x + 1))
                .and_then(|x| Ok(x + 1))
                .and_then(|x| Ok(x + 1))
                .and_then(|x| Ok(x + 1))
                .and_then(|x| Ok(x + 1))
        })
    });

    // Every step returns Partial — loss accumulates at every hop.
    // The delta between this and the Result baseline IS the measurement.
    group.bench_function("Imperfect::eh_all_partial", |b| {
        b.iter(|| {
            let i: Imperfect<i32, String, ConvergenceLoss> =
                Imperfect::Partial(black_box(1), ConvergenceLoss::new(1));
            i.eh(|x| Imperfect::Partial(x + 1, ConvergenceLoss::new(1)))
                .eh(|x| Imperfect::Partial(x + 1, ConvergenceLoss::new(1)))
                .eh(|x| Imperfect::Partial(x + 1, ConvergenceLoss::new(1)))
                .eh(|x| Imperfect::Partial(x + 1, ConvergenceLoss::new(1)))
                .eh(|x| Imperfect::Partial(x + 1, ConvergenceLoss::new(1)))
                .eh(|x| Imperfect::Partial(x + 1, ConvergenceLoss::new(1)))
                .eh(|x| Imperfect::Partial(x + 1, ConvergenceLoss::new(1)))
                .eh(|x| Imperfect::Partial(x + 1, ConvergenceLoss::new(1)))
                .eh(|x| Imperfect::Partial(x + 1, ConvergenceLoss::new(1)))
                .eh(|x| Imperfect::Partial(x + 1, ConvergenceLoss::new(1)))
        })
    });

    // Mixed: starts Success, hits Partial midway. Loss appears at step 5.
    group.bench_function("Imperfect::eh_mixed_partial_at_5", |b| {
        b.iter(|| {
            let i: Imperfect<i32, String, ConvergenceLoss> = Imperfect::Success(black_box(1));
            i.eh(|x| Imperfect::Success(x + 1))
                .eh(|x| Imperfect::Success(x + 1))
                .eh(|x| Imperfect::Success(x + 1))
                .eh(|x| Imperfect::Success(x + 1))
                .eh(|x| Imperfect::Partial(x + 1, ConvergenceLoss::new(3))) // loss enters
                .eh(|x| Imperfect::Success(x + 1))
                .eh(|x| Imperfect::Success(x + 1))
                .eh(|x| Imperfect::Success(x + 1))
                .eh(|x| Imperfect::Success(x + 1))
                .eh(|x| Imperfect::Success(x + 1))
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// 4. .recover() vs Result::or_else
// ---------------------------------------------------------------------------

fn bench_recover(c: &mut Criterion) {
    let mut group = c.benchmark_group("recover");

    group.bench_function("Result::or_else", |b| {
        b.iter(|| {
            let r: Result<i32, String> = Err(black_box(String::from("gone")));
            r.or_else(|_e| Ok::<i32, String>(black_box(0)))
        })
    });

    group.bench_function("Imperfect::recover", |b| {
        b.iter(|| {
            let i: Imperfect<i32, String, ConvergenceLoss> =
                Imperfect::Failure(black_box(String::from("gone")), ConvergenceLoss::new(3));
            i.recover(|_e| Imperfect::Success(black_box(0)))
        })
    });

    // Recovery from success (passthrough — no work)
    group.bench_function("Imperfect::recover_noop", |b| {
        b.iter(|| {
            let i: Imperfect<i32, String, ConvergenceLoss> = Imperfect::Success(black_box(42));
            i.recover(|_e| Imperfect::Success(0))
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// 5. .map() — Imperfect::map vs Result::map
// ---------------------------------------------------------------------------

fn bench_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("map");

    group.bench_function("Result::map", |b| {
        b.iter(|| {
            let r: Result<i32, String> = Ok(black_box(42));
            r.map(|x| x * 2)
        })
    });

    group.bench_function("Imperfect::map_success", |b| {
        b.iter(|| {
            let i: Imperfect<i32, String, ConvergenceLoss> = Imperfect::Success(black_box(42));
            i.map(|x| x * 2)
        })
    });

    group.bench_function("Imperfect::map_partial", |b| {
        b.iter(|| {
            let i: Imperfect<i32, String, ConvergenceLoss> =
                Imperfect::Partial(black_box(42), ConvergenceLoss::new(3));
            i.map(|x| x * 2)
        })
    });

    group.bench_function("Imperfect::map_failure", |b| {
        b.iter(|| {
            let i: Imperfect<i32, String, ConvergenceLoss> =
                Imperfect::Failure(black_box(String::from("gone")), ConvergenceLoss::new(3));
            i.map(|x| x * 2)
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// 6. .compose() — legacy path
// ---------------------------------------------------------------------------

fn bench_compose(c: &mut Criterion) {
    let mut group = c.benchmark_group("compose");

    group.bench_function("success_then_success", |b| {
        b.iter(|| {
            let a: Imperfect<i32, String, ConvergenceLoss> = Imperfect::Success(black_box(1));
            let next: Imperfect<i32, String, ConvergenceLoss> = Imperfect::Success(black_box(2));
            a.compose(next)
        })
    });

    group.bench_function("partial_then_partial", |b| {
        b.iter(|| {
            let a: Imperfect<i32, String, ConvergenceLoss> =
                Imperfect::Partial(black_box(1), ConvergenceLoss::new(2));
            let next: Imperfect<i32, String, ConvergenceLoss> =
                Imperfect::Partial(black_box(2), ConvergenceLoss::new(3));
            a.compose(next)
        })
    });

    group.bench_function("failure_shortcircuit", |b| {
        b.iter(|| {
            let a: Imperfect<i32, String, ConvergenceLoss> =
                Imperfect::Failure(black_box(String::from("nope")), ConvergenceLoss::new(1));
            let next: Imperfect<i32, String, ConvergenceLoss> = Imperfect::Success(black_box(2));
            a.compose(next)
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// 7. Eh context — 10 operations through the accumulator
// ---------------------------------------------------------------------------

fn bench_eh_context(c: &mut Criterion) {
    let mut group = c.benchmark_group("eh_context_x10");

    // All success through Eh — no loss accumulates
    group.bench_function("all_success", |b| {
        b.iter(|| {
            let mut eh = Eh::<ConvergenceLoss>::new();
            let mut val = eh
                .eh(Imperfect::<i32, String, _>::Success(black_box(1)))
                .unwrap();
            for _ in 0..9 {
                val = eh
                    .eh(Imperfect::<i32, String, _>::Success(val + 1))
                    .unwrap();
            }
            eh.finish::<i32, String>(val)
        })
    });

    // All partial through Eh — loss accumulates every step
    group.bench_function("all_partial", |b| {
        b.iter(|| {
            let mut eh = Eh::<ConvergenceLoss>::new();
            let mut val = eh
                .eh(Imperfect::<i32, String, _>::Partial(
                    black_box(1),
                    ConvergenceLoss::new(1),
                ))
                .unwrap();
            for _ in 0..9 {
                val = eh
                    .eh(Imperfect::<i32, String, _>::Partial(
                        val + 1,
                        ConvergenceLoss::new(1),
                    ))
                    .unwrap();
            }
            eh.finish::<i32, String>(val)
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// 8. Loss type construction and combine
// ---------------------------------------------------------------------------

fn bench_loss_types(c: &mut Criterion) {
    let mut group = c.benchmark_group("loss_types");

    // ConvergenceLoss
    group.bench_function("ConvergenceLoss::new", |b| {
        b.iter(|| black_box(ConvergenceLoss::new(black_box(42))))
    });

    group.bench_function("ConvergenceLoss::combine", |b| {
        b.iter(|| {
            let a = ConvergenceLoss::new(black_box(3));
            let b_val = ConvergenceLoss::new(black_box(7));
            a.combine(b_val)
        })
    });

    // ApertureLoss
    group.bench_function("ApertureLoss::new", |b| {
        b.iter(|| black_box(ApertureLoss::new(black_box(vec![1, 3, 5]), black_box(10))))
    });

    group.bench_function("ApertureLoss::combine", |b| {
        b.iter(|| {
            let a = ApertureLoss::new(vec![1, 3, 5], 10);
            let b_val = ApertureLoss::new(vec![2, 3, 7], 10);
            a.combine(b_val)
        })
    });

    // RoutingLoss
    group.bench_function("RoutingLoss::new", |b| {
        b.iter(|| black_box(RoutingLoss::new(black_box(1.5), black_box(0.3))))
    });

    group.bench_function("RoutingLoss::combine", |b| {
        b.iter(|| {
            let a = RoutingLoss::new(1.5, 0.3);
            let b_val = RoutingLoss::new(2.1, 0.1);
            a.combine(b_val)
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// The cascade
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_construction,
    bench_eh_pipeline,
    bench_eh_partial,
    bench_recover,
    bench_map,
    bench_compose,
    bench_eh_context,
    bench_loss_types,
);
criterion_main!(benches);
