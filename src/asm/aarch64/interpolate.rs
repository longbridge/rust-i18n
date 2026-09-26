use std::arch::asm;

use super::Pattern;

/// Interpolates complete `%{key}` markers into caller-owned spare capacity.
///
/// Returns the initialized byte count, or `usize::MAX` to request the Rust
/// fallback. On fallback, the output may contain an incomplete prefix that
/// must be discarded. The first equal descriptor wins; unknown markers are
/// copied unchanged. Stray percent signs, nested percent signs and incomplete
/// markers deliberately use the existing Rust parser for historical behavior.
///
/// # Safety
///
/// - Input and descriptor pointers must identify initialized readable slices
///   of the supplied lengths. Each descriptor must contain valid readable key
///   and value byte slices. Zero-length slices may use dangling nonnull pointers.
/// - `output` must identify `capacity` writable bytes in one allocation,
///   disjoint from input, descriptor storage, and every key/value slice.
/// - All slices must satisfy Rust slice size bounds (`isize::MAX`).
/// - `Pattern` must have the documented 64-bit C layout: key pointer and length
///   at offsets 0 and 8, value pointer and length at offsets 16 and 24.
///
/// The safe runtime adapter validates lengths and builds descriptors from
/// borrowed strings before calling this kernel. Complete string input and
/// replacements produce UTF-8 output, since marker boundaries are ASCII.
pub(super) unsafe fn interpolate(
    input: *const u8,
    input_len: usize,
    patterns: *const Pattern,
    pattern_count: usize,
    output: *mut u8,
    capacity: usize,
) -> usize {
    let written: usize;
    // SAFETY: The caller supplies the valid, disjoint slices above. Every input
    // byte load checks its end pointer; descriptor loads check the count; key
    // comparisons check key length; copies check remaining output capacity.
    // All mutated registers and condition flags are declared. No stack access,
    // external calls, retained pointers, or reserved x18 register are used.
    unsafe {
        asm!(
            include_str!("interpolate.asm"),
            inlateout("x0") input => written,
            in("x1") input.add(input_len),
            in("x2") patterns,
            in("x3") pattern_count,
            inlateout("x4") output => _,
            in("x5") output.add(capacity),
            out("x6") _,
            out("x7") _,
            out("x8") _,
            out("x9") _,
            out("x10") _,
            out("x11") _,
            out("x12") _,
            out("x13") _,
            out("x14") _,
            out("x15") _,
            out("x16") _,
            out("x17") _,
            options(nostack),
        );
    }
    written
}
