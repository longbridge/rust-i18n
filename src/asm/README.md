# Placeholder byte scanning

`mod.rs` exposes the safe `find_percent(&[u8]) -> Option<usize>` wrapper. It uses
ordinary Rust for slices shorter than 256 bytes and on other architectures.
Longer slices use `x86_64/mod.rs` or `aarch64/mod.rs`; each Rust file supplies
the register constraints and safety argument for its `find_percent.asm` file.
Name new instruction files after the operation they implement, inside the
corresponding architecture directory. These are inline instruction fragments;
the named Rust function supplies the callable interface.

The `.asm` files are included by Rust's `asm!` macro and assembled by LLVM's
integrated assembler. The suffix does **not** mean NASM syntax or a separate
assembly/link step. Keep operands in the Rust `asm!` call synchronized with
registers read and written by the included instructions.

The wrapper must return the first matching byte offset without reading beyond
the slice. The x86-64 path bounds `repne scasb` with `RCX`; the AArch64 path
loads only complete 16-byte chunks and scans the remainder in Rust. Preserve
those bounds, the architecture fallback, and the documented `unsafe` contracts
when changing either implementation.

Run the correctness tests for supported targets, including boundary and
unaligned slices. Benchmark against the Rust scan on real hardware before
changing the dispatch threshold or claiming a performance improvement; a
cross-compile establishes build compatibility, not runtime speed.
