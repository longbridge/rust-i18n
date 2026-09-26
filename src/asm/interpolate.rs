//! Safe adapter for the experimental assembly interpolation kernel.
//!
//! Call `replace_patterns_cow` from
//! the crate's existing public Cow entry point. The original Rust parser is
//! retained as the fallback for unsupported or rejected inputs.

use std::borrow::Cow;

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
use std::mem::MaybeUninit;

/// Layout consumed by both architecture kernels. Only the initialized prefix
/// of a stack array is passed to assembly.
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
#[repr(C)]
#[derive(Clone, Copy)]
#[allow(dead_code)] // Fields are read by the architecture assembly kernels.
pub(super) struct Pattern {
    key_ptr: *const u8,
    key_len: usize,
    value_ptr: *const u8,
    value_len: usize,
}

#[cfg(target_arch = "x86_64")]
#[path = "x86_64/interpolate.rs"]
mod arch;

#[cfg(target_arch = "aarch64")]
#[path = "aarch64/interpolate.rs"]
mod arch;

pub(crate) fn replace_patterns_cow(
    input: &str,
    patterns: &[&str],
    values: &[Cow<'_, str>],
) -> String {
    try_replace_patterns_cow(input, patterns, values)
        .unwrap_or_else(|| crate::replace_patterns_impl(input, patterns, values))
}

/// `Some` means assembly completed; `None` asks the unchanged Rust parser to
/// handle the input. Exposed within the crate for forced-path experiment tests.
pub(crate) fn try_replace_patterns_cow(
    input: &str,
    patterns: &[&str],
    values: &[Cow<'_, str>],
) -> Option<String> {
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    {
        // The original parser pairs entries with `zip`, so unpaired names or
        // values must be ignored. Larger paired sets use the Rust path.
        let count = patterns.len().min(values.len());
        if count > 8 {
            return None;
        }

        let requested_capacity = input.len().checked_add(128)?;
        let mut descriptors: [MaybeUninit<Pattern>; 8] = [MaybeUninit::uninit(); 8];
        for (slot, (&key, value)) in descriptors
            .iter_mut()
            .zip(patterns.iter().zip(values.iter()))
        {
            let value: &str = value.as_ref();
            slot.write(Pattern {
                key_ptr: key.as_ptr(),
                key_len: key.len(),
                value_ptr: value.as_ptr(),
                value_len: value.len(),
            });
        }

        // Match the Rust fast path's initial reservation. Its allocation is
        // separate from the input, descriptors, and every key/value slice.
        let mut output = Vec::with_capacity(requested_capacity);

        // SAFETY: `input` and every descriptor point to live initialized
        // bytes for this synchronous call. Exactly `count` descriptors were
        // initialized above. `output` has capacity writable bytes and does
        // not alias any source. The kernel contract bounds every read/write,
        // never retains a pointer, and returns either the initialized output
        // length (at most capacity) or usize::MAX after partial writes.
        let written = unsafe {
            arch::interpolate(
                input.as_ptr(),
                input.len(),
                descriptors.as_ptr().cast::<Pattern>(),
                count,
                output.as_mut_ptr(),
                output.capacity(),
            )
        };
        if written == usize::MAX || written > output.capacity() {
            return None;
        }

        // SAFETY: On success, the kernel has initialized exactly `written`
        // bytes as a concatenation of whole substrings of the UTF-8 input,
        // whole UTF-8 replacement values, and complete ASCII marker bytes.
        // Thus the initialized prefix is valid UTF-8; rejected partial output
        // is never published through a String.
        unsafe {
            output.set_len(written);
            Some(String::from_utf8_unchecked(output))
        }
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        let _ = (input, patterns, values);
        None
    }
}
