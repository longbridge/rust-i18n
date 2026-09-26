//! Benchmark-only percent scanners.
//!
//! Each scanner is a naked function whose whole body is an `.asm` file under
//! `<arch>/`. The file follows the declared ABI itself and returns the index of
//! the first `%` byte, or `usize::MAX` when there is none. x86_64 kernels use
//! `sysv64` so Windows shares the same instructions.

//! # Safety
//!
//! Every kernel requires `ptr` to be readable for `len` bytes. It reads only
//! those bytes, writes no memory, and retains no pointer.

use core::arch::naked_asm;

#[cfg(target_arch = "x86_64")]
#[unsafe(naked)]
pub(super) unsafe extern "sysv64" fn find_percent_repne(ptr: *const u8, len: usize) -> usize {
    naked_asm!(include_str!("x86_64/find_percent_repne.asm"))
}

#[cfg(target_arch = "x86_64")]
#[unsafe(naked)]
pub(super) unsafe extern "sysv64" fn find_percent_sse2(ptr: *const u8, len: usize) -> usize {
    naked_asm!(include_str!("x86_64/find_percent_sse2.asm"))
}

/// Requires AVX2; `find_percent_simd` checks before calling.
#[cfg(target_arch = "x86_64")]
#[unsafe(naked)]
pub(super) unsafe extern "sysv64" fn find_percent_avx2(ptr: *const u8, len: usize) -> usize {
    naked_asm!(include_str!("x86_64/find_percent_avx2.asm"))
}

#[cfg(target_arch = "aarch64")]
#[unsafe(naked)]
pub(super) unsafe extern "C" fn find_percent_neon(ptr: *const u8, len: usize) -> usize {
    naked_asm!(include_str!("aarch64/find_percent_neon.asm"))
}

fn found(index: usize) -> Option<usize> {
    (index != usize::MAX).then_some(index)
}

/// Legacy scanner: `repne scasb` on x86_64, 16-byte NEON blocks on AArch64.
pub(crate) fn find_percent_legacy(bytes: &[u8]) -> Option<usize> {
    // SAFETY: Each kernel reads only `bytes.len()` bytes from `bytes.as_ptr()`,
    // writes no memory, and retains no pointer.
    #[cfg(target_arch = "x86_64")]
    let index = unsafe { find_percent_repne(bytes.as_ptr(), bytes.len()) };
    #[cfg(target_arch = "aarch64")]
    let index = unsafe { find_percent_neon(bytes.as_ptr(), bytes.len()) };
    found(index)
}

/// SIMD scanner matched with `benches/support/scan_intrinsics.rs`.
pub(crate) fn find_percent_simd(bytes: &[u8]) -> Option<usize> {
    // SAFETY: As in `find_percent_legacy`; AVX2 is checked before use.
    #[cfg(target_arch = "x86_64")]
    let index = unsafe {
        if std::is_x86_feature_detected!("avx2") {
            find_percent_avx2(bytes.as_ptr(), bytes.len())
        } else {
            find_percent_sse2(bytes.as_ptr(), bytes.len())
        }
    };
    #[cfg(target_arch = "aarch64")]
    let index = unsafe { find_percent_neon(bytes.as_ptr(), bytes.len()) };
    found(index)
}
