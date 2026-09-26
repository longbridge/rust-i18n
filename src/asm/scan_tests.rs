#[cfg(test)]
mod experiment_tests {
    use super::find_percent;

    fn scalar_find_percent(bytes: &[u8]) -> Option<usize> {
        bytes.iter().position(|&byte| byte == b'%')
    }

    #[test]
    fn matches_scalar_at_boundaries_and_every_position() {
        const LENGTHS: &[usize] = &[0, 1, 15, 16, 17, 31, 32, 33, 255, 256, 257, 511, 4096];

        for &len in LENGTHS {
            for offset in 0..32 {
                let mut storage = vec![b'a'; offset + len + 32];
                // Percent bytes outside the slice catch a scanner that fails
                // to mask its first or final partial vector.
                if offset > 0 {
                    storage[offset - 1] = b'%';
                }
                storage[offset + len] = b'%';

                let bytes = &mut storage[offset..offset + len];
                assert_eq!(
                    find_percent(bytes),
                    scalar_find_percent(bytes),
                    "len={len}, offset={offset}, no match"
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
                        "len={len}, offset={offset}, position={position}"
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
                        "len={len}, offset={offset}, multiple matches"
                    );
                }
            }
        }
    }

    #[test]
    fn unicode_bytes_and_tail_match_scalar() {
        let mut text = "é🙂".repeat(128).into_bytes();
        assert_eq!(find_percent(&text), scalar_find_percent(&text));
        text.push(b'%');
        assert_eq!(find_percent(&text), scalar_find_percent(&text));
        text.push(b'x');
        assert_eq!(find_percent(&text), scalar_find_percent(&text));
    }
}
