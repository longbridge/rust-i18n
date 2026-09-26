# Isolating the benefit of assembly scanning

The before/after benchmark for this PR includes changes to locale lookup,
placeholder parsing, and macro-generated code. It cannot attribute the full
translation speedup to assembly. This experiment holds those Rust changes at
the **same source revision** and switches only the routine that finds the next
`%` byte in a translation string.

Run `python3 scripts/benchmark-asm.py` on the architecture being evaluated.
The script creates isolated source overlays and build outputs under `target/`;
it does not change production features, `cfg` choices, public APIs, or files
in the active checkout. Each variant uses the same translation fixtures,
benchmark source, lockfile, compiler, and build profile.
The assembly candidates are inline instruction fragments from `.asm` files,
included by narrow Rust `asm!` wrappers with explicit operands and safety
comments. They require no separate assembler or linker step.

## Scanner variants

| Variant | Scan used for slices of at least 256 bytes |
| --- | --- |
| Rust scalar | Byte iterator search in ordinary Rust. |
| Current ASM | The PR's architecture-specific scanner (`repne scasb` on x86-64; 16-byte NEON blocks on AArch64). |
| Matched SIMD intrinsics | SSE2 or runtime-detected AVX2 on x86-64; NEON on AArch64. |
| Matched SIMD ASM | The same block widths, dispatch, match test, and scalar tail as the intrinsics variant, with the vector instructions in `.asm` fragments. |
| `memchr` | The `memchr` crate's byte search; the dependency is present in every overlay for a matched build. |

All five variants keep the same ordinary Rust scan for slices shorter than
256 bytes. The matched SIMD pair lets us compare the language used to express
the *same algorithm*. The scalar and `memchr` variants provide practical
alternatives; comparing either with current ASM does not by itself isolate an
"ASM language" effect. On AArch64, current ASM may be the same algorithm as
matched SIMD ASM, so their results should be interpreted accordingly.

## Measurements and interpretation

`benches/support/asm_effect.rs` measures complete `t!` calls using
`benches/fixtures/asm/`. Plain translation and explicit-locale calls are
controls: they do not need the placeholder scanner. Short and dynamic
arguments represent common interpolation, while
long sparse, no-marker, dense-marker, and Unicode fixtures exercise different
scan patterns. The fixtures assert the expected translations before timing so
a lookup miss cannot masquerade as a fast result.

Build all candidate binaries before timing them. Measure each variant in
forward and reverse order, two passes per variant with 30 Criterion samples
per pass, on the same otherwise idle machine. Report the per-pass estimates
and their range as well as the aggregate; run-to-run variation is distinct
from Criterion's within-pass confidence intervals. Include the CPU, OS,
compiler, target architecture, source revision, and exact command in the
report.

For each measured case, calculate both:

* **Current ASM vs Rust scalar:** scalar time divided by current ASM time.
  This is the end-to-end effect of changing the scan implementation relative
  to a simple Rust scanner.
* **Matched SIMD ASM vs matched SIMD intrinsics:** intrinsics time divided by
  ASM time. This is the comparison needed to ask whether hand-written
  assembly adds anything beyond the SIMD algorithm itself.

A ratio above 1 means the named ASM variant was faster; a ratio below 1 means
it was slower. Report the `memchr` result alongside these ratios. Do not infer
a universal speedup from a synthetic long string or from a cross-compiled
binary. Only native runs establish runtime performance for that CPU.

## Whole interpolation kernel

Run `python3 scripts/benchmark-core-asm.py` to compare the Rust interpolation
path with the optional assembly kernel in `src/asm/interpolate.rs` and its
architecture-specific files. Both overlays use the same Rust locale lookup,
macro expansion, benchmark inputs, and scalar `%` scanner. The script changes
only the interpolation path in one overlay, then runs shared correctness tests
before timing complete `t!` calls. It also measures direct replacement as a
control.

The whole-kernel comparison asks whether replacing the interpolation routine
helps the complete translation call. Its result includes dispatch and fallback
costs. Report the Rust and assembly times and their ratio for each case,
including short, long, Unicode, and malformed inputs. Keep the scanner results
separate when attributing gains to hand-written instructions.
