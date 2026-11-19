#!/usr/bin/env python3
"""Build and check the Drive CLAP development artifact. Never installs it."""
from __future__ import annotations
import argparse
from collections import Counter
import json
import os
from pathlib import Path
import platform
import plistlib
from plugin_bundle_layout import bundle_info, preview_binary
import shutil
import sys
from drive_artifact_check import build_artifact, check_artifact, collect_report
from drive_check_support import owned_scratch, run_logged
from package_plugins import copy_preview_notices, finalize_macos_bundle

ROOT = Path(__file__).resolve().parents[1]
FORBIDDEN = {"daw-desktop", "daw-cli", "daw-core", "daw-ui", "daw-engine", "daw-project", "daw-output", "daw-media", "daw-separation", "cpal", "rfd", "eframe"}
SIGNATURE = "DAW Drive CLAP Phase 2 scratch v1\n"

def scratch(path: Path) -> Path:
    return owned_scratch(
        path, ROOT, ".drive-clap-phase2", SIGNATURE,
        outside="Choose a dedicated scratch directory under ignored work/",
        unrecognized="Unrecognized scratch ownership marker",
    )


def run(command: list[str], log: Path, env: dict, allowed=(0,)) -> str:
    return run_logged(command, log, env, cwd=ROOT, allowed=allowed)


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
    artifact, tests, names, plugin_version = build_artifact(
        "clap", work, env, args.fetch, FORBIDDEN, run)
    bundle = work / "Drive.clap"
    system = platform.system()
    binary = preview_binary(bundle, "Drive", "clap", system)
    binary.parent.mkdir(parents=True, exist_ok=True)
    if system == "Darwin":
        info = bundle_info("Drive", "io.github.fecabrera.daw.drive", name="Drive", version=plugin_version)
        (bundle / "Contents/Info.plist").write_bytes(plistlib.dumps(info))
    notices = (work / "notices" if system != "Darwin" else bundle / "Contents/Resources/licenses")
    shutil.copy2(artifact, binary)
    copy_preview_notices(ROOT, notices, "clap", merge=True)
    (notices / "DEVELOPMENT.txt").write_text("Drive editor development artifact. Complete release dependency/notice audit, native accessibility, platform and external-host checks remain gates. Not a release package.\n")
    if platform.system() == "Darwin":
        finalize_macos_bundle(bundle, binary)
    check_artifact("clap", work, env, binary, tests, run)
    raw = json.loads(run([str(validator), "validate", "--json", str(bundle)], work / "validator.json", env, allowed=(0, 1)))
    assessment = assess_validator(raw["results"])
    report = collect_report(work, env, binary, names, plugin_version, version, assessment, run)
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
