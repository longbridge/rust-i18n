use std::arch::asm;

use super::Pattern;

/// Interpolates complete `%{key}` markers into caller-owned spare capacity.
///
/// Returns the initialized byte count, or `usize::MAX` to request the Rust
/// fallback. On fallback, the output may contain an incomplete prefix that
/// must be discarded. The first equal descriptor wins; unknown markers are
/// copied unchanged. Stray percent signs, nested percent signs and incomplete
/// markers deliberately use the existing Rust parser for historical behavior.
///
/// # Safety
///
/// - Input and descriptor pointers must identify initialized readable slices
///   of the supplied lengths. Each descriptor must contain valid readable key
///   and value byte slices. Zero-length slices may use dangling nonnull pointers.
/// - `output` must identify `capacity` writable bytes in one allocation,
///   disjoint from input, descriptor storage, and every key/value slice.
/// - All slices must satisfy Rust slice size bounds (`isize::MAX`).
/// - `Pattern` must have the documented 64-bit C layout: key pointer and length
///   at offsets 0 and 8, value pointer and length at offsets 16 and 24.
///
/// The safe runtime adapter validates lengths and builds descriptors from
/// borrowed strings before calling this kernel. Complete string input and
/// replacements produce UTF-8 output, since marker boundaries are ASCII.
pub(super) unsafe fn interpolate(
    input: *const u8,
    input_len: usize,
    patterns: *const Pattern,
    pattern_count: usize,
    output: *mut u8,
    capacity: usize,
) -> usize {
    let written: usize;
    // SAFETY: The caller supplies the valid, disjoint slices above. Every input
    // byte load checks its end pointer; descriptor loads check the count; key
    // comparisons check key length; copies check remaining output capacity.
    // Vector literal and key-scan loads require 16 or 8 remaining input bytes,
    // and literal stores require as many remaining output bytes. Key compares
    // and value copies use overlapping chunks that stay inside each slice.
    // All mutated registers are declared, including vector scratch v0-v3;
    // omitting `preserves_flags` declares NZCV clobbered. No stack access,
    // external calls, retained pointers, or reserved x18 register are used.
    unsafe {
        asm!(
            include_str!("interpolate.asm"),
            inlateout("x0") input => written,
            in("x1") input.add(input_len),
            in("x2") patterns,
            in("x3") pattern_count,
            inlateout("x4") output => _,
            in("x5") output.add(capacity),
            out("x6") _,
            out("x7") _,
            out("x8") _,
            out("x9") _,
            out("x10") _,
            out("x11") _,
            out("x12") _,
            out("x13") _,
            out("x14") _,
            out("x15") _,
            out("x16") _,
            out("x17") _,
            out("v0") _,
            out("v1") _,
            out("v2") _,
            out("v3") _,
            options(nostack),
        );
    }
    written
}

#[cfg(test)]
mod tests {
    use super::{interpolate, Pattern};
    use std::borrow::Cow;

    const CANARY: u8 = 0xA5;

    /// Byte-level model of the kernel contract: `None` means fallback.
    fn model(input: &[u8], pairs: &[(&[u8], &[u8])], capacity: usize) -> Option<Vec<u8>> {
        let mut output = Vec::new();
        let mut index = 0;
        while index < input.len() {
            if input[index] != b'%' {
                output.push(input[index]);
                index += 1;
                continue;
            }
            if input.get(index + 1) != Some(&b'{') {
                return None;
            }
            let start = index + 2;
            let end = start
                + input[start..]
                    .iter()
                    .position(|&b| b == b'}' || b == b'%')?;
            if input[end] == b'%' {
                return None;
            }
            let key = &input[start..end];
            match pairs.iter().find(|(pattern, _)| *pattern == key) {
                Some((_, value)) => output.extend_from_slice(value),
                None => output.extend_from_slice(&input[index..=end]),
            }
            index = end + 1;
        }
        (output.len() <= capacity).then_some(output)
    }

    fn run_kernel(input: &[u8], pairs: &[(&[u8], &[u8])], capacity: usize) -> Option<Vec<u8>> {
        let descriptors: Vec<Pattern> = pairs
            .iter()
            .map(|(key, value)| Pattern {
                key_ptr: key.as_ptr(),
                key_len: key.len(),
                value_ptr: value.as_ptr(),
                value_len: value.len(),
            })
            .collect();
        let mut storage = vec![CANARY; capacity + 64];
        // SAFETY: Every slice is live and initialized; the output buffer has
        // `capacity` writable bytes and does not alias any source.
        let written = unsafe {
            interpolate(
                input.as_ptr(),
                input.len(),
                descriptors.as_ptr(),
                descriptors.len(),
                storage.as_mut_ptr(),
                capacity,
            )
        };
        assert!(
            storage[capacity..].iter().all(|&b| b == CANARY),
            "kernel wrote past capacity {capacity} for input={:?}",
            String::from_utf8_lossy(input)
        );
        if written == usize::MAX {
            return None;
        }
        assert!(written <= capacity);
        storage.truncate(written);
        Some(storage)
    }

    fn check(input: &[u8], pairs: &[(&[u8], &[u8])], capacity: usize) {
        let expected = model(input, pairs, capacity);
        let actual = run_kernel(input, pairs, capacity);
        assert_eq!(
            actual.as_deref().map(String::from_utf8_lossy),
            expected.as_deref().map(String::from_utf8_lossy),
            "input={:?} capacity={capacity} pairs={}",
            String::from_utf8_lossy(input),
            pairs.len()
        );
    }

    fn check_against_legacy(input: &str, patterns: &[&str], values: &[Cow<'_, str>]) {
        let legacy = crate::replace_patterns_legacy(input, patterns, values);
        if let Some(actual) = super::super::try_replace_patterns_cow(input, patterns, values) {
            assert_eq!(actual, legacy, "input={input:?} patterns={patterns:?}");
        }
        assert_eq!(
            super::super::replace_patterns_cow(input, patterns, values),
            legacy,
            "input={input:?} patterns={patterns:?}"
        );
    }

    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, bound: usize) -> usize {
            (self.next() % bound as u64) as usize
        }
    }

    const KEYS: &[&str] = &[
        "",
        "a",
        "ab",
        "abc",
        "abcd",
        "abcdefg",
        "abcdefgh",
        "abcdefghi",
        "abcdefghijklmnop",
        "abcdefghijklmnopq",
        "名字",
        "name",
        "nam",
        "namex",
    ];

    const PIECES: &[&str] = &[
        "%", "{", "}", "%{", "%%", "}}", "x", "hello ", "é", "🙂", "世界", "%{a", "%{}",
    ];

    fn random_value(rng: &mut Rng) -> String {
        let len = match rng.below(4) {
            0 => rng.below(4),
            1 => rng.below(17),
            2 => rng.below(40),
            _ => rng.below(260),
        };
        (0..len)
            .map(|i| match (rng.next() + i as u64) % 5 {
                0 => '界',
                1 => '%',
                _ => char::from(b'a' + (i % 26) as u8),
            })
            .collect()
    }

    fn random_input(rng: &mut Rng, keys: &[&str]) -> String {
        let mut input = String::new();
        for _ in 0..rng.below(12) {
            match rng.below(5) {
                0 | 1 => {
                    let key = if rng.below(4) == 0 {
                        KEYS[rng.below(KEYS.len())]
                    } else if keys.is_empty() {
                        "missing"
                    } else {
                        keys[rng.below(keys.len())]
                    };
                    input.push_str("%{");
                    input.push_str(key);
                    input.push('}');
                }
                2 => input.push_str(PIECES[rng.below(PIECES.len())]),
                _ => {
                    for _ in 0..rng.below(40) {
                        input.push(char::from(b'a' + rng.below(26) as u8));
                    }
                }
            }
        }
        // Keep most inputs well formed so the kernel path is exercised.
        if rng.below(3) != 0 {
            input = input.replace("%%", "").replace("%{a%", "%{a}");
        }
        input
    }

    #[test]
    fn randomized_kernel_matches_model_and_legacy() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let mut accepted = 0;
        for _ in 0..20_000 {
            let count = rng.below(12);
            let keys: Vec<&str> = (0..count).map(|_| KEYS[rng.below(KEYS.len())]).collect();
            let values: Vec<String> = (0..count).map(|_| random_value(&mut rng)).collect();
            let input = random_input(&mut rng, &keys);
            let pairs: Vec<(&[u8], &[u8])> = keys
                .iter()
                .zip(&values)
                .map(|(key, value)| (key.as_bytes(), value.as_bytes()))
                .collect();

            let capacity = match rng.below(3) {
                0 => input.len() + 128,
                1 => rng.below(input.len() + 300),
                _ => model(input.as_bytes(), &pairs, usize::MAX).map_or(0, |out| out.len()),
            };
            check(input.as_bytes(), &pairs, capacity);
            accepted += usize::from(model(input.as_bytes(), &pairs, capacity).is_some());

            let cows: Vec<Cow<'_, str>> =
                values.iter().map(|v| Cow::Borrowed(v.as_str())).collect();
            check_against_legacy(&input, &keys, &cows);
            // Unpaired names or values follow `zip` semantics.
            check_against_legacy(&input, &keys, &cows[..count / 2]);
        }
        assert!(accepted > 5_000, "too few accepted inputs: {accepted}");
    }

    #[test]
    fn every_length_and_alignment_with_boundary_sentinels() {
        let long_value = "v".repeat(129);
        let values: [&[u8]; 3] = [b"", b"VALUE", long_value.as_bytes()];
        for len in 0..=64_usize {
            for offset in 0..16 {
                for marker in [None, Some(0), Some(len / 2), Some(len.saturating_sub(4))] {
                    let mut storage = vec![b'%'; offset + len + 16];
                    let body = &mut storage[offset..offset + len];
                    body.fill(b'x');
                    if let Some(at) = marker.filter(|&at| at + 4 <= len) {
                        body[at..at + 4].copy_from_slice(b"%{k}");
                    }
                    let input = &storage[offset..offset + len];
                    for value in values {
                        let pairs = [(&b"k"[..], value)];
                        for capacity in [0, len / 2, len, len + 8, len + 16, len + 200] {
                            check(input, &pairs, capacity);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn every_value_length_and_source_alignment() {
        let source: Vec<u8> = (0..300).map(|i| b'A' + (i % 26) as u8).collect();
        for offset in 0..16 {
            for len in 0..=260 {
                let value = &source[offset..offset + len];
                for input in [&b"%{k}"[..], b"ab%{k}cd", b"0123456789abcdef%{k}0123456789"] {
                    let pairs = [(&b"q"[..], &b"no"[..]), (&b"k"[..], value)];
                    for capacity in [len, len + 30, len + 64] {
                        check(input, &pairs, capacity);
                    }
                }
            }
        }
    }

    #[test]
    fn keys_of_every_length_match_only_exactly() {
        let base: Vec<u8> = (0..40).map(|i| b'a' + (i % 26) as u8).collect();
        for len in 0..=33_usize {
            let key = &base[..len];
            let mut input = b"%{".to_vec();
            input.extend_from_slice(key);
            input.push(b'}');
            for flip in 0..len {
                let mut other = key.to_vec();
                other[flip] ^= 0x20;
                let pairs = [(&other[..], &b"WRONG"[..]), (key, &b"RIGHT"[..])];
                check(&input, &pairs, 64);
            }
            let shorter = &base[..len.saturating_sub(1)];
            let longer = &base[..len + 1];
            let pairs = [(shorter, &b"S"[..]), (longer, &b"L"[..]), (key, &b"K"[..])];
            check(&input, &pairs, 64);
            // Duplicate keys: the first descriptor wins.
            let pairs = [(key, &b"first"[..]), (key, &b"second"[..])];
            check(&input, &pairs, 64);
        }
    }

    #[test]
    fn malformed_markers_fall_back_to_legacy() {
        let values = [Cow::Borrowed("A"), Cow::Borrowed("")];
        for input in [
            "%",
            "%{",
            "abc%{",
            "%{a",
            "%{a%{x}",
            "%%{a}",
            "%x{a}",
            "%{a}%",
            "}{%{a}}",
            "0123456789abcdef%",
            "0123456789abcdef%{",
            "0123456789abcdef%{abcdefghijk",
        ] {
            check_against_legacy(input, &["a", "x"], &values);
        }
        assert!(super::super::try_replace_patterns_cow("%{a}%", &["a"], &values).is_none());
        assert_eq!(
            super::super::try_replace_patterns_cow("%{}%{a}}", &["", "a"], &values).as_deref(),
            Some("A}")
        );
    }

    #[test]
    fn more_than_eight_descriptors_in_kernel() {
        let names: Vec<String> = (0..20).map(|i| format!("key{i}")).collect();
        let values: Vec<String> = (0..20).map(|i| format!("value{i}")).collect();
        let pairs: Vec<(&[u8], &[u8])> = names
            .iter()
            .zip(&values)
            .map(|(k, v)| (k.as_bytes(), v.as_bytes()))
            .collect();
        let input: String = names.iter().rev().map(|k| format!("[%{{{k}}}]")).collect();
        check(input.as_bytes(), &pairs, 1024);
    }
}
