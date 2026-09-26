//! Controlled end-to-end benchmarks for comparing placeholder scanners.
//!
//! The experiment exports this unchanged file as `benches/asm_effect.rs` in
//! every variant. Only the scanner implementation should differ.

use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rust_i18n::t;

rust_i18n::i18n!("./experiments/asm/locales");

fn bench_full_t(c: &mut Criterion) {
    rust_i18n::set_locale("en");
    let name = String::from("Jason");

    // These assertions run before any timing loop. A missing translation would
    // otherwise benchmark the miss path instead of the intended string shape.
    assert_eq!(t!("plain"), "English plain");
    assert_eq!(t!("plain", locale = "fr"), "Texte français");
    assert_eq!(
        t!("short", name = "Jason", place = "world"),
        "Hello, Jason! Welcome to world."
    );
    assert_eq!(
        t!("short", name = name.as_str(), place = "world"),
        "Hello, Jason! Welcome to world."
    );
    assert_eq!(
        t!("long_sparse", name = "Jason", place = "world"),
        format!("{}Jason{}world", "a".repeat(2048), "b".repeat(2048))
    );
    assert_eq!(t!("long_no_marker", name = "Jason"), "c".repeat(4096));
    assert_eq!(t!("long_dense", name = "Jason"), "xJason".repeat(512));
    assert_eq!(
        t!("unicode_long", name = "Jason", place = "world"),
        format!("{}Jason{}world", "你".repeat(512), "界".repeat(512))
    );

    c.bench_function("asm_effect_plain", |b| b.iter(|| t!("plain")));
    c.bench_function("asm_effect_explicit_locale", |b| {
        b.iter(|| t!("plain", locale = "fr"))
    });
    c.bench_function("asm_effect_short_args", |b| {
        b.iter(|| t!("short", name = "Jason", place = "world"))
    });
    c.bench_function("asm_effect_dynamic_args", |b| {
        b.iter(|| {
            let name = black_box(name.as_str());
            t!("short", name = name, place = "world")
        })
    });
    c.bench_function("asm_effect_long_sparse", |b| {
        b.iter(|| t!("long_sparse", name = "Jason", place = "world"))
    });
    c.bench_function("asm_effect_long_no_marker", |b| {
        b.iter(|| t!("long_no_marker", name = "Jason"))
    });
    c.bench_function("asm_effect_long_dense", |b| {
        b.iter(|| t!("long_dense", name = "Jason"))
    });
    c.bench_function("asm_effect_unicode_long", |b| {
        b.iter(|| t!("unicode_long", name = "Jason", place = "world"))
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2))
        .sample_size(30);
    targets = bench_full_t
}
criterion_main!(benches);
