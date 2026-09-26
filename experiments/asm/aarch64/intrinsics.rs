use std::arch::aarch64::{vceqq_u8, vdupq_n_u8, vld1q_u8, vmaxvq_u8};

/// Experimental AArch64 scan with the same block algorithm as `asm.rs`.
///
/// Intended for AArch64 hosts with NEON, including Apple Silicon. Intrinsics
/// leave register allocation and loop-invariant instruction motion to LLVM.
pub fn find_percent(bytes: &[u8]) -> Option<usize> {
    let mut chunks = bytes.chunks_exact(16);
    for (chunk_index, chunk) in chunks.by_ref().enumerate() {
        // SAFETY: NEON is available on the Apple Silicon experiment target.
        // The unaligned load reads exactly 16 initialized bytes from `chunk`.
        // No pointer escapes and the other intrinsics access no memory.
        let has_match = unsafe {
            let block = vld1q_u8(chunk.as_ptr());
            let percent = vdupq_n_u8(b'%');
            vmaxvq_u8(vceqq_u8(block, percent))
        };
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
