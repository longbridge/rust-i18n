#!/usr/bin/env python3
"""Compare Criterion results from a Git revision and the current checkout.

The baseline is exported under target/, with the current benchmark source copied
into it. Neither Git worktrees nor changes to the current source are needed.
"""

import argparse
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
CORE_BENCHMARKS = (
    "t",
    "t_with_locale",
    "t_with_locale_late",
    "t_with_locale_missing",
    "t_with_args",
    "t_with_args (str)",
    "t_with_args (many)",
    "t_dynamic_key",
    "t_dynamic_locale",
    "t_with_dynamic_args",
    "t_with_threads",
    "t_lorem_ipsum",
    "t_large",
    "t_large_with_locale",
    "t_large_with_args",
    "t_large_dynamic_key",
)
FILTER = r"^(?:t$|t_with_locale(?:_(?:late|missing))?$|t_with_args(?: \((?:str|many)\))?$|t_dynamic_(?:key|locale)$|t_with_dynamic_args$|t_with_threads$|t_lorem_ipsum$|t_large(?:_with_locale|_with_args|_dynamic_key)?$|replace_patterns_)"
# Each Criterion target defines its own `i18n!` catalog.
BENCH_TARGETS = ("bench", "large_catalog")
LARGE_CATALOG_BENCH = """
[[bench]]
harness = false
name = "large_catalog"
"""


def run(command, *, cwd=ROOT, env=None, stdout=None):
    print("+", " ".join(map(str, command)), file=sys.stderr, flush=True)
    return subprocess.run(command, cwd=cwd, env=env, stdout=stdout, check=True)


def capture(command):
    return subprocess.check_output(command, cwd=ROOT, text=True).strip()


def cpu_name():
    if sys.platform == "linux":
        cpuinfo = Path("/proc/cpuinfo")
        if cpuinfo.exists():
            for line in cpuinfo.read_text().splitlines():
                if line.startswith("model name"):
                    return line.split(":", 1)[1].strip()
    elif sys.platform == "darwin":
        for key in ("machdep.cpu.brand_string", "hw.model"):
            result = subprocess.run(
                ["sysctl", "-n", key], text=True, capture_output=True, check=False
            )
            if result.returncode == 0 and result.stdout.strip():
                return result.stdout.strip()
    return platform.processor() or "unknown"


def export_baseline(base, destination, archive_path):
    with archive_path.open("wb") as archive:
        run(["git", "archive", "--format=tar", base], stdout=archive)
    destination.mkdir()
    with tarfile.open(archive_path) as archive:
        # Git archive contents are trusted, but refuse paths outside the export.
        for member in archive.getmembers():
            member_path = (destination / member.name).resolve()
            if not member_path.is_relative_to(destination.resolve()):
                raise ValueError(f"Unsafe archive path: {member.name}")
        # The filter keyword was added in Python 3.12; macOS system Python
        # may be older. The archive comes from the local Git repository.
        if sys.version_info >= (3, 12):
            archive.extractall(destination, filter="data")
        else:
            archive.extractall(destination)
    archive_path.unlink()
    shutil.copy2(ROOT / "benches/bench.rs", destination / "benches/bench.rs")
    shutil.copy2(ROOT / "benches/large_catalog.rs", destination / "benches/large_catalog.rs")
    fixtures = destination / "benches/fixtures/large_catalog"
    shutil.rmtree(fixtures, ignore_errors=True)
    shutil.copytree(ROOT / "benches/fixtures/large_catalog", fixtures)
    manifest = destination / "Cargo.toml"
    if 'name = "large_catalog"' not in manifest.read_text():
        with manifest.open("a") as file:
            file.write(LARGE_CATALOG_BENCH)
    shutil.copy2(ROOT / "examples/size_probe.rs", destination / "examples/size_probe.rs")
    # A shared lockfile keeps dependency versions identical across revisions.
    # The baseline is built without --locked, so Cargo may drop packages that
    # only the current revision uses; it keeps the versions of shared ones.
    shutil.copy2(ROOT / "Cargo.lock", destination / "Cargo.lock")


def benchmark_environment(target):
    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = str(target)
    environment["CRITERION_HOME"] = str(target / "criterion")
    return environment


def lock_flags(source):
    return ["--locked"] if source == ROOT else []


def build_benchmark(source, target):
    targets = [argument for name in BENCH_TARGETS for argument in ("--bench", name)]
    run(
        ["cargo", "bench", *lock_flags(source), *targets, "--no-run"],
        cwd=source,
        env=benchmark_environment(target),
    )
    executables = []
    for name in BENCH_TARGETS:
        candidates = [
            path for path in (target / "release/deps").glob(f"{name}-*")
            if path.is_file() and os.access(path, os.X_OK)
        ]
        if len(candidates) != 1:
            raise RuntimeError(
                f"Expected one {name} benchmark executable in {target}, found {candidates}"
            )
        executables.append(candidates[0])
    return executables


def benchmark(source, target, executables, sample_size, pass_number):
    for executable in executables:
        run(
            [
                str(executable), "--bench", FILTER, "--sample-size", str(sample_size),
                "--save-baseline", f"pass_{pass_number}",
            ],
            cwd=source,
            env=benchmark_environment(target),
        )


def build_size_probe(source, target):
    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = str(target)
    run(
        [
            "cargo", "rustc", *lock_flags(source), "--release", "--example", "size_probe",
            "--", "-C", "strip=symbols",
        ],
        cwd=source,
        env=environment,
    )
    executable = target / "release/examples" / ("size_probe.exe" if os.name == "nt" else "size_probe")
    result = subprocess.run(
        [str(executable), "en", "Jason"],
        cwd=source,
        capture_output=True,
        check=True,
    )
    return executable.stat().st_size, result.stdout


def estimates(target, pass_number):
    criterion = target / "criterion"
    results = {}
    for path in criterion.glob(f"*/pass_{pass_number}/estimates.json"):
        name = path.parent.parent.name
        data = json.loads(path.read_text())["mean"]
        results[name] = data
    return results


def number(value):
    return f"{value:,.2f}"


def mean_of_passes(passes, name):
    return sum(result[name]["point_estimate"] for result in passes) / len(passes)


def pass_range(passes, name):
    values = [result[name]["point_estimate"] for result in passes]
    return f"{number(min(values))}–{number(max(values))}"


def within_pass_intervals(passes, name):
    intervals = []
    for index, result in enumerate(passes, 1):
        interval = result[name]["confidence_interval"]
        intervals.append(
            f"{index}: [{number(interval['lower_bound'])}, "
            f"{number(interval['upper_bound'])}]"
        )
    return "; ".join(intervals)


def report(base, sample_size, old_passes, new_passes, old_size, new_size):
    replacements = sorted(name for name in new_passes[0] if name.startswith("replace_patterns_"))
    names = (*CORE_BENCHMARKS, *replacements)
    if not replacements:
        raise RuntimeError("No replace_patterns_ benchmarks were measured")
    missing = [
        name for name in names
        if any(name not in result for result in (*old_passes, *new_passes))
    ]
    if missing:
        raise RuntimeError(f"Missing Criterion measurements: {', '.join(missing)}")

    lines = [
        "# Benchmark comparison",
        "",
        f"Baseline: `{base}`; current checkout: `{capture(['git', 'rev-parse', '--short', 'HEAD'])}` "
        "(including working-tree changes).",
        f"Hardware: {cpu_name()} ({platform.machine()}, {os.cpu_count()} logical CPUs); "
        f"OS: {platform.platform()}.",
        f"Rust compiler: `{capture(['rustc', '--version'])}`; "
        f"Criterion sample size: {sample_size}; {len(old_passes)} passes per revision.",
        "",
        "| Benchmark | Old mean (ns) | Old pass range (ns) | New mean (ns) | New pass range (ns) | Speedup (old/new) | Performance increase |",
        "| --- | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for name in names:
        old_ns = mean_of_passes(old_passes, name)
        new_ns = mean_of_passes(new_passes, name)
        ratio = old_ns / new_ns
        lines.append(
            f"| `{name}` | {number(old_ns)} | {pass_range(old_passes, name)} | "
            f"{number(new_ns)} | {pass_range(new_passes, name)} | "
            f"{ratio:.2f}× | {(ratio - 1) * 100:+.1f}% |"
        )
    lines.extend(
        [
            "",
            "Each mean is the arithmetic mean of Criterion's mean estimates across "
            "passes. Pass ranges show the minimum and maximum of those estimates; "
            "they describe run-to-run variation, not confidence intervals. Both "
            "benchmark binaries were built before timing, with the same benchmark "
            "source, lockfile, compiler, and default build flags. Runs alternated "
            "old/new, then new/old.",
            "",
            "### Within-pass Criterion 95% confidence intervals (ns)",
            "",
            "| Benchmark | Old passes | New passes |",
            "| --- | --- | --- |",
        ]
    )
    for name in names:
        lines.append(
            f"| `{name}` | {within_pass_intervals(old_passes, name)} | "
            f"{within_pass_intervals(new_passes, name)} |"
        )
    lines.extend(
        [
            "",
            "Pass numbers are ordered within each revision; these intervals reflect "
            "Criterion sampling inside one pass and are separate from the ranges above.",
            "",
            "## Representative executable size",
            "",
            "| Artifact | Old (bytes) | New (bytes) | Size change |",
            "| --- | ---: | ---: | ---: |",
            f"| `size_probe` | {old_size:,} | {new_size:,} | "
            f"{(new_size / old_size - 1) * 100:+.2f}% |",
            "",
            "Both revisions built the same `size_probe` example with "
            "`cargo rustc --release --example size_probe -- -C strip=symbols` "
            "(`--locked` for the current revision). "
            "Their output for `en Jason` matched. This is one representative stripped "
            "executable, not a universal artifact size.",
            "",
        ]
    )
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", required=True, help="Baseline Git commit or ref")
    parser.add_argument(
        "--output", type=Path, default=Path("target/benchmark-comparison.md"),
        help="Markdown report path (relative paths are resolved from the repository root)",
    )
    parser.add_argument("--sample-size", type=int, default=30)
    parser.add_argument("--passes", type=int, default=2, help="Measurement passes per revision")
    parser.add_argument(
        "--debug", action="store_true", help="Permit sample sizes below 30 for script debugging"
    )
    parser.add_argument(
        "--append-github-summary", action="store_true",
        help="Also append the report to the file named by GITHUB_STEP_SUMMARY",
    )
    args = parser.parse_args()
    if args.sample_size < 30 and not args.debug:
        parser.error("--sample-size below 30 requires --debug")
    if args.sample_size < 10:
        parser.error("Criterion requires --sample-size of at least 10")
    if args.passes < 1:
        parser.error("--passes must be at least 1")
    if args.append_github_summary and not os.environ.get("GITHUB_STEP_SUMMARY"):
        parser.error("--append-github-summary requires GITHUB_STEP_SUMMARY")

    base = capture(["git", "rev-parse", "--verify", f"{args.base}^{{commit}}"])
    run_dir = ROOT / "target" / "benchmark-comparison" / uuid.uuid4().hex
    run_dir.mkdir(parents=True)
    baseline_source = run_dir / "baseline-source"
    export_baseline(base, baseline_source, run_dir / "baseline.tar")
    old_target = run_dir / "old-target"
    new_target = run_dir / "new-target"
    old_executables = build_benchmark(baseline_source, old_target)
    new_executables = build_benchmark(ROOT, new_target)
    old_passes = []
    new_passes = []
    revisions = (
        (baseline_source, old_target, old_executables, old_passes),
        (ROOT, new_target, new_executables, new_passes),
    )
    for pass_number in range(1, args.passes + 1):
        for source, target, executables, results in (
            revisions if pass_number % 2 else reversed(revisions)
        ):
            benchmark(source, target, executables, args.sample_size, pass_number)
            results.append(estimates(target, pass_number))
    old_size, old_output = build_size_probe(baseline_source, old_target)
    new_size, new_output = build_size_probe(ROOT, new_target)
    if old_output != new_output:
        raise RuntimeError(
            "size_probe output differs between baseline and current revision for en Jason"
        )

    output = args.output if args.output.is_absolute() else ROOT / args.output
    output.parent.mkdir(parents=True, exist_ok=True)
    markdown = report(base, args.sample_size, old_passes, new_passes, old_size, new_size)
    output.write_text(markdown)
    print(f"Wrote {output}")
    if args.append_github_summary:
        with Path(os.environ["GITHUB_STEP_SUMMARY"]).open("a") as summary:
            summary.write(markdown)


if __name__ == "__main__":
    main()
