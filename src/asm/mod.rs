#[cfg(target_arch = "aarch64")]
mod aarch64;
#[cfg(target_arch = "x86_64")]
mod x86_64;

/// Returns the index of the first percent byte in `bytes`.
pub(crate) fn find_percent(bytes: &[u8]) -> Option<usize> {
    // Short messages are common in translations. Avoid assembly setup for them.
    if bytes.len() < 256 {
        return bytes.iter().position(|&byte| byte == b'%');
    }

    #[cfg(target_arch = "x86_64")]
    {
        x86_64::find_percent(bytes)
    }
    #[cfg(target_arch = "aarch64")]
    {
        aarch64::find_percent(bytes)
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    bytes.iter().position(|&byte| byte == b'%')
}

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
