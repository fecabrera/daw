#!/usr/bin/env python3
"""Reproduce the macOS Drive Phase 0 feasibility experiment in ignored work space.

This builds disposable gain probes, not Drive plugins. A completed experiment
may reject the candidate framework; consult production_contract_satisfied.
Use --fetch for explicit dependency downloads and --gui for temporary windows.
No plugin is installed, no user project is opened, and no release is published.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import platform
import plistlib
import re
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
TOOL = ROOT / "tools" / "drive_plugin_phase0"
SOURCES = json.loads((TOOL / "sources.json").read_text())
FORBIDDEN = {"daw-desktop", "daw-cli", "daw-core", "daw-engine", "daw-project", "daw-output", "daw-media", "daw-separation", "cpal", "rfd", "eframe"}


def assess_framework(vst3: dict) -> dict:
    """These are independent production gates; validator success cannot waive them."""
    curve = vst3.get("curve_interpolated") is True
    bounded = vst3.get("dense_event_allocations") == 0 and vst3.get("dense_event_deallocations") == 0
    return {"vst3_curve_contract": curve, "callback_allocation_contract": bounded,
            "production_contract_satisfied": curve and bounded}


def reachable_packages(metadata: dict) -> list[dict]:
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    pending = [metadata["resolve"]["root"]]
    reached = set()
    while pending:
        identity = pending.pop()
        if identity in reached:
            continue
        reached.add(identity)
        pending.extend(dep["pkg"] for dep in nodes[identity]["deps"])
    return [package for package in metadata["packages"] if package["id"] in reached]


def assess_baseline(baseline: dict, budgets: dict) -> dict:
    tail_seconds = budgets["proposed_tail_policy_seconds"]
    tails = baseline.get("tails", [])
    timings = baseline.get("direct_drive_timings", [])
    tail_ok = len(tails) == 6 and all(item["last_above_threshold_frame"] < item["rate"] * tail_seconds for item in tails)
    callback_ok = len(timings) == 4 and all(item["p99_ns"] <= item["period_ns"] * budgets["callback_p99_budget_fraction_of_period"] for item in timings)
    return {"tail_survey_within_policy": tail_ok, "direct_baseline_within_callback_budget": callback_ok}


def prepare_work_directory(path: Path, root: Path = ROOT) -> Path:
    root = root.resolve()
    work = path.resolve()
    if not work.is_relative_to(root / "work") or work == root / "work":
        raise ValueError("Scratch directory must be a dedicated directory under the repository's ignored work/")
    work.mkdir(parents=True, exist_ok=True)
    marker = work / ".drive-plugin-phase0"
    signature = "DAW Drive Phase 0 scratch v1\n"
    if marker.exists():
        if marker.read_text() != signature:
            raise ValueError("Scratch directory has an unrecognized ownership marker")
    elif any(work.iterdir()):
        raise ValueError("Retaining unmarked scratch contents; choose a fresh --work-dir")
    else:
        marker.write_text(signature)
    return work


def run(command: list[str], log: Path, env: dict[str, str], *, cwd: Path = ROOT) -> str:
    log.parent.mkdir(parents=True, exist_ok=True)
    print("Running " + " ".join(str(arg) for arg in command[:3]), flush=True)
    result = subprocess.run(command, cwd=cwd, env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    log.write_text(result.stdout)
    log.with_suffix(log.suffix + ".stderr").write_text(result.stderr)
    if result.returncode:
        raise RuntimeError(f"Command failed ({result.returncode}); inspect {log} and its .stderr file")
    return result.stdout


def prepare_source(name: str, source: dict, work: Path, env: dict[str, str], fetch: bool) -> None:
    path = work / name
    created = not path.exists()
    if created:
        if not fetch:
            raise RuntimeError(f"Missing {name}; rerun with --fetch to download pinned sources")
        run(["git", "clone", "--no-checkout", "--depth", "1", source["url"], str(path)], work / f"{name}-clone.log", env)
    # A new --no-checkout clone must be checked out even if HEAD already matches.
    # Existing tracked changes are never discarded, including at the right HEAD.
    if not created:
        dirty = subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=no"], cwd=path, env=env, text=True)
        if dirty:
            raise RuntimeError(f"Retaining local changes in {path}; use a fresh scratch directory")
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=path, env=env, text=True, capture_output=True)
    if created or head.returncode or head.stdout.strip() != source["revision"]:
        if not fetch:
            raise RuntimeError(f"{name} revision differs from sources.json; use a fresh scratch directory or --fetch")
        run(["git", "fetch", "--depth", "1", "origin", source["revision"]], work / f"{name}-fetch.log", env, cwd=path)
        run(["git", "checkout", "--detach", source["revision"]], work / f"{name}-checkout.log", env, cwd=path)


def prepare_sources(work: Path, env: dict[str, str], fetch: bool) -> None:
    for name, source in SOURCES.items():
        prepare_source(name, source, work, env, fetch)
    if fetch:
        run(["git", "submodule", "update", "--init", "--depth", "1", "base", "cmake", "pluginterfaces", "public.sdk"], work / "vst3-submodules.log", env, cwd=work / "vst3sdk")
        run(["cargo", "fetch", "--locked", "--manifest-path", str(TOOL / "Cargo.toml")], work / "probe-fetch.log", env)
        run(["cargo", "fetch", "--locked", "--manifest-path", str(work / "clap-validator" / "Cargo.toml")], work / "validator-fetch.log", env)
    status = subprocess.check_output(["git", "submodule", "status", "base", "cmake", "pluginterfaces", "public.sdk"], cwd=work / "vst3sdk", env=env, text=True)
    if any(not line.startswith(" ") for line in status.splitlines()):
        raise RuntimeError("VST3 submodules differ from their pinned gitlinks; retain changes and use fresh scratch space")
    for relative in ["base/source/fobject.cpp", "cmake/modules/SMTG_VST3_SDK.cmake", "pluginterfaces/vst/ivstaudioprocessor.h", "public.sdk/source/vst/vstinitiids.cpp"]:
        if not (work / "vst3sdk" / relative).is_file():
            raise RuntimeError("Missing pinned VST3 submodules; rerun with --fetch")


def notices(work: Path, env: dict[str, str]) -> tuple[list[dict], Path]:
    metadata = json.loads(run(["cargo", "metadata", "--locked", "--offline", "--format-version", "1", "--filter-platform", "aarch64-apple-darwin", "--manifest-path", str(TOOL / "Cargo.toml")], work / "cargo-metadata.json", env))
    packages = reachable_packages(metadata)
    unexpected = FORBIDDEN.intersection(package["name"] for package in packages)
    if unexpected:
        raise RuntimeError("Probe depends on DAW/application packages: " + ", ".join(sorted(unexpected)))
    inventory = [{key: package.get(key) for key in ["name", "version", "license", "license_file", "source"]} for package in packages]
    (work / "dependency-inventory.json").write_text(json.dumps(inventory, indent=2) + "\n")
    destination = work / "notices"
    destination.mkdir(exist_ok=True)
    shutil.copy2(ROOT / "LICENSE", destination / "DAW-LICENSE")
    # Copy published license/notice texts including nested font notices. No source
    # code is copied into the repository or included in the prototype bundles.
    extra = json.loads((TOOL / "licenses" / "sources.json").read_text())
    shutil.copytree(TOOL / "licenses", destination / "framework", dirs_exist_ok=True)
    missing_texts = []
    for package in packages:
        source = Path(package["manifest_path"]).parent
        if source.is_relative_to(ROOT):
            continue
        found_text = False
        for path in source.rglob("*"):
            name = path.name.lower()
            is_notice = any(word in name for word in ("license", "licence", "copying", "notice")) or name in ("ofl.txt", "ufl.txt") or ("fonts" in path.parts and path.suffix == ".txt")
            if path.is_file() and is_notice:
                found_text = True
                relative = path.relative_to(source)
                target = destination / f"{package['name']}-{package['version']}" / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(path, target)
        if not found_text and f"{package['name']}-{package['version']}" not in extra:
            missing_texts.append(f"{package['name']} {package['version']}")
    # A registry SPDX declaration is not a substitute for missing copyright texts.
    # These private feasibility bundles are not public distribution artifacts.
    (work / "notice-text-gaps.json").write_text(json.dumps(missing_texts, indent=2) + "\n")
    (destination / "notice-text-gaps.json").write_text(json.dumps(missing_texts, indent=2) + "\n")
    (destination / "inventory.json").write_text(json.dumps(inventory, indent=2) + "\n")
    return inventory, destination


def bundle_probe(work: Path, notice_directory: Path) -> dict[str, tuple[Path, Path]]:
    result = {}
    for format_name in ("clap", "vst3"):
        bundle = work / "artifacts" / f"DAWPhase0Probe.{format_name}"
        contents = bundle / "Contents"
        binary = contents / "MacOS" / "DAWPhase0Probe"
        binary.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(work / "target" / "release" / "libdrive_plugin_phase0.dylib", binary)
        (contents / "Info.plist").write_bytes(plistlib.dumps({
            "CFBundleExecutable": binary.name,
            "CFBundleIdentifier": "invalid.example.daw.phase0-probe." + format_name,
            "CFBundleName": "DAWPhase0Probe",
            "CFBundlePackageType": "BNDL",
            "CFBundleShortVersionString": "0.0.0",
            "CFBundleVersion": "0",
            "NSHighResolutionCapable": True,
        }))
        shutil.copytree(notice_directory, contents / "Resources" / "licenses", dirs_exist_ok=True)
        result[format_name] = (bundle, binary)
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work-dir", type=Path, default=ROOT / "work" / "drive-plugin-phase0")
    parser.add_argument("--fetch", action="store_true", help="Download exact pinned SDK/package sources")
    parser.add_argument("--gui", action="store_true", help="Briefly open native probe windows; requires a macOS desktop session")
    parser.add_argument("--require-framework-contract", action="store_true", help="Exit 2 if the candidate fails either additional production gate")
    args = parser.parse_args()
    if platform.system() != "Darwin" or platform.machine() != "arm64":
        parser.error("This Phase 0 experiment currently supports macOS arm64 only; cross-platform validation remains open")
    work = prepare_work_directory(args.work_dir)
    env = os.environ.copy()
    if "DEVELOPER_DIR" not in env and Path("/Applications/Xcode.app/Contents/Developer").is_dir():
        env["DEVELOPER_DIR"] = "/Applications/Xcode.app/Contents/Developer"
    prepare_sources(work, env, args.fetch)
    build = ["cargo", "build", "--locked", "--offline", "--release", "--manifest-path"]
    run(build + [str(TOOL / "Cargo.toml"), "--target-dir", str(work / "target")], work / "probe-build.log", env)
    # Freeze budgets and measure the direct implementation before candidate performance work.
    shutil.copy2(TOOL / "budgets.json", work / "budgets.json")
    baseline = json.loads(run([str(work / "target" / "release" / "drive-baseline")], work / "drive-baseline.json", env))
    budgets = json.loads((TOOL / "budgets.json").read_text())
    baseline_gates = assess_baseline(baseline, budgets)
    if not all(baseline_gates.values()):
        raise RuntimeError("Direct DSP baseline did not fit the predeclared tail/callback budget; retain measurements and investigate")
    inventory, notice_directory = notices(work, env)
    bundles = bundle_probe(work, notice_directory)
    sdk = work / "vst3sdk"
    run(["cmake", "-S", str(sdk), "-B", str(work / "vst3-build"), "-G", "Ninja", "-DCMAKE_BUILD_TYPE=Release", "-DCMAKE_CXX_COMPILER=/usr/bin/clang++", "-DCMAKE_C_COMPILER=/usr/bin/clang", "-DSMTG_ENABLE_VSTGUI_SUPPORT=OFF", "-DSMTG_ENABLE_VST3_PLUGIN_EXAMPLES=OFF", "-DSMTG_ENABLE_VST3_HOSTING_EXAMPLES=ON"], work / "vst3-configure.log", env)
    run(["cmake", "--build", str(work / "vst3-build"), "--target", "validator", "-j", "4"], work / "vst3-validator-build.log", env)
    run(build + [str(work / "clap-validator" / "Cargo.toml"), "-p", "clap-validator"], work / "clap-validator-build.log", env)
    host = work / "native-host"
    run(["xcrun", "clang++", "-std=c++17", "-fobjc-arc", "-O2", "-I" + str(work / "clap" / "include"), "-I" + str(sdk), str(TOOL / "native_host.mm"), str(sdk / "pluginterfaces/base/funknown.cpp"), str(sdk / "pluginterfaces/base/coreiids.cpp"), str(sdk / "public.sdk/source/vst/vstinitiids.cpp"), str(sdk / "public.sdk/source/common/commoniids.cpp"), "-framework", "Cocoa", "-o", str(host)], work / "native-host-build.log", env)
    results = {}
    for format_name, (bundle, binary) in bundles.items():
        command = [str(host), format_name, str(binary), str(bundle)]
        if args.gui:
            command.append("--gui")
        results[format_name] = json.loads(run(command, work / f"{format_name}-host.json", env))
    clap_result = json.loads(run([str(work / "clap-validator/target/release/clap-validator"), "validate", "--json", str(bundles["clap"][0])], work / "clap-validation.json", env))
    clap_codes = [item["status"]["code"] for item in clap_result["results"]]
    if set(clap_codes) - {"success", "skipped"}:
        raise RuntimeError("CLAP validator returned non-success/non-skipped results")
    vst_output = run([str(work / "vst3-build/bin/Release/validator"), str(bundles["vst3"][0])], work / "vst3-validation.log", env)
    match = re.search(r"Result: (\d+) tests passed, (\d+) tests failed", vst_output)
    if not match or int(match[2]):
        raise RuntimeError("VST3 validation did not report a passing test summary")
    hardware = {}
    for key in ["machdep.cpu.brand_string", "hw.ncpu", "hw.memsize"]:
        query = subprocess.run(["sysctl", "-n", key], text=True, capture_output=True)
        hardware[key] = query.stdout.strip() if query.returncode == 0 else "unavailable in this execution context"
    report = {"schema_version": 1, "hardware": hardware, "platform": platform.platform(), "architecture": platform.machine(),
              "rustc": run(["rustc", "--version"], work / "rustc.log", env).strip(),
              "clang": run(["xcrun", "clang++", "--version"], work / "clang.log", env).splitlines()[0],
              "sources": SOURCES, "dependency_count": len(inventory), "baseline": baseline, "baseline_gates": baseline_gates,
              "notice_text_gaps": json.loads((work / "notice-text-gaps.json").read_text()),
              "probes": results, "candidate_gates": assess_framework(results["vst3"]),
              "clap_validator": {"version": "0.4.1", "passed": clap_codes.count("success"), "skipped": clap_codes.count("skipped")},
              "vst3_validator": {"sdk_revision": SOURCES["vst3sdk"]["revision"], "passed": int(match[1]), "failed": int(match[2])},
              "native_editor_checked": args.gui, "external_daw_checked": False,
              "note": "Disposable gain probe; this is not a Drive plugin or release compatibility claim."}
    (work / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"report": str(work / "report.json"), **report["candidate_gates"]}, indent=2))
    return 2 if args.require_framework_contract and not report["candidate_gates"]["production_contract_satisfied"] else 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (RuntimeError, OSError, ValueError, subprocess.CalledProcessError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
