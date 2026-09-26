# Assembly implementation layout

Every assembly routine is a naked function whose whole body is one `.asm`
file:

```rust
#[unsafe(naked)]
unsafe extern "sysv64" fn find_percent_sse2(ptr: *const u8, len: usize) -> usize {
    core::arch::naked_asm!(include_str!("x86_64/find_percent_sse2.asm"))
}
```

Rust never lists operands, clobbers, or instruction text. Each `.asm` file is
a complete function: it follows the calling convention itself, saves any
callee-saved registers it uses, and returns. It starts with a comment that
documents its arguments, return value, and clobbers. Comments must not contain
braces, because `naked_asm!` treats them as template syntax. x86_64 kernels use
`extern "sysv64"` so Windows runs the same instructions. AArch64 kernels use
`extern "C"` (AAPCS64) and never touch `x18`, which Apple reserves. LLVM
assembles the files, so no separate assembler is needed.

```text
src/asm/
  mod.rs                  production scanner, module wiring
  interpolate.rs          safe adapter and naked kernel declarations
  scan.rs                 naked scanner declarations (benchmarks and tests)
  interpolation_tests.rs  kernel tests, including a differential test
  scan_tests.rs           scanner tests
  x86_64/
    interpolate.asm
    find_percent_repne.asm    legacy REPNE scanner
    find_percent_sse2.asm     16-byte blocks and a scalar tail
    find_percent_avx2.asm     32-byte blocks and a scalar tail
  aarch64/
    interpolate.asm
    find_percent_neon.asm     16-byte NEON blocks and a scalar tail
```

The `src/asm/` root holds Rust only. The `<arch>/` directories hold only
`.asm` files. The SIMD intrinsics scanner used for comparison is ordinary Rust,
so it lives in `benches/support/scan_intrinsics.rs`.

## Production use

`rust_i18n::replace_patterns_cow`, which `t!` calls when it has arguments,
goes through `asm::interpolate::replace_patterns_cow` on 64-bit x86_64 and
AArch64. The adapter builds up to sixteen key/value descriptors on the stack
and an output buffer of `input.len()` plus the larger of 128 bytes and the
longest value. The kernel scans literal text, parses `%{key}` markers, matches
keys in argument order, and copies the output.

The kernel returns the bytes written and the input bytes consumed. When the
output fills, it stops at literal text or at a marker's `%`; the adapter grows
the buffer (projecting the expansion seen so far) and calls it again on the
rest. A `%` that is not followed by `{`, and a final `%{` without a `}`, are
copied as literal text, as the Rust parser does. The kernel rejects only a `%`
inside a marker and a stray `%` whose run reaches a `{`; rejected input and
more than sixteen arguments go to the original Rust parser, so results never
differ. Partial output is never published. Other targets always use Rust.

`replace_patterns` (with `String` values) and the percent scanner
`find_percent` use plain Rust. The scanners declared in `scan.rs` are used only
in benchmarks and tests. On AArch64 the legacy and SIMD scanners are the same
NEON kernel.

## Tests

`cargo test` runs all of the assembly tests on a native x86_64 or AArch64
host:

- `asm::interpolation_tests` checks the kernel against the optimized and
  original Rust parsers. It covers fallback cases, slice bounds, every short
  marker sequence, and 20,000 seeded random inputs. The inputs mix Unicode,
  `%`, `%{`, `}`, known, unknown, duplicate and empty keys, 0 to 12 arguments,
  unpaired values, and values longer than 128 bytes.
- `asm::scan_tests` checks the production scanner and every `.asm` scanner
  (SSE2 and AVX2 separately) against a scalar search. It covers every match
  position and 32 alignments for lengths 0 to 511 around each block boundary,
  plus 4,096-byte and Unicode inputs.

CI runs these tests natively on Linux and Windows x86_64 and on macOS arm64,
in debug and release builds. On an x86_64 Linux host, the AArch64 kernels can
be tested under QEMU:

```sh
PATH="$(rustc --print sysroot)/lib/rustlib/x86_64-unknown-linux-gnu/bin:$PATH" \
CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=rust-lld \
CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_RUNNER=qemu-aarch64-static \
cargo test --target aarch64-unknown-linux-musl -p rust-i18n --lib
```

Emulation checks correctness only; `cargo build --release --target
aarch64-apple-darwin` checks that the kernels assemble for Apple targets.
Neither says anything about speed.

See [benchmark methodology](asm-benchmarks.md) for performance comparisons.
