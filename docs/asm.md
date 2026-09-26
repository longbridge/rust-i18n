# Assembly implementation layout

`src/asm/` contains only `.asm` instruction source files, grouped by CPU
architecture and named after their operation:

```text
src/asm/
  x86_64/
    interpolate.asm
    find_percent.asm
    find_percent_sse2.asm
    find_percent_avx2.asm
  aarch64/
    interpolate.asm
    find_percent.asm
    find_percent_neon.asm
```

`interpolate.asm` contains the interpolation algorithm: scanning literal text,
parsing complete markers, matching argument keys, and copying output. Rust
interfaces and register constraints live separately in `src/interpolation/`.
The `.asm` files are included by `asm!(include_str!(...))` and assembled by LLVM;
they do not require a separate NASM toolchain. The named Rust wrapper supplies
the callable interface, explicit clobbers, and the unsafe contract.

`src/interpolation/interpolate.rs` owns descriptor preparation, output storage,
and fallback to the existing Rust parser. Kernels require 64-bit pointers on
x86_64 or AArch64; other targets use Rust. Bounded vector loads, complete UTF-8
replacement strings, checked output capacity, and first-match argument order
must be preserved. Failure never publishes partially initialized output.

The portable `find_percent` fallback uses ordinary Rust. The old REPNE/NEON
scanner wrappers remain available for controlled benchmark comparisons; they
are not selected by the production scanner. The whole-interpolation candidate
is selected by the benchmark driver while native performance is validated.

Tests live in `src/interpolation/`, benchmark inputs in `benches/`, and drivers
in `scripts/`. See [benchmark methodology](asm-benchmarks.md). Cross-compilation
checks assembly syntax and target compatibility; runtime correctness and speed
must also be measured on native hardware before enabling a candidate.
