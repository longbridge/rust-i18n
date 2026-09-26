//! Tests for the assembly interpolation kernel behind `replace_patterns_cow`.
//!
//! Every result must match both the optimized Rust parser and the original
//! state machine, whether the kernel completed or asked for the Rust fallback.

use super::interpolate::{replace_patterns_cow, try_replace_patterns_cow};
use crate::{replace_patterns_impl, replace_patterns_legacy};
use std::borrow::Cow;

const KERNEL_TARGET: bool = cfg!(all(
    target_pointer_width = "64",
    any(target_arch = "x86_64", target_arch = "aarch64")
));

fn assert_matches_rust(input: &str, patterns: &[&str], values: &[Cow<'_, str>]) {
    let actual = replace_patterns_cow(input, patterns, values);
    assert_eq!(
        actual,
        replace_patterns_impl(input, patterns, values),
        "optimized Rust differs for input={input:?}, patterns={patterns:?}"
    );
    assert_eq!(
        actual,
        replace_patterns_legacy(input, patterns, values),
        "legacy Rust differs for input={input:?}, patterns={patterns:?}"
    );
    assert_eq!(
        actual,
        crate::replace_patterns_cow(input, patterns, values),
        "public Cow entry differs for input={input:?}, patterns={patterns:?}"
    );
}

fn assert_kernel_used(input: &str, patterns: &[&str], values: &[Cow<'_, str>]) {
    if KERNEL_TARGET {
        assert!(
            try_replace_patterns_cow(input, patterns, values).is_some(),
            "kernel did not run for supported input={input:?}, patterns={patterns:?}"
        );
    }
    assert_matches_rust(input, patterns, values);
}

#[test]
fn kernel_handles_empty_plain_unicode_unknown_and_duplicate_keys() {
    let empty: [Cow<'_, str>; 0] = [];
    assert_kernel_used("", &[], &empty);
    assert_kernel_used("plain text", &[], &empty);

    let values = [Cow::Borrowed("世界"), Cow::Owned("second".to_string())];
    assert_kernel_used("你好 %{name}🙂", &["name", "unused"], &values);
    assert_kernel_used("你好 %{名字}🙂", &["名字", "unused"], &values);
    assert_kernel_used("%{unknown} %{name}", &["name", "unused"], &values);
    assert_kernel_used("%{name} %{name}", &["name", "name"], &values);
}

#[test]
fn kernel_handles_zero_through_eight_values_and_ninth_falls_back() {
    for count in 0..=9 {
        let names: Vec<String> = (0..count).map(|index| format!("key{index}")).collect();
        let patterns: Vec<&str> = names.iter().map(String::as_str).collect();
        let values: Vec<Cow<'_, str>> = (0..count)
            .map(|index| Cow::Owned(format!("value{index}")))
            .collect();
        let input = names
            .iter()
            .map(|name| format!("%{{{name}}}"))
            .collect::<String>();

        if count <= 8 {
            assert_kernel_used(&input, &patterns, &values);
        } else {
            assert!(
                try_replace_patterns_cow(&input, &patterns, &values).is_none(),
                "nine arguments should use Rust fallback"
            );
            assert_matches_rust(&input, &patterns, &values);
        }
    }
}

#[test]
fn unequal_pattern_and_value_counts_preserve_zip_semantics() {
    let one_value = [Cow::Borrowed("A")];
    assert_matches_rust("%{a} %{b}", &["a", "b"], &one_value);

    let two_values = [Cow::Borrowed("A"), Cow::Borrowed("B")];
    assert_matches_rust("%{a} %{b}", &["a"], &two_values);
}

#[test]
fn malformed_markers_and_literal_percent_use_rust_fallback() {
    let values = [Cow::Borrowed("A"), Cow::Borrowed("X")];
    for input in [
        "%",
        "literal % text",
        "%{a",
        "%{a%{x}",
        "%%{a}",
        "%{a} then %",
        "%{a} then %{unfinished",
    ] {
        // The kernels treat stray and unfinished percent signs as literal
        // text, like the legacy parser; see `kernel_treats_*` below.
        if input == "%{a%{x}" {
            assert!(
                try_replace_patterns_cow(input, &["a", "x"], &values).is_none(),
                "malformed input unexpectedly used kernel: {input:?}"
            );
        }
        assert_matches_rust(input, &["a", "x"], &values);
    }
}

#[test]
fn output_exceeding_initial_capacity_falls_back_without_truncation() {
    let long_value = "界".repeat(512);
    let values = [Cow::Borrowed(long_value.as_str())];
    let input = "before %{name} after";
    assert!(
        try_replace_patterns_cow(input, &["name"], &values).is_none(),
        "oversized output should use Rust fallback"
    );
    assert_matches_rust(input, &["name"], &values);
}

#[test]
fn slice_boundary_sentinels_are_not_part_of_input() {
    let empty: [Cow<'_, str>; 0] = [];
    for len in [0, 1, 15, 16, 17, 255, 256, 257, 511] {
        for offset in 1..=16 {
            let mut storage = String::with_capacity(offset + len + 1);
            storage.push_str(&"x".repeat(offset - 1));
            storage.push('%');
            storage.push_str(&"a".repeat(len));
            storage.push('%');
            let input = &storage[offset..offset + len];
            assert_kernel_used(input, &[], &empty);
        }
    }
}

#[test]
fn exhaustive_short_marker_sequences_match_legacy() {
    const ALPHABET: &[u8] = b"%{}ax";
    let values = [Cow::Borrowed("世界"), Cow::Owned("%{".to_string())];
    for len in 0..=7_u32 {
        for mut code in 0..ALPHABET.len().pow(len) {
            let mut input = String::with_capacity(len as usize);
            for _ in 0..len {
                input.push(ALPHABET[code % ALPHABET.len()] as char);
                code /= ALPHABET.len();
            }
            assert_matches_rust(&input, &["a", "x"], &values);
        }
    }
}

/// Deterministic xorshift64* generator; the seed makes failures reproducible.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }
}

const KEYS: &[&str] = &["a", "b", "name", "名字", "", "k1", "k-2", "🙂"];
const UNKNOWN_KEYS: &[&str] = &["zz", "A", "name ", "missing"];
const TEXT: &[&str] = &[
    "x", "hello ", " ", "é", "世界", "🙂", "%", "%{", "{", "}", "%%", "{}", "\n", "abc123",
];
const VALUES: &[&str] = &[
    "",
    "v",
    "世界",
    "%{a}",
    "%",
    "}",
    "🙂🙂",
    "value with spaces",
];

fn random_value(rng: &mut Rng) -> String {
    match rng.below(8) {
        // Longer than the adapter's 128-byte slack, forcing capacity fallback
        // once combined with a short input.
        0 => "長".repeat(43 + rng.below(40)),
        1 => "y".repeat(100 + rng.below(80)),
        _ => rng.pick(VALUES).to_string(),
    }
}

fn random_input(rng: &mut Rng) -> String {
    let mut input = String::new();
    for _ in 0..rng.below(24) {
        match rng.below(10) {
            0..=3 => {
                input.push_str("%{");
                input.push_str(rng.pick(KEYS));
                input.push('}');
            }
            4 => {
                input.push_str("%{");
                input.push_str(rng.pick(UNKNOWN_KEYS));
                input.push('}');
            }
            5 => input.push_str(&"a".repeat(rng.below(300))),
            _ => input.push_str(rng.pick(TEXT)),
        }
    }
    input
}

#[test]
fn random_inputs_match_legacy() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let mut kernel_runs = 0;
    let mut fallbacks = 0;
    for iteration in 0..20_000 {
        let input = random_input(&mut rng);
        let patterns: Vec<&str> = (0..rng.below(13)).map(|_| rng.pick(KEYS)).collect();
        // Occasionally unpair names and values to exercise `zip` semantics.
        let value_count = match rng.below(8) {
            0 => patterns.len().saturating_sub(1),
            1 => patterns.len() + 1,
            _ => patterns.len(),
        };
        let owned: Vec<String> = (0..value_count).map(|_| random_value(&mut rng)).collect();
        let values: Vec<Cow<'_, str>> = owned
            .iter()
            .map(|value| {
                if rng.below(2) == 0 {
                    Cow::Borrowed(value.as_str())
                } else {
                    Cow::Owned(value.clone())
                }
            })
            .collect();

        let expected = replace_patterns_legacy(&input, &patterns, &values);
        match try_replace_patterns_cow(&input, &patterns, &values) {
            Some(actual) => {
                kernel_runs += 1;
                assert_eq!(
                    actual, expected,
                    "kernel differs at iteration {iteration}: input={input:?}, \
                     patterns={patterns:?}, values={values:?}"
                );
            }
            None => fallbacks += 1,
        }
        assert_eq!(
            replace_patterns_cow(&input, &patterns, &values),
            expected,
            "adapter differs at iteration {iteration}: input={input:?}, \
             patterns={patterns:?}, values={values:?}"
        );
    }

    if KERNEL_TARGET {
        // Guard against a generator change that silently stops exercising
        // either the kernel or its fallback.
        assert!(kernel_runs > 2_000, "kernel ran only {kernel_runs} times");
        assert!(fallbacks > 2_000, "fallback ran only {fallbacks} times");
    }
}

#[test]
fn kernel_treats_stray_and_unfinished_percent_as_literal() {
    let values = [Cow::Borrowed("A"), Cow::Borrowed("X")];
    let long = "a".repeat(2048);
    let long_unfinished = format!("{long}%{{a}} %{{unfinished");
    let long_stray = format!("%{{a}} 100% {long} %{{x}} 50%");
    let unfinished_tail = format!("%{{a}} %{{{long}");
    let stray_run = format!("% {long}%{{a}}");
    for input in [
        "%",
        "%%",
        "%{",
        "%}",
        "%{a",
        "%%{a}",
        "literal % text",
        "Hello %{a}! 100% ready",
        "%{a} then %",
        "%{a} then %{unfinished",
        "%{a} then %{unfinished}}",
        "50% } %{x}",
        "{%}%{a}{",
        long_unfinished.as_str(),
        long_stray.as_str(),
        unfinished_tail.as_str(),
        stray_run.as_str(),
    ] {
        assert_kernel_used(input, &["a", "x"], &values);
    }
    // A brace after a stray percent sign opens a legacy marker at the brace.
    for input in ["100% a{a}", "% {a}", "%}{a}", "%{a%}", "%{a} % x{"] {
        assert!(
            try_replace_patterns_cow(input, &["a", "x"], &values).is_none(),
            "kernel accepted legacy brace marker: {input:?}"
        );
        assert_matches_rust(input, &["a", "x"], &values);
    }
}

#[test]
fn stray_percent_runs_across_block_boundaries_match_legacy() {
    let values = [Cow::Borrowed("A")];
    for prefix in 0..20 {
        for len in 0..48 {
            let lead = "p".repeat(prefix);
            let run = "x".repeat(len);
            for tail in ["", "{a}", "%{a}", "}", "%", "%{", "%{a", "}{a}"] {
                let input = format!("{lead}%{run}{tail}");
                assert_matches_rust(&input, &["a"], &values);
                let input = format!("{lead}%{{a{run}{tail}");
                assert_matches_rust(&input, &["a"], &values);
            }
        }
    }
}
