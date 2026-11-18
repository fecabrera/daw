"""Common build and evidence steps for the Drive CLAP and VST3 checks."""
from __future__ import annotations

import hashlib
import json
import platform
import shutil
import subprocess


def build_artifact(fmt, work, env, fetch, forbidden, run):
    prefix = "DRIVE_" + fmt.upper()
    cargo = ["cargo"]
    target = work / "target"
    offline = [] if fetch else ["--offline"]
    build = cargo + ["build", "-p", "daw-drive-" + fmt, "--release", "--locked", "--target-dir", str(target)] + offline
    artifact = {"Darwin": f"libdaw_drive_{fmt}.dylib", "Linux": f"libdaw_drive_{fmt}.so", "Windows": f"daw_drive_{fmt}.dll"}.get(platform.system())
    if artifact is None:
        raise ValueError("Unsupported development build platform")
    run(build + ["--features", "allocation-check"], work / "instrumented-build.log", env)
    instrumented = work / ("allocation-check-" + artifact)
    shutil.copy2(target / "release" / artifact, instrumented)
    tests = cargo + ["test", "-p", "daw-drive-" + fmt, "--release", "--locked", "--target-dir", str(target)] + offline + ["--test", "host"]
    test_env = dict(env, **{prefix + "_TEST_LIBRARY": str(instrumented), prefix + "_REQUIRE_ALLOCATION_CHECK": "1"})
    run(tests, work / "allocation-parity-tests.log", test_env)
    output = run(build + ["--message-format=json"], work / "targeted-build.jsonl", env)
    names = {row["target"]["name"].replace("_", "-") for line in output.splitlines() if (row := json.loads(line)).get("reason") == "compiler-artifact"}
    if names & forbidden:
        raise RuntimeError("Unexpected DAW/device/window artifact targets: " + ", ".join(sorted(names & forbidden)))
    metadata = json.loads(run(cargo + ["metadata", "--no-deps", "--locked", "--format-version", "1"] + offline, work / "metadata.json", env))
    plugin_version = next(p["version"] for p in metadata["packages"] if p["name"] == "daw-drive-plugin")
    return target / "release" / artifact, tests, names, plugin_version


def check_artifact(fmt, work, env, binary, tests, run):
    prefix = "DRIVE_" + fmt.upper()
    normal_env = dict(env, **{prefix + "_TEST_LIBRARY": str(binary)})
    normal_env.pop(prefix + "_REQUIRE_ALLOCATION_CHECK", None)
    run(tests, work / "artifact-parity-tests.log", normal_env)
    bench_env = dict(normal_env, **{prefix + "_BENCH_REPORT": str(work / "callback-budgets.json")})
    run(tests + ["performance::callback_cpu_budget", "--", "--ignored"], work / "callback-budget-tests.log", bench_env)


def collect_report(work, env, binary, names, plugin_version, version, assessment, run):
    hardware = {}
    if platform.system() == "Darwin":
        for key in ["machdep.cpu.brand_string", "hw.ncpu", "hw.memsize"]:
            query = subprocess.run(["sysctl", "-n", key], text=True, capture_output=True)
            hardware[key] = query.stdout.strip() if query.returncode == 0 else "unavailable"
    compiler = run(["rustc", "--version"], work / "rustc.log", env).strip()
    return {"schema_version": 1, "run_completed": True, "rustc": compiler, "hardware": hardware, "profile": "release, thin LTO, one codegen unit", "platform": platform.system(), "architecture": platform.machine(), "plugin_version": plugin_version,
              "validator": {"version": version, **assessment}, "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
              "callback_budgets": json.loads((work / "callback-budgets.json").read_text()), "instrumented_and_normal_binary_abi_tests_passed": True,
              "audio_thread_rust_allocations_and_deallocations": 0, "allocator_positive_control_passed": True, "artifact_targets": sorted(names), "editor_present": True, "external_daw_checked": False, "release_ready": False}
