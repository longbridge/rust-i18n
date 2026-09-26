#!/usr/bin/env python3
"""Compare placeholder scanners in identical exports of the current Git HEAD.

Requires a native x86_64 or AArch64 host. All task files stay under target/.
"""

import argparse
import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
import tarfile
import uuid
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
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


def scanner_source(variant, production, arch, scanner_dir):
    preamble, marker, production_tests = production.partition("#[cfg(test)]")
    if not marker:
        raise RuntimeError("Production scanner tests marker not found")
    if variant == "current":
        replacement = preamble
    elif variant == "scalar":
        replacement = """/// Find the first percent byte using the scalar Rust iterator.
pub(crate) fn find_percent(bytes: &[u8]) -> Option<usize> {
    bytes.iter().position(|&byte| byte == b'%')
}

"""
    elif variant in ("intrinsics", "simd_asm"):
        source_name = "scan_intrinsics.rs" if variant == "intrinsics" else "scan_simd.rs"
        candidate = scanner_dir / arch / source_name
        if not candidate.is_file():
            raise RuntimeError(f"Missing {candidate}")
        replacement = """mod candidate;

/// Use the common scalar path for short translations.
pub(crate) fn find_percent(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < 256 {
        bytes.iter().position(|&byte| byte == b'%')
    } else {
        candidate::find_percent(bytes)
    }
}

"""
    elif variant == "memchr":
        replacement = """/// Use the common scalar path for short translations.
pub(crate) fn find_percent(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < 256 {
        bytes.iter().position(|&byte| byte == b'%')
    } else {
        memchr::memchr(b'%', bytes)
    }
}

"""
    else:
        raise ValueError(variant)
    scanner_tests = (scanner_dir / "scan_tests.rs").read_text()
    return replacement + marker + production_tests + "\n" + scanner_tests + "\n"


def prepare_variant(source, variant, arch):
    scanner_dir = ROOT / "src/asm"
    shutil.copy2(ROOT / "benches/support/asm_effect.rs", source / "benches/asm_effect.rs")
    shutil.copytree(
        ROOT / "benches/fixtures/asm", source / "benches/fixtures/asm", dirs_exist_ok=True
    )

    manifest = source / "Cargo.toml"
    contents = manifest.read_text()
    dependency_header = "[dependencies]\n"
    if contents.count(dependency_header) != 1:
        raise RuntimeError("Expected exactly one [dependencies] header")
    if "memchr =" in contents:
        raise RuntimeError("Export already has a direct memchr dependency")
    contents = contents.replace(dependency_header, dependency_header + 'memchr = "=2.7.5"\n')
    contents += '\n[[bench]]\nname = "asm_effect"\nharness = false\n'
    manifest.write_text(contents)
    shutil.copy2(ROOT / "Cargo.lock", source / "Cargo.lock")

    module = source / "src/asm/mod.rs"
    production = module.read_text()
    module.write_text(scanner_source(variant, production, arch, scanner_dir))
    if variant in ("intrinsics", "simd_asm"):
        candidate_dir = scanner_dir / arch
        candidate_name = "scan_intrinsics.rs" if variant == "intrinsics" else "scan_simd.rs"
        shutil.copy2(candidate_dir / candidate_name, source / "src/asm/candidate.rs")
        if variant == "simd_asm":
            for sibling in candidate_dir.glob("*.asm"):
                shutil.copy2(sibling, source / "src/asm" / sibling.name)


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


def measure(source, target, bench_binary, pass_number):
    run(
        [str(bench_binary), "--bench", "--sample-size", "30", "--warm-up-time", "1",
         "--measurement-time", "2", "--save-baseline", f"pass_{pass_number}"],
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


def report(revision, arch, selected, measurements, lock_hash, sizes=None):
    lines = [
        "# Controlled placeholder scanner comparison",
        "",
        f"Git revision: `{revision}`. All variants export this same revision and "
        "use the same benchmark source and dependency lockfile.",
        f"Host: {cpu_name()} ({arch}); {platform.platform()}.",
        f"Compiler: `{capture(['rustc', '--version'])}`; "
        "Criterion: 30 samples, 1 s warmup, 2 s measurement, two passes per variant.",
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
                f"## Production ASM versus {reference}",
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
    parser.add_argument("--append-github-summary", action="store_true")
    args = parser.parse_args()
    selected = tuple(dict.fromkeys(args.variants))
    if args.append_github_summary and not os.environ.get("GITHUB_STEP_SUMMARY"):
        parser.error("--append-github-summary requires GITHUB_STEP_SUMMARY")
    arch = architecture()
    for required in (
        ROOT / "src/asm/scan_tests.rs",
        ROOT / "benches/support/asm_effect.rs",
        ROOT / "benches/fixtures/asm/en.yml",
        ROOT / "benches/fixtures/asm/fr.yml",
    ):
        if not required.is_file():
            raise RuntimeError(f"Missing {required}")
    revision = capture(["git", "rev-parse", "HEAD"])
    run(["cargo", "fetch", "--locked"], cwd=ROOT)
    run_dir = ROOT / "target/asm-comparison" / uuid.uuid4().hex
    run_dir.mkdir(parents=True)
    sources = {}
    targets = {}
    for variant in selected:
        source = run_dir / variant / "source"
        source.parent.mkdir()
        export_head(source, source.parent / "head.tar", revision)
        prepare_variant(source, variant, arch)
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
            measure(sources[variant], targets[variant], binaries[variant], pass_number)
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
    markdown = report(revision, arch, selected, measurements, next(iter(hashes.values())), sizes)
    output.write_text(markdown)
    print(f"Wrote {output}")
    if args.append_github_summary:
        with Path(os.environ["GITHUB_STEP_SUMMARY"]).open("a") as summary:
            summary.write(markdown)


if __name__ == "__main__":
    main()
