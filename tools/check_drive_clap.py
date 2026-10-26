#!/usr/bin/env python3
"""Build and check the Drive CLAP development artifact. Never installs it."""
from __future__ import annotations
import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import shutil
import subprocess
import sys
from package_plugins import finalize_macos_bundle

ROOT = Path(__file__).resolve().parents[1]
FORBIDDEN = {"daw-desktop", "daw-cli", "daw-core", "daw-ui", "daw-engine", "daw-project", "daw-output", "daw-media", "daw-separation", "cpal", "rfd", "eframe"}
SIGNATURE = "DAW Drive CLAP Phase 2 scratch v1\n"

def scratch(path: Path) -> Path:
    path = path.resolve()
    if path == ROOT / "work" or not path.is_relative_to(ROOT / "work"):
        raise ValueError("Choose a dedicated scratch directory under ignored work/")
    path.mkdir(parents=True, exist_ok=True)
    marker = path / ".drive-clap-phase2"
    if marker.exists():
        if marker.read_text() != SIGNATURE:
            raise ValueError("Unrecognized scratch ownership marker")
    elif any(path.iterdir()):
        raise ValueError("Retaining unmarked scratch contents; choose a fresh --work-dir")
    else:
        marker.write_text(SIGNATURE)
    return path

def run(command: list[str], log: Path, env: dict, allowed=(0,)) -> str:
    print("Running " + " ".join(map(str, command[:4])), flush=True)
    result = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True)
    log.write_text(result.stdout)
    log.with_suffix(log.suffix + ".stderr").write_text(result.stderr)
    if result.returncode not in allowed:
        raise RuntimeError(f"Command failed ({result.returncode}); inspect {log} and its .stderr file")
    return result.stdout

def assess_validator(results: list[dict]) -> dict:
    failed = [row for row in results if row["status"]["code"] not in ("success", "skipped")]
    # Keep the full upstream failure. Do not reinterpret it as a validator pass.
    names = [next(iter(row["test"].values()))["test"] for row in results]
    complete = len(results) == 44 and len(set(names)) == 44
    expected = complete and len(failed) == 1 and failed[0]["status"]["code"] == "failed" and failed[0]["test"].get("plugin-instance", {}).get("test") == "process-varying-sample-rates" and "8000.00hz" in (failed[0]["status"].get("details") or "") and "activate()" in (failed[0]["status"].get("details") or "")
    counts = Counter(row["status"]["code"] for row in results)
    return {"counts": dict(counts), "all_passed": complete and not failed, "complete_result_set": complete, "only_failure_is_outside_six_rate_contract": expected, "failures": failed}

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work-dir", type=Path, default=ROOT / "work/drive-plugin-phase2")
    parser.add_argument("--validator", type=Path, default=ROOT / "work/drive-plugin-phase0/clap-validator/target/release/clap-validator")
    parser.add_argument("--fetch", action="store_true", help="Allow Cargo to download missing locked dependencies")
    parser.add_argument("--strict-validator", action="store_true", help="Exit 2 for any upstream validator failure, including its unsupported-rate sweep")
    args = parser.parse_args()
    work = scratch(args.work_dir)
    (work / "report.json").write_text(json.dumps({"schema_version": 1, "run_completed": False, "release_ready": False}) + "\n")
    validator = args.validator.resolve()
    if not validator.is_file():
        raise ValueError("Missing clap-validator; build the pinned 0.4.1 validator or pass --validator (see README)")
    env = os.environ.copy()
    version = run([str(validator), "--version"], work / "validator-version.log", env).strip()
    if version != "clap-validator 0.4.1":
        raise ValueError("This check records clap-validator 0.4.1; select that version explicitly")
    env.pop("CARGO_BUILD_TARGET", None)  # This runner executes native binaries.
    cargo = ["cargo"]
    target = work / "target"
    offline = [] if args.fetch else ["--offline"]
    build = cargo + ["build", "-p", "daw-drive-clap", "--release", "--locked", "--target-dir", str(target)] + offline
    artifact = {"Darwin": "libdaw_drive_clap.dylib", "Linux": "libdaw_drive_clap.so", "Windows": "daw_drive_clap.dll"}.get(platform.system())
    if artifact is None:
        raise ValueError("Unsupported development build platform")
    run(build + ["--features", "allocation-check"], work / "instrumented-build.log", env)
    instrumented = work / ("allocation-check-" + artifact)
    shutil.copy2(target / "release" / artifact, instrumented)
    tests = cargo + ["test", "-p", "daw-drive-clap", "--release", "--locked", "--target-dir", str(target)] + offline + ["--test", "host"]
    test_env = dict(env, DRIVE_CLAP_TEST_LIBRARY=str(instrumented), DRIVE_CLAP_REQUIRE_ALLOCATION_CHECK="1")
    run(tests, work / "allocation-parity-tests.log", test_env)
    output = run(build + ["--message-format=json"], work / "targeted-build.jsonl", env)
    names = {row["target"]["name"].replace("_", "-") for line in output.splitlines() if (row := json.loads(line)).get("reason") == "compiler-artifact"}
    if names & FORBIDDEN:
        raise RuntimeError("Unexpected DAW/device/window artifact targets: " + ", ".join(sorted(names & FORBIDDEN)))
    metadata = json.loads(run(cargo + ["metadata", "--no-deps", "--locked", "--format-version", "1"] + offline, work / "metadata.json", env))
    plugin_version = next(p["version"] for p in metadata["packages"] if p["name"] == "daw-drive-plugin")
    bundle = work / "Drive.clap"
    if platform.system() == "Darwin":
        binary = bundle / "Contents/MacOS/Drive"
        binary.parent.mkdir(parents=True, exist_ok=True)
        (bundle / "Contents/Info.plist").write_bytes(plistlib.dumps({"CFBundleExecutable": "Drive", "CFBundleIdentifier": "io.github.fecabrera.daw.drive", "CFBundleName": "Drive", "CFBundlePackageType": "BNDL", "CFBundleVersion": plugin_version, "CFBundleShortVersionString": plugin_version}))
        notices = bundle / "Contents/Resources/licenses"
    else:
        binary = bundle
        notices = work / "notices"
    shutil.copy2(target / "release" / artifact, binary)
    notices.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT / "LICENSE", notices / "DAW-LICENSE.txt")
    for source, name in [("crates/plugin-clap/assets/licenses", "clap"), ("crates/equalizer/assets/licenses", "dsp"), ("crates/oscillator/assets/licenses", "oscillator-dsp")]:
        shutil.copytree(ROOT / source, notices / name, dirs_exist_ok=True)
    for source, name in [("crates/plugin-window/assets/licenses", "window"), ("crates/unit-ui/assets/licenses", "fallback-fonts"), ("crates/ui/assets/licenses", "ui")]:
        shutil.copytree(ROOT / source, notices / name, dirs_exist_ok=True)
    for source in ["crates/unit-ui/assets/fonts/OFL.txt", "crates/unit-ui/assets/fonts/Hack-LICENSE.txt"]:
        shutil.copy2(ROOT / source, notices / ("Outfit-OFL.txt" if source.endswith("OFL.txt") else "Hack-LICENSE.txt"))
    (notices / "DEVELOPMENT.txt").write_text("Drive editor development artifact. Complete release dependency/notice audit, native accessibility, platform and external-host checks remain gates. Not a release package.\n")
    if platform.system() == "Darwin":
        finalize_macos_bundle(bundle, binary)
    normal_env = dict(env, DRIVE_CLAP_TEST_LIBRARY=str(binary))
    normal_env.pop("DRIVE_CLAP_REQUIRE_ALLOCATION_CHECK", None)
    run(tests, work / "artifact-parity-tests.log", normal_env)
    bench_env = dict(normal_env, DRIVE_CLAP_BENCH_REPORT=str(work / "callback-budgets.json"))
    run(tests + ["performance::callback_cpu_budget", "--", "--ignored"], work / "callback-budget-tests.log", bench_env)
    raw = json.loads(run([str(validator), "validate", "--json", str(bundle)], work / "validator.json", env, allowed=(0, 1)))
    assessment = assess_validator(raw["results"])
    hardware = {}
    if platform.system() == "Darwin":
        for key in ["machdep.cpu.brand_string", "hw.ncpu", "hw.memsize"]:
            query = subprocess.run(["sysctl", "-n", key], text=True, capture_output=True)
            hardware[key] = query.stdout.strip() if query.returncode == 0 else "unavailable"
    compiler = run(["rustc", "--version"], work / "rustc.log", env).strip()
    report = {"schema_version": 1, "run_completed": True, "rustc": compiler, "hardware": hardware, "profile": "release, thin LTO, one codegen unit", "platform": platform.system(), "architecture": platform.machine(), "plugin_version": plugin_version,
              "validator": {"version": version, **assessment}, "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
              "callback_budgets": json.loads((work / "callback-budgets.json").read_text()), "instrumented_and_normal_binary_abi_tests_passed": True,
              "audio_thread_rust_allocations_and_deallocations": 0, "allocator_positive_control_passed": True, "artifact_targets": sorted(names), "editor_present": True, "external_daw_checked": False, "release_ready": False}
    (work / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"report": str(work / "report.json"), "validator": assessment, "release_ready": False}, indent=2))
    if not assessment["all_passed"] and (args.strict_validator or not assessment["only_failure_is_outside_six_rate_contract"]):
        return 2
    return 0

if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, RuntimeError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
