#!/usr/bin/env python3
"""Build and check the Drive VST3 development artifact. Never installs it."""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import platform
import plistlib
from plugin_bundle_layout import bundle_info, preview_binary
import shutil
import re
import sys
from drive_artifact_check import build_artifact, check_artifact, collect_report
from drive_check_support import owned_scratch, run_logged
from package_plugins import copy_preview_notices, finalize_macos_bundle

ROOT = Path(__file__).resolve().parents[1]
FORBIDDEN = {"daw-desktop", "daw-cli", "daw-core", "daw-ui", "daw-engine", "daw-project", "daw-output", "daw-media", "daw-separation", "cpal", "rfd", "eframe", "clap-sys", "daw-plugin-clap", "daw-drive-clap"}
SIGNATURE = "DAW Drive VST3 Phase 3 scratch v1\n"

def scratch(path: Path) -> Path:
    return owned_scratch(
        path, ROOT, ".drive-vst3-phase3", SIGNATURE,
        outside="Choose a dedicated scratch directory under ignored work/",
        unrecognized="Unrecognized scratch ownership marker",
    )


def run(command: list[str], log: Path, env: dict, allowed=(0,)) -> str:
    return run_logged(command, log, env, cwd=ROOT, allowed=allowed, status=True)


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
    artifact, tests, names, plugin_version = build_artifact(
        "vst3", work, env, args.fetch, FORBIDDEN, run)
    bundle = work / "Drive.vst3"
    system = platform.system()
    binary = preview_binary(bundle, "Drive", "vst3", system)
    binary.parent.mkdir(parents=True, exist_ok=True)
    if system == "Darwin":
        info = bundle_info("Drive", "io.github.fecabrera.daw.drive.vst3", name="Drive", version=plugin_version)
        (bundle / "Contents/Info.plist").write_bytes(plistlib.dumps(info))
    notices = bundle / "Contents/Resources/licenses"
    shutil.copy2(artifact, binary)
    copy_preview_notices(ROOT, notices, "vst3", merge=True)
    (notices / "DEVELOPMENT.txt").write_text("Drive editor development artifact. Complete release dependency/notice audit, native accessibility, platform and external-host checks remain gates. Not a release package.\n")
    if platform.system() == "Darwin":
        finalize_macos_bundle(bundle, binary)
    check_artifact("vst3", work, env, binary, tests, run)
    raw = run([str(validator), str(bundle)], work / "validator.log", env, allowed=(0, 1, 255))
    assessment = assess_validator(raw, int((work / "validator.log.status").read_text()))
    report = collect_report(work, env, binary, names, plugin_version, version, assessment, run)
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
