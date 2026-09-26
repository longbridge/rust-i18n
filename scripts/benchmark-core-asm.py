#!/usr/bin/env python3
"""Compare the Rust interpolation path with the production ASM kernel.

Both variants export Git HEAD into target/ and overlay the working-tree `src/`
and `benches/` directories, so uncommitted kernel changes are measured. The ASM
variant keeps production routing (`replace_patterns_cow` calls the kernel); the
Rust variant rewrites only that entry to call the Rust parser directly.
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
CORE_CASES = (
    "t",
    "t_with_locale",
    "t_with_args",
    "t_with_args (str)",
    "t_with_args (many)",
    "t_dynamic_key",
    "t_dynamic_locale",
    "t_with_dynamic_args",
)
DIRECT_CASES = (
    "replace_patterns_short",
    "replace_patterns_long_sparse",
    "replace_patterns_long_no_marker",
    "replace_patterns_long_malformed",
)
LONG_CASES = (
    "asm_effect_long_sparse",
    "asm_effect_long_no_marker",
    "asm_effect_long_dense",
    "asm_effect_unicode_long",
    "asm_effect_literal_percent",
    "asm_effect_unfinished",
    "asm_effect_nine_args",
    "asm_effect_large_value",
    "asm_effect_repeated_growth",
)
ALL_CASES = CORE_CASES + DIRECT_CASES + LONG_CASES
BENCHMARK_INPUTS = (
    "benches/bench.rs",
    "benches/asm_effect.rs",
    "benches/fixtures/asm/en.yml",
    "benches/fixtures/asm/fr.yml",
)


def run(command, *, cwd=ROOT, env=None, stdout=None):
    print("+", " ".join(map(str, command)), file=sys.stderr, flush=True)
    return subprocess.run(command, cwd=cwd, env=env, stdout=stdout, check=True)


def capture(command):
    return subprocess.check_output(command, cwd=ROOT, text=True).strip()


OVERLAY_DIRS = ("src", "benches")
COW_ENTRY = re.compile(
    r"(pub fn replace_patterns_cow\([^)]*\) -> String \{\n)(.*?)(\n\}\n)", re.DOTALL
)
ASM_CALL = "asm::interpolate::replace_patterns_cow(input, patterns, values)"
RUST_CALL = "replace_patterns_impl(input, patterns, values)"


def arch():
    machine = platform.machine().lower()
    if machine in ("x86_64", "amd64"):
        return "x86_64"
    if machine in ("aarch64", "arm64"):
        return "aarch64"
    raise RuntimeError(f"Native x86_64 or AArch64 required; found {machine}")


def environment(target):
    result = os.environ.copy()
    result["CARGO_TARGET_DIR"] = str(target)
    result["CRITERION_HOME"] = str(target / "criterion")
    return result


def cpu_name():
    if sys.platform == "linux":
        path = Path("/proc/cpuinfo")
        if path.exists():
            for line in path.read_text().splitlines():
                if line.startswith("model name"):
                    return line.split(":", 1)[1].strip()
    if sys.platform == "darwin":
        for key in ("machdep.cpu.brand_string", "hw.model"):
            process = subprocess.run(["sysctl", "-n", key], text=True, capture_output=True)
            if process.returncode == 0 and process.stdout.strip():
                return process.stdout.strip()
    return platform.processor() or "unknown"


def write_if_changed(path, contents):
    data = contents.encode() if isinstance(contents, str) else contents
    if not path.exists() or path.read_bytes() != data:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)


def copy_if_changed(source, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    if not destination.exists() or source.read_bytes() != destination.read_bytes():
        shutil.copy2(source, destination)


def export_head(source, archive_path, revision):
    with archive_path.open("wb") as archive:
        run(["git", "archive", "--format=tar", revision], stdout=archive)
    source.mkdir()
    with tarfile.open(archive_path) as archive:
        for member in archive.getmembers():
            if not (source / member.name).resolve().is_relative_to(source.resolve()):
                raise ValueError(f"Unsafe archive path: {member.name}")
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


def cow_route(lib, call):
    """Return src/lib.rs with the public Cow entry's body set to `call`."""
    matches = COW_ENTRY.findall(lib)
    if len(matches) != 1:
        raise RuntimeError("Expected one `pub fn replace_patterns_cow` in src/lib.rs")
    if matches[0][1].strip() != ASM_CALL:
        raise RuntimeError(f"Production Cow entry must call `{ASM_CALL}`; found {matches[0][1].strip()!r}")
    return COW_ENTRY.sub(lambda match: match.group(1) + "    " + call + match.group(3), lib, count=1)


def prepare(source, variant, architecture, overlay):
    # Rewrite only changed files on --resume, preserving incremental builds.
    for relative in overlay:
        if relative == "src/lib.rs":
            continue
        path = ROOT / relative
        destination = source / relative
        if path.is_file():
            write_if_changed(destination, path.read_bytes())
        elif destination.exists():
            destination.unlink()
    lib = (ROOT / "src/lib.rs").read_text()
    write_if_changed(source / "src/lib.rs", cow_route(lib, ASM_CALL if variant == "asm" else RUST_CALL))

    for required in (
        # Naked-function kernels have no per-architecture Rust shim.
        *[path for path in (f"src/asm/{architecture}/interpolate.rs",) if (ROOT / path).is_file()],
        f"src/asm/{architecture}/interpolate.asm",
        "src/asm/interpolation_tests.rs",
        "benches/support/asm_effect.rs",
        "benches/fixtures/asm/en.yml",
        "benches/fixtures/asm/fr.yml",
    ):
        if not (source / required).is_file():
            raise RuntimeError(f"Missing {required} in {source}")
    copy_if_changed(source / "benches/support/asm_effect.rs", source / "benches/asm_effect.rs")
    manifest = source / "Cargo.toml"
    contents = manifest.read_text()
    stanza = '\n[[bench]]\nname = "asm_effect"\nharness = false\n'
    if 'name = "asm_effect"' not in contents:
        write_if_changed(manifest, contents + stanza)


def lock_hash(source):
    return hashlib.sha256((source / "Cargo.lock").read_bytes()).hexdigest()


def source_hash(source, paths):
    digest = hashlib.sha256()
    for relative in paths:
        contents = (source / relative).read_bytes()
        digest.update(relative.encode())
        digest.update(b"\0")
        digest.update(len(contents).to_bytes(8, "big"))
        digest.update(contents)
    return digest.hexdigest()


def binary(target, name):
    candidates = [
        path for path in (target / "release/deps").glob(name + "-*")
        if path.is_file() and os.access(path, os.X_OK)
    ]
    if len(candidates) != 1:
        raise RuntimeError(f"Expected one {name} executable in {target}, found {candidates}")
    return candidates[0]


def build(source, target):
    env = environment(target)
    run(["cargo", "test", "--offline", "--locked", "--release", "--lib", "--no-run"], cwd=source, env=env)
    benchmarks = {}
    for name in ("bench", "asm_effect"):
        run(["cargo", "bench", "--offline", "--locked", "--bench", name, "--no-run"], cwd=source, env=env)
        benchmarks[name] = binary(target, name)
    return benchmarks


def test(source, target):
    run(["cargo", "test", "--offline", "--locked", "--release", "--lib"], cwd=source, env=environment(target))


def measure(source, target, executables, cases, pass_number, quick):
    values = {}
    # On --resume, a failed prior pass must not contribute stale named results.
    for case in cases:
        saved = target / "criterion" / case / f"pass_{pass_number}"
        if saved.exists():
            shutil.rmtree(saved)
    for bench_name, bench_cases in (
        ("bench", [case for case in cases if case in CORE_CASES + DIRECT_CASES]),
        ("asm_effect", [case for case in cases if case in LONG_CASES]),
    ):
        if not bench_cases:
            continue
        expression = "^(?:" + "|".join(re.escape(case) for case in bench_cases) + ")$"
        arguments = [
            str(executables[bench_name]), "--bench", expression,
            "--sample-size", "10" if quick else "30",
            "--warm-up-time", "0.1" if quick else "1",
            "--measurement-time", "0.2" if quick else "2",
            "--save-baseline", f"pass_{pass_number}",
        ]
        run(arguments, cwd=source, env=environment(target))
    for case in cases:
        estimate = target / "criterion" / case / f"pass_{pass_number}" / "estimates.json"
        if not estimate.is_file():
            raise RuntimeError(f"Missing Criterion result: {estimate}")
        values[case] = json.loads(estimate.read_text())["mean"]["point_estimate"]
    return values


def size_probe(source, target):
    run(
        ["cargo", "rustc", "--offline", "--locked", "--release", "--example",
         "size_probe", "--", "-C", "strip=symbols"],
        cwd=source,
        env=environment(target),
    )
    path = target / "release/examples" / ("size_probe.exe" if os.name == "nt" else "size_probe")
    return path.stat().st_size, subprocess.check_output([str(path), "en", "Jason"], cwd=source)


def report(revision, architecture, cases, measurements, digest, overlays, dirty, quick, sizes=None):
    def pair(case, variant):
        return [result[case] for result in measurements[variant]]

    def average(values):
        return sum(values) / len(values)

    lines = [
        "# Controlled core interpolation comparison",
        "",
        f"Git revision: `{revision}`"
        + (" with uncommitted `src/` or `benches/` changes" if dirty else "")
        + ". Both variants use the same source, benchmarks, scalar percent scanner, "
        "and Cargo.lock. Rust calls the Rust parser from `replace_patterns_cow`; "
        "ASM keeps the production route through the kernel.",
        f"Host: {cpu_name()} ({architecture}); {platform.platform()}.",
        f"Compiler: `{capture(['rustc', '--version'])}`; Cargo.lock SHA-256: `{digest}`.",
        f"Kernel source SHA-256: `{overlays['asm']}`; "
        f"benchmark inputs SHA-256: `{overlays['benchmarks']}`.",
        "The hashes cover file names and bytes of the kernel adapter, wrapper, "
        "instructions, tests, benchmark source, and locale fixtures.",
        "Criterion: " + ("quick 10 samples, 0.1 s warmup, 0.2 s measurement" if quick else
                        "30 samples, 1 s warmup, 2 s measurement") + "; two passes per variant, Rust–ASM–ASM–Rust.",
        "",
    ]
    for title, group in (
        ("Ordinary translation calls", CORE_CASES),
        ("Direct replacement controls", DIRECT_CASES),
        ("Additional translation and fallback calls", LONG_CASES),
    ):
        shown = [case for case in group if case in cases]
        if not shown:
            continue
        lines.extend([
            f"## {title}", "",
            "| Case | Before: Rust (ns) | After: ASM (ns) | Speedup |",
            "| --- | ---: | ---: | ---: |",
        ])
        for case in shown:
            before = average(pair(case, "rust"))
            after = average(pair(case, "asm"))
            lines.append(f"| `{case}` | {before:,.2f} | {after:,.2f} | {before / after:.2f}× |")
        lines.append("")
    lines.extend([
        "<details><summary>Per-pass means and ranges</summary>", "",
        "Each pass mean is Criterion's estimate in ns/iteration. The range shows "
        "variation between passes, not a confidence interval.", "",
        "| Case | Rust pass 1 | Rust pass 2 | Rust range | ASM pass 1 | ASM pass 2 | ASM range |",
        "| --- | ---: | ---: | ---: | ---: | ---: | ---: |",
    ])
    for case in cases:
        before = pair(case, "rust")
        after = pair(case, "asm")
        lines.append(
            f"| `{case}` | {before[0]:,.2f} | {before[1]:,.2f} | "
            f"{min(before):,.2f}–{max(before):,.2f} | {after[0]:,.2f} | "
            f"{after[1]:,.2f} | {min(after):,.2f}–{max(after):,.2f} |"
        )
    lines.extend(["", "</details>", ""])
    if sizes:
        rust_bytes = sizes["rust"]
        asm_bytes = sizes["asm"]
        lines.extend([
            "## Representative stripped executable size", "",
            "| Probe | Before: Rust (bytes) | After: ASM (bytes) | Size change |",
            "| --- | ---: | ---: | ---: |",
            f"| `size_probe` | {rust_bytes:,} | {asm_bytes:,} | "
            f"{(asm_bytes / rust_bytes - 1) * 100:+.2f}% |", "",
            "Both variants built the same probe with `-C strip=symbols`; output "
            "for `en Jason` matched. This is one representative executable.", "",
        ])
    lines.append(
        "Both release libraries and benchmark binaries were built before timing, "
        "and both release library test suites passed. Direct replacement controls "
        "call `replace_patterns`, which does not use the kernel."
    )
    lines.append("")
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path("target/core-asm-comparison.md"))
    parser.add_argument("--filter", help="Regex selecting case names from the fixed benchmark set")
    parser.add_argument("--quick", "--debug", dest="quick", action="store_true",
                        help="Short Criterion samples for checking the script, not for results")
    parser.add_argument("--resume", type=Path, help="Reuse an existing target/core-asm-comparison run directory")
    parser.add_argument("--include-size", action="store_true")
    parser.add_argument("--append-github-summary", action="store_true")
    args = parser.parse_args()
    if args.append_github_summary and not os.environ.get("GITHUB_STEP_SUMMARY"):
        parser.error("--append-github-summary requires GITHUB_STEP_SUMMARY")
    pattern = re.compile(args.filter) if args.filter else None
    cases = tuple(case for case in ALL_CASES if pattern is None or pattern.search(case))
    if not cases:
        parser.error("--filter matched no benchmark cases")
    architecture = arch()
    revision = capture(["git", "rev-parse", "HEAD"])
    overlay = worktree_files()
    dirty = worktree_dirty()
    run(["cargo", "fetch", "--locked"], cwd=ROOT)
    base = ROOT / "target/core-asm-comparison"
    resume_path = None
    if args.resume:
        resume_path = args.resume if args.resume.is_absolute() else ROOT / args.resume
    run_dir = resume_path.resolve() if resume_path else base / uuid.uuid4().hex
    if args.resume:
        if not run_dir.is_dir() or not run_dir.is_relative_to(base.resolve()):
            parser.error("--resume must name an existing run under target/core-asm-comparison")
        previous_revision = (run_dir / "revision.txt").read_text().strip()
        if previous_revision != revision:
            parser.error(f"--resume revision {previous_revision} differs from HEAD {revision}")
    else:
        run_dir.mkdir(parents=True)
        (run_dir / "revision.txt").write_text(revision + "\n")
    sources = {}
    targets = {}
    for variant in ("rust", "asm"):
        source = run_dir / variant / "source"
        if not args.resume:
            source.parent.mkdir()
            export_head(source, source.parent / "head.tar", revision)
        prepare(source, variant, architecture, overlay)
        sources[variant] = source
        targets[variant] = run_dir / variant / "target"
    digests = {variant: lock_hash(source) for variant, source in sources.items()}
    if len(set(digests.values())) != 1:
        raise RuntimeError(f"Variant lockfiles differ: {digests}")
    benchmark_hashes = {variant: source_hash(source, BENCHMARK_INPUTS) for variant, source in sources.items()}
    if len(set(benchmark_hashes.values())) != 1:
        raise RuntimeError(f"Variant benchmark inputs differ: {benchmark_hashes}")
    candidate_files = tuple(
        path for path in (
            "src/asm/interpolate.rs",
            "src/asm/interpolation_tests.rs",
            f"src/asm/{architecture}/interpolate.rs",
            f"src/asm/{architecture}/interpolate.asm",
        )
        if (sources["asm"] / path).is_file()
    )
    overlays = {
        "asm": source_hash(sources["asm"], candidate_files),
        "benchmarks": benchmark_hashes["rust"],
    }
    binaries = {variant: build(sources[variant], targets[variant]) for variant in sources}
    for variant in sources:
        test(sources[variant], targets[variant])
    measurements = {"rust": [], "asm": []}
    for variant in ("rust", "asm", "asm", "rust"):
        pass_number = len(measurements[variant]) + 1
        measurements[variant].append(
            measure(sources[variant], targets[variant], binaries[variant], cases, pass_number, args.quick)
        )
    sizes = None
    if args.include_size:
        sizes = {}
        expected_output = None
        for variant in ("rust", "asm"):
            sizes[variant], output = size_probe(sources[variant], targets[variant])
            if expected_output is not None and expected_output != output:
                raise RuntimeError("size_probe output differs between Rust and ASM variants")
            expected_output = output
    output = args.output if args.output.is_absolute() else ROOT / args.output
    output.parent.mkdir(parents=True, exist_ok=True)
    markdown = report(
        revision, architecture, cases, measurements,
        next(iter(digests.values())), overlays, dirty, args.quick, sizes,
    )
    output.write_text(markdown)
    print(f"Wrote {output}")
    if args.append_github_summary:
        with Path(os.environ["GITHUB_STEP_SUMMARY"]).open("a") as summary:
            summary.write(markdown)


if __name__ == "__main__":
    main()
