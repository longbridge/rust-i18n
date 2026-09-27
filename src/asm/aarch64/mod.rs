use std::arch::asm;

/// Returns the byte index of the first percent sign.
pub(super) fn find_percent(bytes: &[u8]) -> Option<usize> {
    let mut chunks = bytes.chunks_exact(16);

    for (chunk_index, chunk) in chunks.by_ref().enumerate() {
        let has_match: u32;

        // SAFETY: `chunks_exact(16)` yields a slice of 16 initialized bytes,
        // so the single 16-byte `ldr q0` cannot read beyond `bytes`. The
        // assembly only reads through the pointer and does not retain it.
        // `v0` and `v1` are declared clobbered. The pointer and result use
        // compiler-allocated general registers, avoiding reserved registers.
        unsafe {
            asm!(
                include_str!("find_percent.asm"),
                ptr = in(reg) chunk.as_ptr(),
                has_match = lateout(reg) has_match,
                out("v0") _,
                out("v1") _,
                options(nostack, readonly),
            );
        }

        if has_match != 0 {
            // The vector reduction only says whether a match exists. Search
            // this bounded chunk to recover the first matching byte index.
            return chunk
                .iter()
                .position(|&byte| byte == b'%')
                .map(|index| chunk_index * 16 + index);
        }
    }

    let tail = chunks.remainder();
    tail.iter()
        .position(|&byte| byte == b'%')
        .map(|index| bytes.len() - tail.len() + index)
}

#[cfg(test)]
mod tests {
    use super::find_percent;

    #[test]
    fn finds_first_match_at_chunk_edges_and_in_tail() {
        for index in [0, 15, 16, 31, 32, 47, 48] {
            let mut bytes = vec![b'x'; 49];
            bytes[index] = b'%';
            assert_eq!(find_percent(&bytes), Some(index));
        }
    }

    #[test]
    fn finds_first_of_multiple_matches() {
        let mut bytes = vec![b'x'; 49];
        bytes[17] = b'%';
        bytes[48] = b'%';
        assert_eq!(find_percent(&bytes), Some(17));
    }

    #[test]
    fn handles_short_and_empty_inputs() {
        assert_eq!(find_percent(b""), None);
        assert_eq!(find_percent(b"abc"), None);
        assert_eq!(find_percent(b"abc%"), Some(3));
    }
}
