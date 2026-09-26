use std::arch::asm;

/// Experimental AArch64 scan: 16-byte SIMD blocks followed by a scalar tail.
///
/// Keep the block algorithm matched to `intrinsics.rs` so measurements isolate
/// instruction selection rather than a different search algorithm.
pub fn find_percent(bytes: &[u8]) -> Option<usize> {
    let mut chunks = bytes.chunks_exact(16);
    for (chunk_index, chunk) in chunks.by_ref().enumerate() {
        let has_match: u32;
        // SAFETY: The load reads exactly the 16 initialized bytes in `chunk`;
        // AArch64 permits unaligned loads from this ordinary byte slice. No
        // pointer escapes. v0/v1 are clobbered; the general-register output is
        // written after the pointer input is consumed, permitting lateout.
        // Compiler-selected registers avoid Apple's reserved x18 register.
        unsafe {
            asm!(
                include_str!("find_percent_neon.asm"),
                ptr = in(reg) chunk.as_ptr(),
                has_match = lateout(reg) has_match,
                out("v0") _,
                out("v1") _,
                options(nostack, readonly),
            );
        }
        if has_match != 0 {
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
