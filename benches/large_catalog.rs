use rust_i18n::t;

// 240 keys in 4 locales exceeds the generated static lookup limit, so calls
// use the runtime backend, as in most applications.
rust_i18n::i18n!("./benches/fixtures/large_catalog", fallback = "en");

use criterion::{black_box, criterion_group, criterion_main, Criterion};

const SECTIONS: [&str; 8] = [
    "account",
    "billing",
    "dashboard",
    "orders",
    "profile",
    "settings",
    "security",
    "reports",
];

fn bench_large_catalog(c: &mut Criterion) {
    assert_eq!(t!("orders.save_1"), "Text orders.save_1");
    c.bench_function("t_large", |b| b.iter(|| t!("orders.save_1")));

    assert_eq!(t!("orders.save_1", locale = "zh-CN"), "文本 orders.save_1");
    c.bench_function("t_large_with_locale", |b| {
        b.iter(|| t!("orders.save_1", locale = "zh-CN"))
    });

    assert_eq!(
        t!("account.title_0", name = "Jason"),
        "Text account.title_0 Jason"
    );
    c.bench_function("t_large_with_args", |b| {
        b.iter(|| t!("account.title_0", name = "Jason"))
    });

    // Different keys per call, as when rendering a page.
    let keys: Vec<String> = SECTIONS
        .iter()
        .flat_map(|section| (0..8).map(move |i| format!("{section}.label_{}", i % 2)))
        .collect();
    c.bench_function("t_large_dynamic_key", |b| {
        let mut i = 0;
        b.iter(|| {
            i = (i + 1) % keys.len();
            t!(black_box(keys[i].as_str()))
        })
    });
}

criterion_group!(benches, bench_large_catalog);
criterion_main!(benches);
