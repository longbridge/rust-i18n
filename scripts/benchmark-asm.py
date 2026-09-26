#!/usr/bin/env python3
"""Compare placeholder scanners in identical exports of the current Git HEAD.

Each variant overlays the working-tree `src/` and `benches/` directories, routes
`replace_patterns_cow` through the Rust parser (production uses the interpolation
kernel, which never calls the scanner), and replaces only `find_percent` in
`src/asm/mod.rs`. Requires a
native x86_64 or AArch64 host. All task files stay under target/.
"""

import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import uuid
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OVERLAY_DIRS = ("src", "benches")
PRODUCTION_SCANNER = re.compile(
    r"^pub\(crate\) fn find_percent\(bytes: &\[u8\]\) -> Option<usize> \{\n.*?^\}\n",
    re.DOTALL | re.MULTILINE,
)
COW_ENTRY = re.compile(
    r"(pub fn replace_patterns_cow\([^)]*\) -> String \{\n)(.*?)(\n\}\n)", re.DOTALL
)
ASM_CALL = "asm::interpolate::replace_patterns_cow(input, patterns, values)"
RUST_CALL = "replace_patterns_impl(input, patterns, values)"
# Long-slice scan expression for each variant. The asm variants call naked
# `.asm` kernels declared in src/asm/scan.rs; `intrinsics` is spliced in from
# benches/support/scan_intrinsics.rs as src/asm/scan_intrinsics.rs.
LONG_SCANS = {
    "current": "scan::find_percent_legacy(bytes)",
    "simd_asm": "scan::find_percent_simd(bytes)",
    "intrinsics": "scan_intrinsics::find_percent(bytes)",
    "memchr": "memchr::memchr(b'%', bytes)",
}
SCAN_MODULE = re.compile(r"#\[cfg\(all\(\s*test,(.*?)\)\)\]\nmod scan;\n", re.DOTALL)
VARIANTS = ("scalar", "current", "intrinsics", "simd_asm", "memchr")
CASES = (
    "asm_effect_plain",
    "asm_effect_explicit_locale",
    "asm_effect_short_args",
    "asm_effect_dynamic_args",
    "asm_effect_long_sparse",
    "asm_effect_long_no_marker",
    "asm_effect_long_dense",
    "asm_effect_unicode_long",
)


def run(command, *, cwd=ROOT, env=None, stdout=None):
    print("+", " ".join(map(str, command)), file=sys.stderr, flush=True)
    return subprocess.run(command, cwd=cwd, env=env, stdout=stdout, check=True)


def capture(command):
    return subprocess.check_output(command, cwd=ROOT, text=True).strip()


def architecture():
    machine = platform.machine().lower()
    if machine in ("aarch64", "arm64"):
        return "aarch64"
    if machine in ("x86_64", "amd64"):
        return "x86_64"
    raise RuntimeError(f"Native x86_64 or AArch64 required; found {machine}")


def environment(target):
    result = os.environ.copy()
    result["CARGO_TARGET_DIR"] = str(target)
    result["CRITERION_HOME"] = str(target / "criterion")
    return result


def cpu_name():
    if sys.platform == "linux":
        cpuinfo = Path("/proc/cpuinfo")
        if cpuinfo.exists():
            for line in cpuinfo.read_text().splitlines():
                if line.startswith("model name"):
                    return line.split(":", 1)[1].strip()
    if sys.platform == "darwin":
        for key in ("machdep.cpu.brand_string", "hw.model"):
            process = subprocess.run(["sysctl", "-n", key], capture_output=True, text=True)
            if process.returncode == 0 and process.stdout.strip():
                return process.stdout.strip()
    return platform.processor() or "unknown"


def export_head(source, archive_path, revision):
    with archive_path.open("wb") as archive:
        run(["git", "archive", "--format=tar", revision], stdout=archive)
    source.mkdir()
    with tarfile.open(archive_path) as archive:
        for member in archive.getmembers():
            if not (source / member.name).resolve().is_relative_to(source.resolve()):
                raise ValueError(f"Unsafe Git archive member: {member.name}")
        if sys.version_info >= (3, 12):
            archive.extractall(source, filter="data")
        else:
            archive.extractall(source)
    archive_path.unlink()


def worktree_files():
    """Tracked and untracked, non-ignored paths under OVERLAY_DIRS."""
    output = subprocess.check_output(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard", "--", *OVERLAY_DIRS],
        cwd=ROOT,
    )
    return sorted({path.decode() for path in output.split(b"\0") if path})


def worktree_dirty():
    return bool(capture(["git", "status", "--porcelain", "--", *OVERLAY_DIRS]))


def rust_cow_route(lib):
    """Return src/lib.rs with the public Cow entry calling the Rust parser."""
    matches = COW_ENTRY.findall(lib)
    if len(matches) != 1 or matches[0][1].strip() != ASM_CALL:
        raise RuntimeError(f"Expected `pub fn replace_patterns_cow` in src/lib.rs to call `{ASM_CALL}`")
    return COW_ENTRY.sub(lambda match: match.group(1) + "    " + RUST_CALL + match.group(3), lib, count=1)


def scanner_source(variant, production):
    """Return src/asm/mod.rs with the production scanner replaced."""
    if len(PRODUCTION_SCANNER.findall(production)) != 1:
        raise RuntimeError("Expected one production find_percent in src/asm/mod.rs")
    if variant == "scalar":
        return production
    if len(SCAN_MODULE.findall(production)) != 1:
        raise RuntimeError("Expected one test-only `mod scan;` in src/asm/mod.rs")
    # Build the naked scanners outside tests too. Only one is used per variant.
    production = SCAN_MODULE.sub(
        lambda match: f"#[cfg(all({match.group(1).strip()}))]\n#[allow(dead_code)]\nmod scan;\n",
        production,
        count=1,
    )
    module = "mod scan_intrinsics;\n\n" if variant == "intrinsics" else ""
    # `current` is the former thresholded REPNE/NEON scanner, retained as the
    # legacy comparison; production now uses scalar Rust.
    replacement = module + f"""/// Use the common scalar path for short translations.
pub(crate) fn find_percent(bytes: &[u8]) -> Option<usize> {{
    if bytes.len() < 256 {{
        bytes.iter().position(|&byte| byte == b'%')
    }} else {{
        {LONG_SCANS[variant]}
    }}
}}
"""
    return PRODUCTION_SCANNER.sub(lambda _: replacement, production, count=1)


def prepare_variant(source, variant, overlay):
    for relative in overlay:
        path = ROOT / relative
        destination = source / relative
        if path.is_file():
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(path, destination)
        elif destination.exists():
            destination.unlink()
    if variant == "intrinsics":
        shutil.copy2(source / "benches/support/scan_intrinsics.rs", source / "src/asm/scan_intrinsics.rs")
    shutil.copy2(source / "benches/support/asm_effect.rs", source / "benches/asm_effect.rs")

    manifest = source / "Cargo.toml"
    contents = manifest.read_text()
    dependency_header = "[dependencies]\n"
    if contents.count(dependency_header) != 1:
        raise RuntimeError("Expected exactly one [dependencies] header")
    if "memchr =" in contents:
        raise RuntimeError("Export already has a direct memchr dependency")
    # Every variant gets the dependency so the builds stay matched.
    contents = contents.replace(dependency_header, dependency_header + 'memchr = "=2.7.5"\n')
    contents += '\n[[bench]]\nname = "asm_effect"\nharness = false\n'
    manifest.write_text(contents)
    shutil.copy2(ROOT / "Cargo.lock", source / "Cargo.lock")

    lib = source / "src/lib.rs"
    lib.write_text(rust_cow_route(lib.read_text()))
    module = source / "src/asm/mod.rs"
    module.write_text(scanner_source(variant, module.read_text()))


def lock_digest(source):
    return hashlib.sha256((source / "Cargo.lock").read_bytes()).hexdigest()


def executable(target, prefix):
    candidates = [
        path for path in (target / "release/deps").glob(prefix + "-*")
        if path.is_file() and os.access(path, os.X_OK)
    ]
    if len(candidates) != 1:
        raise RuntimeError(f"Expected one {prefix} executable in {target}, found {candidates}")
    return candidates[0]


def build_variant(source, target):
    env = environment(target)
    run(["cargo", "test", "--offline", "--locked", "--release", "--lib", "--no-run"], cwd=source, env=env)
    run(["cargo", "bench", "--offline", "--locked", "--bench", "asm_effect", "--no-run"], cwd=source, env=env)
    return executable(target, "asm_effect")


def test_variant(source, target):
    run(["cargo", "test", "--offline", "--locked", "--release", "--lib"], cwd=source, env=environment(target))


def measure(source, target, bench_binary, pass_number, quick):
    run(
        [str(bench_binary), "--bench",
         "--sample-size", "10" if quick else "30",
         "--warm-up-time", "0.1" if quick else "1",
         "--measurement-time", "0.2" if quick else "2",
         "--save-baseline", f"pass_{pass_number}"],
        cwd=source,
        env=environment(target),
    )
    results = {}
    for path in (target / "criterion").glob(f"*/pass_{pass_number}/estimates.json"):
        results[path.parent.parent.name] = json.loads(path.read_text())["mean"]["point_estimate"]
    missing = set(CASES) - results.keys()
    if missing:
        raise RuntimeError(f"Missing Criterion results in {target}: {sorted(missing)}")
    return results


def size_probe(source, target):
    run(
        ["cargo", "rustc", "--offline", "--locked", "--release", "--example",
         "size_probe", "--", "-C", "strip=symbols"],
        cwd=source,
        env=environment(target),
    )
    binary = target / "release/examples" / ("size_probe.exe" if os.name == "nt" else "size_probe")
    output = subprocess.check_output([str(binary), "en", "Jason"], cwd=source)
    return binary.stat().st_size, output


def mean(values):
    return sum(values) / len(values)


def report(revision, arch, selected, measurements, lock_hash, dirty, quick, sizes=None):
    lines = [
        "# Controlled placeholder scanner comparison",
        "",
        f"Git revision: `{revision}`"
        + (" with uncommitted `src/` or `benches/` changes" if dirty else "")
        + ". All variants use this same source, benchmark source, and dependency lockfile.",
        f"Host: {cpu_name()} ({arch}); {platform.platform()}.",
        f"Compiler: `{capture(['rustc', '--version'])}`; Criterion: "
        + ("quick 10 samples, 0.1 s warmup, 0.2 s measurement" if quick
           else "30 samples, 1 s warmup, 2 s measurement")
        + ", two passes per variant.",
        f"Cargo.lock SHA-256: `{lock_hash}`.",
        "Order: " + " → ".join(selected + tuple(reversed(selected))) + ".",
        "",
        "Each table cell is the arithmetic mean of two Criterion pass means, "
        "followed by the pass range in brackets (nanoseconds per iteration).",
        "",
        "| Full `t!` case | " + " | ".join(selected) + " |",
        "| --- | " + " | ".join("---:" for _ in selected) + " |",
    ]
    for case in CASES:
        cells = []
        for variant in selected:
            values = [entry[case] for entry in measurements[variant]]
            cells.append(f"{mean(values):,.2f} [{min(values):,.2f}–{max(values):,.2f}]")
        lines.append(f"| `{case}` | " + " | ".join(cells) + " |")

    if "current" in selected:
        for reference in ("scalar", "intrinsics", "memchr"):
            if reference not in selected:
                continue
            lines.extend([
                "",
                f"## Legacy scanner ASM versus {reference}",
                "",
                "| Full `t!` case | Before (ns) | After (ns) | Speedup |",
                "| --- | ---: | ---: | ---: |",
            ])
            for case in CASES:
                reference_ns = mean([entry[case] for entry in measurements[reference]])
                current_ns = mean([entry[case] for entry in measurements["current"]])
                ratio = reference_ns / current_ns
                lines.append(
                    f"| `{case}` | {reference_ns:,.2f} | {current_ns:,.2f} | {ratio:.2f}× |"
                )
    if "simd_asm" in selected and "intrinsics" in selected:
        lines.extend([
            "",
            "## Matched SIMD ASM versus matched intrinsics",
            "",
            "| Full `t!` case | Before (ns) | After (ns) | Speedup |",
            "| --- | ---: | ---: | ---: |",
        ])
        for case in CASES:
            intrinsic_ns = mean([entry[case] for entry in measurements["intrinsics"]])
            asm_ns = mean([entry[case] for entry in measurements["simd_asm"]])
            ratio = intrinsic_ns / asm_ns
            lines.append(
                f"| `{case}` | {intrinsic_ns:,.2f} | {asm_ns:,.2f} | {ratio:.2f}× |"
            )
    if sizes:
        lines.extend([
            "",
            "## Representative stripped executable size",
            "",
            "| Variant | Bytes | Change versus scalar |",
            "| --- | ---: | ---: |",
        ])
        reference = sizes.get("scalar")
        for variant in selected:
            size = sizes[variant]
            delta = f"{(size / reference - 1) * 100:+.2f}%" if reference else "—"
            lines.append(f"| {variant} | {size:,} | {delta} |")
        lines.extend([
            "",
            "These are sizes of one identical `size_probe` executable built with "
            "`-C strip=symbols`, not universal binary size estimates. All probe "
            "outputs for `en Jason` matched.",
        ])
    lines.extend([
        "",
        "The `current` variant is the legacy thresholded architecture scanner; "
        "the production default is scalar. Every variant routes `replace_patterns_cow` "
        "through the Rust parser so the scanner is on the measured path.",
        "",
        "All variants retain the same production runtime outside the scanner, "
        "with a common scalar path below 256 bytes except the all-scalar control. "
        "Benchmarks and release library tests passed in every variant before timing.",
        "",
    ])
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path("target/asm-comparison.md"))
    parser.add_argument("--variants", nargs="+", choices=VARIANTS, default=VARIANTS,
                        help="Subset for debugging; default compares all five")
    parser.add_argument("--include-size", action="store_true",
                        help="Build and compare stripped size_probe executables after timing")
    parser.add_argument("--quick", action="store_true",
                        help="Short Criterion samples for checking the script, not for results")
    parser.add_argument("--append-github-summary", action="store_true")
    args = parser.parse_args()
    selected = tuple(dict.fromkeys(args.variants))
    if args.append_github_summary and not os.environ.get("GITHUB_STEP_SUMMARY"):
        parser.error("--append-github-summary requires GITHUB_STEP_SUMMARY")
    arch = architecture()
    for required in (
        ROOT / "benches/support/asm_effect.rs",
        ROOT / "benches/fixtures/asm/en.yml",
        ROOT / "benches/fixtures/asm/fr.yml",
    ):
        if not required.is_file():
            raise RuntimeError(f"Missing {required}")
    revision = capture(["git", "rev-parse", "HEAD"])
    overlay = worktree_files()
    dirty = worktree_dirty()
    run(["cargo", "fetch", "--locked"], cwd=ROOT)
    run_dir = ROOT / "target/asm-comparison" / uuid.uuid4().hex
    run_dir.mkdir(parents=True)
    sources = {}
    targets = {}
    for variant in selected:
        source = run_dir / variant / "source"
        source.parent.mkdir()
        export_head(source, source.parent / "head.tar", revision)
        prepare_variant(source, variant, overlay)
        run(["cargo", "metadata", "--offline", "--format-version", "1"],
            cwd=source, env=environment(run_dir / variant / "target"), stdout=subprocess.DEVNULL)
        sources[variant] = source
        targets[variant] = run_dir / variant / "target"
    hashes = {variant: lock_digest(source) for variant, source in sources.items()}
    if len(set(hashes.values())) != 1:
        raise RuntimeError(f"Variant lockfiles differ: {hashes}")
    binaries = {variant: build_variant(sources[variant], targets[variant]) for variant in selected}
    for variant in selected:
        test_variant(sources[variant], targets[variant])

    measurements = {variant: [] for variant in selected}
    for variant in selected + tuple(reversed(selected)):
        pass_number = len(measurements[variant]) + 1
        measurements[variant].append(
            measure(sources[variant], targets[variant], binaries[variant], pass_number, args.quick)
        )
    sizes = None
    if args.include_size:
        sizes = {}
        expected_output = None
        for variant in selected:
            size, output = size_probe(sources[variant], targets[variant])
            if expected_output is not None and output != expected_output:
                raise RuntimeError(f"size_probe output differs for {variant}")
            expected_output = output
            sizes[variant] = size

    output = args.output if args.output.is_absolute() else ROOT / args.output
    output.parent.mkdir(parents=True, exist_ok=True)
    markdown = report(
        revision, arch, selected, measurements, next(iter(hashes.values())), dirty, args.quick, sizes
    )
    output.write_text(markdown)
    print(f"Wrote {output}")
    if args.append_github_summary:
        with Path(os.environ["GITHUB_STEP_SUMMARY"]).open("a") as summary:
            summary.write(markdown)


if __name__ == "__main__":
    main()
