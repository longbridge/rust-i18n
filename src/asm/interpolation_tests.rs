#[cfg(test)]
mod core_asm_experiment_tests {
    use super::{core_asm_runtime, replace_patterns_impl, replace_patterns_legacy};
    use std::borrow::Cow;

    fn assert_matches_rust(input: &str, patterns: &[&str], values: &[Cow<'_, str>]) {
        let actual = core_asm_runtime::replace_patterns_cow(input, patterns, values);
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
    }

    fn assert_kernel_used(input: &str, patterns: &[&str], values: &[Cow<'_, str>]) {
        if cfg!(any(target_arch = "x86_64", target_arch = "aarch64")) {
            assert!(
                core_asm_runtime::try_replace_patterns_cow(input, patterns, values).is_some(),
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
                    core_asm_runtime::try_replace_patterns_cow(&input, &patterns, &values)
                        .is_none(),
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
            "%", "literal % text", "%{a", "%{a%{x}", "%%{a}", "%{a} then %",
            "%{a} then %{unfinished",
        ] {
            assert!(
                core_asm_runtime::try_replace_patterns_cow(input, &["a", "x"], &values)
                    .is_none(),
                "malformed input unexpectedly used kernel: {input:?}"
            );
            assert_matches_rust(input, &["a", "x"], &values);
        }
    }

    #[test]
    fn output_exceeding_initial_capacity_falls_back_without_truncation() {
        let long_value = "界".repeat(512);
        let values = [Cow::Borrowed(long_value.as_str())];
        let input = "before %{name} after";
        assert!(
            core_asm_runtime::try_replace_patterns_cow(input, &["name"], &values).is_none(),
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
}
