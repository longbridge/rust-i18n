#[cfg(target_arch = "x86_64")]
use std::arch::asm;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::{_mm256_set1_epi8, _mm_set1_epi8};

/// Return the first percent byte. The public function performs the same AVX2
/// dispatch as the matching `intrinsics.rs` experiment.
pub fn find_percent(bytes: &[u8]) -> Option<usize> {
    #[cfg(target_arch = "x86_64")]
    {
        if std::is_x86_feature_detected!("avx2") {
            // SAFETY: The runtime check ensures AVX2 is available.
            return unsafe { find_percent_avx2(bytes) };
        }
        return find_percent_sse2(bytes);
    }

    #[cfg(not(target_arch = "x86_64"))]
    bytes.iter().position(|&byte| byte == b'%')
}

#[cfg(target_arch = "x86_64")]
fn find_percent_sse2(bytes: &[u8]) -> Option<usize> {
    // SAFETY: SSE2 is part of the x86-64 baseline.
    let needle = unsafe { _mm_set1_epi8(b'%' as i8) };
    let mut chunks = bytes.chunks_exact(16);

    for (chunk_index, chunk) in chunks.by_ref().enumerate() {
        let mask: u32;
        // SAFETY: `chunk` contains exactly 16 initialized bytes. `movdqu`
        // reads only those bytes. `data` declares the written SIMD register;
        // the other instructions operate only on registers.
        unsafe {
            asm!(
                include_str!("find_percent_sse2.asm"),
                ptr = in(reg) chunk.as_ptr(),
                needle = in(xmm_reg) needle,
                data = out(xmm_reg) _,
                mask = lateout(reg) mask,
                options(nostack, readonly),
            );
        }
        if mask != 0 {
            return Some(chunk_index * 16 + mask.trailing_zeros() as usize);
        }
    }

    let tail = chunks.remainder();
    tail.iter()
        .position(|&byte| byte == b'%')
        .map(|index| bytes.len() - tail.len() + index)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn find_percent_avx2(bytes: &[u8]) -> Option<usize> {
    // The public wrapper checks AVX2 before entering this target-feature function.
    let needle = _mm256_set1_epi8(b'%' as i8);
    let mut chunks = bytes.chunks_exact(32);

    for (chunk_index, chunk) in chunks.by_ref().enumerate() {
        let mask: u32;
        // SAFETY: `chunk` contains exactly 32 initialized bytes. `vmovdqu`
        // reads only those bytes. `data` declares the written SIMD register;
        // the other instructions operate only on registers.
        unsafe {
            asm!(
                include_str!("find_percent_avx2.asm"),
                ptr = in(reg) chunk.as_ptr(),
                needle = in(ymm_reg) needle,
                data = out(ymm_reg) _,
                mask = lateout(reg) mask,
                options(nostack, readonly),
            );
        }
        if mask != 0 {
            return Some(chunk_index * 32 + mask.trailing_zeros() as usize);
        }
    }

    let tail = chunks.remainder();
    tail.iter()
        .position(|&byte| byte == b'%')
        .map(|index| bytes.len() - tail.len() + index)
}
