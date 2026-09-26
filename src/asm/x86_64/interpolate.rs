use super::Pattern;
use std::arch::asm;

/// Writes a complete interpolation to `output` or returns `usize::MAX` so the
/// caller can discard the partial output and use the legacy Rust parser.
///
/// # Safety
///
/// `input` must point to `input_len` initialized bytes; `patterns` must point
/// to `pattern_count` initialized descriptors whose key and value pointers
/// are readable for their declared lengths; `output` must point to
/// `capacity` writable bytes and must not overlap any readable source.
/// Readable sources may overlap one another. The caller must not expose output
/// bytes before a successful returned length has been checked against
/// `capacity`.
pub(super) unsafe fn interpolate(
    input: *const u8,
    input_len: usize,
    patterns: *const Pattern,
    pattern_count: usize,
    output: *mut u8,
    capacity: usize,
) -> usize {
    // SAFETY: The caller guarantees both input and output pointer/length
    // pairs describe valid allocations. Their one-past-end pointers are used
    // only for bounds comparisons.
    let input_end = unsafe { input.add(input_len) };
    let output_end = unsafe { output.add(capacity) };
    let result: usize;

    // SAFETY: All source reads are preceded by input, key, or value length
    // checks. Each output write is preceded by a capacity check. The template
    // uses no stack or calls. All modified GPRs are declared below. RAX
    // carries the final output cursor or the fallback sentinel; Rust turns a
    // successful cursor into the initialized byte count.
    unsafe {
        asm!(
            include_str!("interpolate.asm"),
            inout("rsi") input => _,
            in("rdx") input_end,
            in("r8") patterns,
            in("r9") pattern_count,
            inout("rdi") output => _,
            in("r10") output_end,
            out("rax") result,
            out("rcx") _,
            out("r11") _,
            out("r12") _,
            out("r13") _,
            out("r14") _,
            out("r15") _,
            options(nostack),
        );
    }

    if result == usize::MAX {
        result
    } else {
        result - output as usize
    }
}
