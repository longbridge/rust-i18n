//! Assembly-backed routines.
//!
//! This directory root holds the Rust adapters, naked function declarations,
//! and tests. Each `<arch>/` directory holds the `.asm` instruction files.
//! Instruction text never appears inline in `.rs` files.

// The full interpolation kernel has its own safe adapter. Rejected input uses
// the portable scanner below instead of the CPU-sensitive legacy REPNE scan.
pub(crate) mod interpolate;

/// Production scanner used by the Rust interpolation path.
///
/// `scripts/benchmark-asm.py` replaces this function in isolated exports to
/// compare the scanner candidates declared below.
pub(crate) fn find_percent(bytes: &[u8]) -> Option<usize> {
    bytes.iter().position(|&byte| byte == b'%')
}

// Benchmark-only naked `.asm` scanners, compiled for tests so every scanner
// stays assembled and checked. `scripts/benchmark-asm.py` enables the module
// in its exports.
#[cfg(all(
    test,
    target_pointer_width = "64",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod scan;

#[cfg(test)]
mod interpolation_tests;
#[cfg(test)]
mod scan_tests;

#[cfg(test)]
mod tests {
    use super::find_percent;

    #[test]
    fn empty_input_has_no_match() {
        assert_eq!(find_percent(b""), None);
    }

    #[test]
    fn finds_last_byte() {
        assert_eq!(find_percent(b"hello%"), Some(5));
    }

    #[test]
    fn no_match() {
        assert_eq!(find_percent(b"hello"), None);
    }

    #[test]
    fn finds_first_of_multiple_matches_in_long_input() {
        let mut input = vec![b'x'; 4096];
        input[3072] = b'%';
        input[4095] = b'%';
        assert_eq!(find_percent(&input), Some(3072));
    }

    #[test]
    fn utf8_bytes_preserve_byte_offsets() {
        assert_eq!(find_percent("é🙂%".as_bytes()), Some(6));
    }

    #[test]
    fn finds_matches_across_assembly_threshold() {
        for len in [255, 256, 257] {
            let mut bytes = vec![b'x'; len];
            bytes[len - 1] = b'%';
            assert_eq!(find_percent(&bytes), Some(len - 1));
        }
    }

    #[test]
    fn matches_iterator_at_every_position_and_alignment() {
        for len in [15, 16, 17, 255, 256, 257, 511] {
            for offset in 0..16 {
                let mut storage = vec![b'x'; offset + len + 16];
                let bytes = &mut storage[offset..offset + len];
                assert_eq!(find_percent(bytes), bytes.iter().position(|&b| b == b'%'));

                for position in 0..len {
                    bytes[position] = b'%';
                    assert_eq!(
                        find_percent(bytes),
                        bytes.iter().position(|&b| b == b'%'),
                        "len={len}, offset={offset}, position={position}"
                    );
                    bytes[position] = b'x';
                }
            }
        }
    }
}
