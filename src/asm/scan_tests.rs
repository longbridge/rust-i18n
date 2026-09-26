//! Differential tests for percent scanners.
//!
//! `super::find_percent` is the active scanner: scalar Rust in production, or
//! the candidate selected by `scripts/benchmark-asm.py` in its exports. The
//! naked `.asm` scanners in `super::scan` are checked directly.

fn scalar_find_percent(bytes: &[u8]) -> Option<usize> {
    bytes.iter().position(|&byte| byte == b'%')
}

fn assert_matches_scalar(name: &str, find_percent: fn(&[u8]) -> Option<usize>) {
    const LENGTHS: &[usize] = &[
        0, 1, 15, 16, 17, 31, 32, 33, 47, 48, 63, 64, 65, 255, 256, 257, 511, 4096,
    ];

    for &len in LENGTHS {
        for offset in 0..32 {
            let mut storage = vec![b'a'; offset + len + 32];
            // Percent bytes outside the slice catch a scanner that fails to
            // mask its first or final partial vector.
            if offset > 0 {
                storage[offset - 1] = b'%';
            }
            storage[offset + len] = b'%';

            let bytes = &mut storage[offset..offset + len];
            assert_eq!(
                find_percent(bytes),
                scalar_find_percent(bytes),
                "{name}: len={len}, offset={offset}, no match"
            );

            let positions: Vec<usize> = if len <= 511 {
                (0..len).collect()
            } else {
                vec![0, len / 2, len - 1]
            };
            for position in positions {
                bytes[position] = b'%';
                assert_eq!(
                    find_percent(bytes),
                    scalar_find_percent(bytes),
                    "{name}: len={len}, offset={offset}, position={position}"
                );
                bytes[position] = b'a';
            }

            if len >= 3 {
                bytes[0] = b'%';
                bytes[len / 2] = b'%';
                bytes[len - 1] = b'%';
                assert_eq!(
                    find_percent(bytes),
                    scalar_find_percent(bytes),
                    "{name}: len={len}, offset={offset}, multiple matches"
                );
            }
        }
    }

    let mut text = "é🙂".repeat(128).into_bytes();
    assert_eq!(find_percent(&text), scalar_find_percent(&text), "{name}");
    text.push(b'%');
    assert_eq!(find_percent(&text), scalar_find_percent(&text), "{name}");
    text.push(b'x');
    assert_eq!(find_percent(&text), scalar_find_percent(&text), "{name}");
}

#[test]
fn active_scanner_matches_scalar() {
    assert_matches_scalar("active", super::find_percent);
}

#[cfg(all(
    target_pointer_width = "64",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod naked {
    use super::super::scan;
    use super::assert_matches_scalar;

    #[cfg(target_arch = "x86_64")]
    fn found(index: usize) -> Option<usize> {
        (index != usize::MAX).then_some(index)
    }

    #[test]
    fn legacy_scanner_matches_scalar() {
        assert_matches_scalar("legacy", scan::find_percent_legacy);
    }

    #[test]
    fn simd_scanner_matches_scalar() {
        assert_matches_scalar("simd", scan::find_percent_simd);
    }

    // `find_percent_simd` picks AVX2 when available, so check SSE2 directly.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn sse2_scanner_matches_scalar() {
        // SAFETY: The slice is readable for its length; SSE2 is baseline.
        assert_matches_scalar("sse2", |bytes| {
            found(unsafe { scan::find_percent_sse2(bytes.as_ptr(), bytes.len()) })
        });
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn avx2_scanner_matches_scalar() {
        if !std::is_x86_feature_detected!("avx2") {
            return;
        }
        // SAFETY: The slice is readable for its length; AVX2 was checked.
        assert_matches_scalar("avx2", |bytes| {
            found(unsafe { scan::find_percent_avx2(bytes.as_ptr(), bytes.len()) })
        });
    }

    // Empty input with a dangling pointer must not be read.
    #[test]
    fn empty_input_with_dangling_pointer() {
        let dangling = std::ptr::NonNull::<u8>::dangling().as_ptr();
        // SAFETY: A zero-length slice at a dangling, aligned pointer is valid.
        let empty = unsafe { std::slice::from_raw_parts(dangling, 0) };
        assert_eq!(scan::find_percent_legacy(empty), None);
        assert_eq!(scan::find_percent_simd(empty), None);
    }
}
