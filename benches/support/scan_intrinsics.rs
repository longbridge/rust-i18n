//! SIMD intrinsics scanner for `scripts/benchmark-asm.py`.
//!
//! The script copies this file into an export's `src/asm/`. It matches the
//! naked `.asm` scanners in block width, dispatch, and scalar tail, so the
//! comparison isolates hand-written instructions from LLVM's code generation.

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::{vceqq_u8, vdupq_n_u8, vld1q_u8, vmaxvq_u8};
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::{
    _mm256_cmpeq_epi8, _mm256_loadu_si256, _mm256_movemask_epi8, _mm256_set1_epi8, _mm_cmpeq_epi8,
    _mm_loadu_si128, _mm_movemask_epi8, _mm_set1_epi8,
};

/// Returns the index of the first percent byte.
#[cfg(target_arch = "x86_64")]
pub(crate) fn find_percent(bytes: &[u8]) -> Option<usize> {
    if std::is_x86_feature_detected!("avx2") {
        // SAFETY: The runtime check ensures AVX2 is available.
        unsafe { find_percent_avx2(bytes) }
    } else {
        find_percent_sse2(bytes)
    }
}

#[cfg(target_arch = "x86_64")]
fn find_percent_sse2(bytes: &[u8]) -> Option<usize> {
    // SAFETY: SSE2 is part of the x86-64 baseline.
    let needle = unsafe { _mm_set1_epi8(b'%' as i8) };
    let (chunks, tail) = bytes.as_chunks::<16>();

    for (chunk_index, chunk) in chunks.iter().enumerate() {
        // SAFETY: `chunk` contains exactly 16 initialized bytes and an
        // unaligned 16-byte load reads no further.
        let mask = unsafe {
            let data = _mm_loadu_si128(chunk.as_ptr().cast());
            _mm_movemask_epi8(_mm_cmpeq_epi8(data, needle)) as u32
        };
        if mask != 0 {
            return Some(chunk_index * 16 + mask.trailing_zeros() as usize);
        }
    }

    tail.iter()
        .position(|&byte| byte == b'%')
        .map(|index| bytes.len() - tail.len() + index)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn find_percent_avx2(bytes: &[u8]) -> Option<usize> {
    let needle = _mm256_set1_epi8(b'%' as i8);
    let (chunks, tail) = bytes.as_chunks::<32>();

    for (chunk_index, chunk) in chunks.iter().enumerate() {
        // SAFETY: `chunk` contains exactly 32 initialized bytes and an
        // unaligned 32-byte load reads no further. AVX2 was checked above.
        let mask = unsafe {
            let data = _mm256_loadu_si256(chunk.as_ptr().cast());
            _mm256_movemask_epi8(_mm256_cmpeq_epi8(data, needle)) as u32
        };
        if mask != 0 {
            return Some(chunk_index * 32 + mask.trailing_zeros() as usize);
        }
    }

    tail.iter()
        .position(|&byte| byte == b'%')
        .map(|index| bytes.len() - tail.len() + index)
}

/// Returns the index of the first percent byte.
#[cfg(target_arch = "aarch64")]
pub(crate) fn find_percent(bytes: &[u8]) -> Option<usize> {
    let (chunks, tail) = bytes.as_chunks::<16>();
    for (chunk_index, chunk) in chunks.iter().enumerate() {
        // SAFETY: NEON is baseline on AArch64. The unaligned load reads
        // exactly the 16 initialized bytes in `chunk`.
        let has_match = unsafe {
            let block = vld1q_u8(chunk.as_ptr());
            vmaxvq_u8(vceqq_u8(block, vdupq_n_u8(b'%')))
        };
        if has_match != 0 {
            return chunk
                .iter()
                .position(|&byte| byte == b'%')
                .map(|index| chunk_index * 16 + index);
        }
    }
    tail.iter()
        .position(|&byte| byte == b'%')
        .map(|index| bytes.len() - tail.len() + index)
}
