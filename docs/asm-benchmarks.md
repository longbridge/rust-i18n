# Isolating the benefit of assembly scanning

The before/after benchmark for this PR includes changes to locale lookup,
placeholder parsing, and macro-generated code. It cannot attribute the full
translation speedup to assembly. This experiment holds those Rust changes at
the **same source revision** and switches only the routine that finds the next
`%` byte in a translation string.

Run `python3 scripts/benchmark-asm.py` on the architecture being evaluated.
The script exports Git HEAD for each variant under `target/asm-comparison/`,
copies the working-tree `src/` and `benches/` over it, and changes two things:

1. `replace_patterns_cow` calls the Rust parser. Production sends it to the
   interpolation kernel, which never calls the scanner.
2. `find_percent` in `src/asm/mod.rs` is swapped for the variant's scanner.

The active checkout is never modified. Each variant uses the same translation
fixtures, benchmark source, lockfile, compiler, and build profile. The scanner
candidates are naked functions declared in `src/asm/scan.rs`, each with a
whole `.asm` file from `src/asm/<arch>/` as its body. The intrinsics variant is
spliced in from `benches/support/scan_intrinsics.rs`.

## Scanner variants

The CLI key `current` denotes the retained legacy ASM scanner for compatibility.
The production scanner uses scalar Rust.

| Variant | Scan used for slices of at least 256 bytes |
| --- | --- |
| Rust scalar | Byte iterator search in ordinary Rust. |
| Legacy ASM | The retained architecture-specific scanner (`repne scasb` on x86-64; 16-byte NEON blocks on AArch64). |
| Matched SIMD intrinsics | SSE2 or runtime-detected AVX2 on x86-64; NEON on AArch64. |
| Matched SIMD ASM | The same block widths, dispatch, match test, and scalar tail as the intrinsics variant, written as complete naked `.asm` functions. |
| `memchr` | The `memchr` crate's byte search; the dependency is present in every overlay for a matched build. |

All five variants keep the same ordinary Rust scan for slices shorter than
256 bytes. The matched SIMD pair lets us compare the language used to express
the *same algorithm*. The scalar and `memchr` variants provide practical
alternatives; comparing either with legacy ASM does not by itself isolate an
"ASM language" effect. On AArch64, legacy ASM and matched SIMD ASM call the
same NEON kernel. A naked function is always an out-of-line call, while LLVM
may inline the intrinsics, so the matched pair also measures that call.

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

* **Legacy ASM vs Rust scalar:** scalar time divided by legacy ASM time.
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
path with the kernel that production uses. Both variants export Git HEAD under
`target/core-asm-comparison/` with the working-tree `src/` and `benches/`
copied over it, so uncommitted kernel changes are measured. The ASM variant
keeps production routing. The Rust variant changes only the body of
`replace_patterns_cow` to call `replace_patterns_impl`. Both variants use the
same locale lookup, macro expansion, benchmark inputs, and scalar `%` scanner.
Both run the full library test suite before timing complete `t!` calls. Direct
`replace_patterns` calls are measured as a control; they do not use the kernel.

The whole-kernel comparison asks whether replacing the interpolation routine
helps the complete translation call. Its result includes dispatch and fallback
costs. Report the Rust and assembly times and their ratio for each case,
including short, long, Unicode, and malformed inputs. Keep the scanner results
separate when attributing gains to hand-written instructions. The report notes
when the measured source has uncommitted changes.

## Running the scripts

```sh
# Full runs, for results.
python3 scripts/benchmark-asm.py --include-size
python3 scripts/benchmark-core-asm.py --include-size

# Check that a script builds, tests, and reports; timings are not usable.
python3 scripts/benchmark-asm.py --quick --variants scalar simd_asm
python3 scripts/benchmark-core-asm.py --quick --filter '^t_with_args$'
```

Reports are written to `target/asm-comparison.md` and
`target/core-asm-comparison.md` unless `--output` is given. The core script
also accepts `--resume target/core-asm-comparison/<run>` to reuse a run's
builds. The `ASM contribution experiment` workflow runs both scripts on
`ubuntu-latest` and `macos-15` and uploads the reports.
