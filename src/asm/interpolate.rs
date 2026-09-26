//! Safe adapter for the assembly interpolation kernels.
//!
//! `replace_patterns_cow` is the crate's production Cow entry point. The
//! original Rust parser is retained as the fallback for rejected inputs.

use std::borrow::Cow;

#[cfg(all(
    target_pointer_width = "64",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
use std::mem::MaybeUninit;

/// Layout consumed by both architecture kernels. Only the initialized prefix
/// of a stack array is passed to assembly. The kernels use 64-bit field offsets.
#[cfg(all(
    target_pointer_width = "64",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[repr(C)]
#[derive(Clone, Copy)]
#[allow(dead_code)] // Fields are read by the architecture assembly kernels.
pub(super) struct Pattern {
    key_ptr: *const u8,
    key_len: usize,
    value_ptr: *const u8,
    value_len: usize,
}

/// Kernel result, returned in two registers (`rax:rdx` or `x0:x1`).
///
/// `written == usize::MAX` rejects the input for the Rust parser. Otherwise
/// `written` output bytes are initialized, and `consumed < input_len` means
/// the output filled up before the remaining input could be copied.
#[cfg(all(
    target_pointer_width = "64",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[repr(C)]
struct Progress {
    written: usize,
    consumed: usize,
}

// The ABI is documented at the top of the `.asm` file.
#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
#[unsafe(naked)]
unsafe extern "sysv64" fn kernel(
    input: *const u8,
    input_len: usize,
    patterns: *const Pattern,
    pattern_count: usize,
    output: *mut u8,
    capacity: usize,
) -> Progress {
    core::arch::naked_asm!(include_str!("x86_64/interpolate.asm"))
}

#[cfg(all(target_arch = "aarch64", target_pointer_width = "64"))]
#[path = "aarch64/interpolate.rs"]
mod arch;

/// Adapts the AArch64 kernel, which completes or rejects without resuming.
#[cfg(all(target_arch = "aarch64", target_pointer_width = "64"))]
unsafe fn kernel(
    input: *const u8,
    input_len: usize,
    patterns: *const Pattern,
    pattern_count: usize,
    output: *mut u8,
    capacity: usize,
) -> Progress {
    // SAFETY: The caller upholds the shared kernel contract.
    let written =
        unsafe { arch::interpolate(input, input_len, patterns, pattern_count, output, capacity) };
    Progress {
        written,
        consumed: input_len,
    }
}

/// Slack reserved beyond the input length, matching the Rust parser.
const SLACK: usize = 128;
/// Paired arguments accepted by the single-shot `try_replace_patterns_cow`.
#[cfg(test)]
const TRY_PATTERN_LIMIT: usize = 8;
/// Paired arguments the production path passes to the kernel from the stack.
const PATTERN_LIMIT: usize = 16;

pub(crate) fn replace_patterns_cow(
    input: &str,
    patterns: &[&str],
    values: &[Cow<'_, str>],
) -> String {
    interpolate(input, patterns, values, PATTERN_LIMIT, true)
        .unwrap_or_else(|| crate::replace_patterns_impl(input, patterns, values))
}

/// One bounded kernel attempt: at most eight pairs and `input.len() + 128`
/// bytes of output, without growing. `Some` means the kernel completed;
/// `None` means the production path would grow the output or use the Rust
/// parser. Exposed within the crate for forced-path tests.
#[cfg(test)]
pub(crate) fn try_replace_patterns_cow(
    input: &str,
    patterns: &[&str],
    values: &[Cow<'_, str>],
) -> Option<String> {
    interpolate(input, patterns, values, TRY_PATTERN_LIMIT, false)
}

#[cfg(all(
    target_pointer_width = "64",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn interpolate<V: AsRef<str>>(
    input: &str,
    patterns: &[&str],
    values: &[V],
    limit: usize,
    grow: bool,
) -> Option<String> {
    // The Rust parser pairs entries with `zip`, so unpaired names or values
    // are ignored. Larger paired sets use the Rust path.
    let count = patterns.len().min(values.len());
    if count > limit {
        return None;
    }

    let mut descriptors = [MaybeUninit::<Pattern>::uninit(); PATTERN_LIMIT];
    for (slot, (&key, value)) in descriptors
        .iter_mut()
        .zip(patterns.iter().zip(values.iter()))
    {
        let value: &str = value.as_ref();
        slot.write(Pattern {
            key_ptr: key.as_ptr(),
            key_len: key.len(),
            value_ptr: value.as_ptr(),
            value_len: value.len(),
        });
    }

    let bytes = input.as_bytes();
    let mut consumed = 0;
    // Match the Rust parser's initial reservation. The allocation is separate
    // from the input, descriptors, and every key/value slice.
    let mut output = Vec::<u8>::with_capacity(bytes.len().checked_add(SLACK)?);
    loop {
        let rest = &bytes[consumed..];
        let spare = output.spare_capacity_mut();
        // SAFETY: `rest` is readable, exactly `count` descriptors are
        // initialized and point to live key/value strings, and `spare` is
        // writable and aliases no source. The kernel retains no pointer.
        let progress = unsafe {
            kernel(
                rest.as_ptr(),
                rest.len(),
                descriptors.as_ptr().cast::<Pattern>(),
                count,
                spare.as_mut_ptr().cast::<u8>(),
                spare.len(),
            )
        };
        if progress.written > spare.len() || progress.consumed > rest.len() {
            return None;
        }
        // SAFETY: The kernel initialized `written` spare bytes. They are a
        // concatenation of input bytes, whole UTF-8 values, and whole ASCII
        // markers, in input order, so the complete output is UTF-8. Partial
        // output stays in `Vec<u8>` until then.
        unsafe { output.set_len(output.len() + progress.written) };
        consumed += progress.consumed;
        if consumed == bytes.len() {
            // SAFETY: Complete; see above.
            return Some(unsafe { String::from_utf8_unchecked(output) });
        }
        if !grow {
            return None;
        }
        // The next literal chunk or value did not fit. Grow at least enough
        // for the rest of the input plus the largest value.
        let largest = values.iter().map(|v| v.as_ref().len()).max().unwrap_or(0);
        output.reserve(
            (bytes.len() - consumed)
                .checked_add(largest)?
                .checked_add(SLACK)?,
        );
    }
}

#[cfg(not(all(
    target_pointer_width = "64",
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
fn interpolate<V: AsRef<str>>(
    input: &str,
    patterns: &[&str],
    values: &[V],
    limit: usize,
    grow: bool,
) -> Option<String> {
    let _ = (input, patterns, values, limit, grow);
    None
}

#[cfg(all(test, target_arch = "x86_64", target_pointer_width = "64"))]
mod tests {
    use super::{kernel, Pattern};
    use std::borrow::Cow;

    const CANARY: u8 = 0xA5;

    /// Byte-level model of the kernel contract: `None` means rejected.
    fn model(input: &[u8], pairs: &[(&[u8], &[u8])]) -> Option<Vec<u8>> {
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
            match pairs.iter().find(|(key, _)| *key == &input[start..end]) {
                Some((_, value)) => output.extend_from_slice(value),
                None => output.extend_from_slice(&input[index..=end]),
            }
            index = end + 1;
        }
        Some(output)
    }

    /// Runs the kernel with an exact `capacity`, resuming into fresh buffers
    /// grown by `step` bytes, and checks that no call writes past its end.
    fn run(
        input: &[u8],
        pairs: &[(&[u8], &[u8])],
        capacity: usize,
        step: usize,
    ) -> Option<Vec<u8>> {
        let descriptors: Vec<Pattern> = pairs
            .iter()
            .map(|(key, value)| Pattern {
                key_ptr: key.as_ptr(),
                key_len: key.len(),
                value_ptr: value.as_ptr(),
                value_len: value.len(),
            })
            .collect();
        let mut consumed = 0;
        let mut done = Vec::new();
        let mut capacity = capacity;
        for _ in 0..10_000 {
            let rest = &input[consumed..];
            let mut storage = vec![CANARY; capacity + 64];
            // SAFETY: All slices are live; `storage` has `capacity` writable
            // bytes and aliases no source.
            let progress = unsafe {
                kernel(
                    rest.as_ptr(),
                    rest.len(),
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
            if progress.written == usize::MAX {
                return None;
            }
            assert!(progress.written <= capacity);
            assert!(progress.consumed <= rest.len());
            done.extend_from_slice(&storage[..progress.written]);
            consumed += progress.consumed;
            if consumed == input.len() {
                return Some(done);
            }
            capacity = capacity * 2 + step;
        }
        panic!("kernel made no progress");
    }

    fn check(input: &[u8], pairs: &[(&[u8], &[u8])], capacity: usize, step: usize) {
        let expected = model(input, pairs);
        let actual = run(input, pairs, capacity, step);
        assert_eq!(
            actual.as_deref().map(String::from_utf8_lossy),
            expected.as_deref().map(String::from_utf8_lossy),
            "input={:?} capacity={capacity} step={step} pairs={}",
            String::from_utf8_lossy(input),
            pairs.len()
        );
    }

    fn check_adapter(input: &str, patterns: &[&str], values: &[Cow<'_, str>]) {
        let legacy = crate::replace_patterns_legacy(input, patterns, values);
        if let Some(actual) = super::try_replace_patterns_cow(input, patterns, values) {
            assert_eq!(actual, legacy, "input={input:?} patterns={patterns:?}");
        }
        assert_eq!(
            super::replace_patterns_cow(input, patterns, values),
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
        "abcdefghijklmnopqrstuvwxyz0123456789",
        "名字",
        "name",
        "nam",
        "namx",
        "city",
    ];

    const PIECES: &[&str] = &[
        "%", "{", "}", "%{", "%%", "}}", "x", "hello ", "é", "🙂", "世界", "%{a", "%{}", "%{a{b}",
    ];

    fn random_value(rng: &mut Rng) -> String {
        let len = match rng.below(4) {
            0 => rng.below(4),
            1 => rng.below(17),
            2 => rng.below(40),
            _ => rng.below(300),
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
        for _ in 0..rng.below(14) {
            match rng.below(5) {
                0 | 1 => {
                    let key = if rng.below(4) == 0 || keys.is_empty() {
                        KEYS[rng.below(KEYS.len())]
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
        if rng.below(3) != 0 {
            input = input.replace('%', "").replace("{", "%{");
        }
        input
    }

    #[test]
    fn randomized_kernel_matches_model_and_legacy() {
        let mut rng = Rng(0x2545_F491_4F6C_DD1D);
        let mut completed = 0;
        for _ in 0..30_000 {
            let count = rng.below(20);
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
                _ => model(input.as_bytes(), &pairs).map_or(0, |out| out.len()),
            };
            let step = 1 + rng.below(64);
            check(input.as_bytes(), &pairs, capacity, step);
            completed += usize::from(model(input.as_bytes(), &pairs).is_some());

            let cows: Vec<Cow<'_, str>> =
                values.iter().map(|v| Cow::Borrowed(v.as_str())).collect();
            check_adapter(&input, &keys, &cows);
            check_adapter(&input, &keys, &cows[..count / 2]);
        }
        assert!(
            completed > 10_000,
            "too few well-formed inputs: {completed}"
        );
    }

    #[test]
    fn every_length_and_alignment_with_boundary_sentinels() {
        let long_value = "v".repeat(129);
        let values: [&[u8]; 3] = [b"", b"VALUE", long_value.as_bytes()];
        for len in 0..=70_usize {
            for offset in 0..16 {
                for marker in [None, Some(0), Some(len / 2), Some(len.saturating_sub(4))] {
                    // `%` sentinels on both sides must never be read as input.
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
                            check(input, &pairs, capacity, 1);
                            check(input, &pairs, capacity, 17);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn every_value_length_and_source_alignment() {
        let source: Vec<u8> = (0..600).map(|i| b'A' + (i % 26) as u8).collect();
        for offset in 0..16 {
            for len in 0..=520 {
                let value = &source[offset..offset + len];
                for input in [&b"%{k}"[..], b"ab%{k}cd", b"0123456789abcdef%{k}0123456789"] {
                    let pairs = [(&b"q"[..], &b"no"[..]), (&b"k"[..], value)];
                    for capacity in [len, len + 30, len + 64] {
                        check(input, &pairs, capacity, 7);
                    }
                }
            }
        }
    }

    #[test]
    fn keys_of_every_length_match_only_exactly() {
        let base: Vec<u8> = (0..60).map(|i| b'a' + (i % 26) as u8).collect();
        for len in 0..=48_usize {
            let key = &base[..len];
            for prefix in [&b""[..], b"0123456789abcdef0123"] {
                let mut input = prefix.to_vec();
                input.extend_from_slice(b"%{");
                input.extend_from_slice(key);
                input.push(b'}');
                for flip in 0..len {
                    let mut other = key.to_vec();
                    other[flip] ^= 0x20;
                    let pairs = [(&other[..], &b"WRONG"[..]), (key, &b"RIGHT"[..])];
                    check(&input, &pairs, 128, 1);
                }
                let shorter = &base[..len.saturating_sub(1)];
                let longer = &base[..len + 1];
                let pairs = [(shorter, &b"S"[..]), (longer, &b"L"[..]), (key, &b"K"[..])];
                check(&input, &pairs, 128, 1);
                let pairs = [(key, &b"first"[..]), (key, &b"second"[..])];
                check(&input, &pairs, 128, 1);
            }
        }
    }

    #[test]
    fn production_path_grows_instead_of_falling_back() {
        let large = "界".repeat(512);
        let values = [Cow::Borrowed(large.as_str())];
        assert!(
            super::try_replace_patterns_cow("before %{name} after", &["name"], &values).is_none()
        );
        assert_eq!(
            super::replace_patterns_cow("before %{name} after", &["name"], &values),
            format!("before {large} after")
        );
        let names: Vec<String> = (0..16).map(|i| format!("k{i}")).collect();
        let patterns: Vec<&str> = names.iter().map(String::as_str).collect();
        let values: Vec<Cow<'_, str>> = (0..16).map(|i| Cow::Owned("v".repeat(i * 20))).collect();
        let input: String = names.iter().rev().map(|k| format!("[%{{{k}}}]")).collect();
        check_adapter(&input, &patterns, &values);
    }
}
