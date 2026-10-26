#!/usr/bin/env python3
"""Build and check the Drive VST3 development artifact. Never installs it."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import shutil
import re
import subprocess
import sys
from package_drive_plugins import finalize_macos_bundle

ROOT = Path(__file__).resolve().parents[1]
FORBIDDEN = {"daw-desktop", "daw-cli", "daw-core", "daw-ui", "daw-engine", "daw-project", "daw-output", "daw-media", "daw-separation", "cpal", "rfd", "eframe", "clap-sys", "daw-plugin-clap", "daw-drive-clap"}
SIGNATURE = "DAW Drive VST3 Phase 3 scratch v1\n"

def scratch(path: Path) -> Path:
    path = path.resolve()
    if path == ROOT / "work" or not path.is_relative_to(ROOT / "work"):
        raise ValueError("Choose a dedicated scratch directory under ignored work/")
    path.mkdir(parents=True, exist_ok=True)
    marker = path / ".drive-vst3-phase3"
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
    log.with_suffix(log.suffix + ".status").write_text(str(result.returncode))
    log.with_suffix(log.suffix + ".stderr").write_text(result.stderr)
    if result.returncode not in allowed:
        raise RuntimeError(f"Command failed ({result.returncode}); inspect {log} and its .stderr file")
    return result.stdout

def assess_validator(output: str, exit_code: int = 0) -> dict:
    summaries = re.findall(r"Result: (\d+) tests passed, (\d+) tests failed", output)
    complete = len(summaries) == 1 and sum(map(int, summaries[0])) == 47
    passed, failed = map(int, summaries[0]) if len(summaries) == 1 else (0, 0)
    diagnostics = [line for line in output.splitlines() if "setupProcessing (..) failed for samplerate" in line]
    return {"passed": passed, "failed": failed, "complete_result_set": complete, "exit_code": exit_code,
            "all_passed": complete and failed == 0 and exit_code == 0,
            "unsupported_rate_diagnostics": diagnostics}

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work-dir", type=Path, default=ROOT / "work/drive-plugin-phase3")
    parser.add_argument("--validator", type=Path, default=ROOT / "work/drive-plugin-phase0/vst3-build/bin/Release/validator")
    parser.add_argument("--fetch", action="store_true", help="Allow Cargo to download missing locked dependencies")

    args = parser.parse_args()
    work = scratch(args.work_dir)
    (work / "report.json").write_text(json.dumps({"schema_version": 1, "run_completed": False, "release_ready": False}) + "\n")
    validator = args.validator.resolve()
    if not validator.is_file():
        raise ValueError("Missing VST3 validator; build the pinned SDK validator or pass --validator (see README)")
    env = os.environ.copy()
    version = run([str(validator), "--version"], work / "validator-version.log", env).strip()
    if "3.8.1" not in version:
        raise ValueError("This check records SDK validator 3.8.1; select that version explicitly")
    env.pop("CARGO_BUILD_TARGET", None)  # This runner executes native binaries.
    cargo = ["cargo"]
    target = work / "target"
    offline = [] if args.fetch else ["--offline"]
    build = cargo + ["build", "-p", "daw-drive-vst3", "--release", "--locked", "--target-dir", str(target)] + offline
    artifact = {"Darwin": "libdaw_drive_vst3.dylib", "Linux": "libdaw_drive_vst3.so", "Windows": "daw_drive_vst3.dll"}.get(platform.system())
    if artifact is None:
        raise ValueError("Unsupported development build platform")
    run(build + ["--features", "allocation-check"], work / "instrumented-build.log", env)
    instrumented = work / ("allocation-check-" + artifact)
    shutil.copy2(target / "release" / artifact, instrumented)
    tests = cargo + ["test", "-p", "daw-drive-vst3", "--release", "--locked", "--target-dir", str(target)] + offline + ["--test", "host"]
    test_env = dict(env, DRIVE_VST3_TEST_LIBRARY=str(instrumented), DRIVE_VST3_REQUIRE_ALLOCATION_CHECK="1")
    run(tests, work / "allocation-parity-tests.log", test_env)
    output = run(build + ["--message-format=json"], work / "targeted-build.jsonl", env)
    names = {row["target"]["name"].replace("_", "-") for line in output.splitlines() if (row := json.loads(line)).get("reason") == "compiler-artifact"}
    if names & FORBIDDEN:
        raise RuntimeError("Unexpected DAW/device/window artifact targets: " + ", ".join(sorted(names & FORBIDDEN)))
    metadata = json.loads(run(cargo + ["metadata", "--no-deps", "--locked", "--format-version", "1"] + offline, work / "metadata.json", env))
    plugin_version = next(p["version"] for p in metadata["packages"] if p["name"] == "daw-drive-plugin")
    bundle = work / "Drive.vst3"
    if platform.system() == "Darwin":
        binary = bundle / "Contents/MacOS/Drive"
        binary.parent.mkdir(parents=True, exist_ok=True)
        (bundle / "Contents/Info.plist").write_bytes(plistlib.dumps({"CFBundleExecutable": "Drive", "CFBundleIdentifier": "io.github.fecabrera.daw.drive.vst3", "CFBundleName": "Drive", "CFBundlePackageType": "BNDL", "CFBundleVersion": plugin_version, "CFBundleShortVersionString": plugin_version}))
    else:
        architecture = "x86_64-linux" if platform.system() == "Linux" else "x86_64-win"
        binary = bundle / "Contents" / architecture / ("Drive.so" if platform.system() == "Linux" else "Drive.vst3")
        binary.parent.mkdir(parents=True, exist_ok=True)
    notices = bundle / "Contents/Resources/licenses"
    shutil.copy2(target / "release" / artifact, binary)
    notices.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT / "LICENSE", notices / "DAW-LICENSE.txt")
    for source, name in [("crates/plugin-vst3/assets/licenses", "vst3"), ("crates/equalizer/assets/licenses", "dsp"), ("crates/oscillator/assets/licenses", "oscillator-dsp")]:
        shutil.copytree(ROOT / source, notices / name, dirs_exist_ok=True)
    for source, name in [("crates/plugin-window/assets/licenses", "window"), ("crates/unit-ui/assets/licenses", "fallback-fonts"), ("crates/ui/assets/licenses", "ui")]:
        shutil.copytree(ROOT / source, notices / name, dirs_exist_ok=True)
    for source in ["crates/unit-ui/assets/fonts/OFL.txt", "crates/unit-ui/assets/fonts/Hack-LICENSE.txt"]:
        shutil.copy2(ROOT / source, notices / ("Outfit-OFL.txt" if source.endswith("OFL.txt") else "Hack-LICENSE.txt"))
    (notices / "DEVELOPMENT.txt").write_text("Drive editor development artifact. Complete release dependency/notice audit, native accessibility, platform and external-host checks remain gates. Not a release package.\n")
    if platform.system() == "Darwin":
        finalize_macos_bundle(bundle, binary)
    normal_env = dict(env, DRIVE_VST3_TEST_LIBRARY=str(binary))
    normal_env.pop("DRIVE_VST3_REQUIRE_ALLOCATION_CHECK", None)
    run(tests, work / "artifact-parity-tests.log", normal_env)
    bench_env = dict(normal_env, DRIVE_VST3_BENCH_REPORT=str(work / "callback-budgets.json"))
    run(tests + ["performance::callback_cpu_budget", "--", "--ignored"], work / "callback-budget-tests.log", bench_env)
    raw = run([str(validator), str(bundle)], work / "validator.log", env, allowed=(0, 1, 255))
    assessment = assess_validator(raw, int((work / "validator.log.status").read_text()))
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
    if not assessment["all_passed"]:
        return 2
    return 0

if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, RuntimeError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
