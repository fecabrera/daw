"""Write a CI job summary and link to command logs, including failed jobs."""

import json
import os
from pathlib import Path
import re


def main():
    steps = json.loads(os.environ.get("CI_STEP_RESULTS", "{}"))
    labels = {
        "checkout": "Checkout",
        "setup": "Native build setup",
        "formatting": "Formatting",
        "clippy": "Clippy",
        "tests": "Workspace tests",
        "cli": "Headless CLI",
        "windows_package": "Windows build and package",
        "macos_package": "macOS build and package",
        "linux_package": "Linux build and package",
        "windows_artifact": "Windows artifact upload",
        "macos_artifact": "macOS artifact upload",
        "linux_artifact": "Linux artifact upload",
        "plugin_packaging_tests": "Plugin packaging tests",
        "plugin_build": "Plugin release build",
        "plugin_tests": "Native plugin tests",
        "plugin_package": "Plugin preview packaging",
        "clap_artifact": "CLAP artifact upload",
        "vst3_artifact": "VST3 artifact upload",
        "plugin_checksums": "Plugin checksum upload",
        "logs": "Log upload",
    }
    lines = [
        f"## {os.environ.get('CI_JOB_LABEL', 'CI job')}",
        f"Result: {os.environ.get('CI_JOB_STATUS', 'unknown')}",
        "", "| Step | Result |", "| --- | --- |",
    ]
    for step, label in labels.items():
        if step in steps:
            lines.append(f"| {label} | {steps[step].get('outcome', 'unknown')} |")
    artifact_url = steps.get("logs", {}).get("outputs", {}).get("artifact-url")
    if artifact_url:
        lines.extend(["", f"[Download command logs]({artifact_url})"])
    lines.extend(["", f"Raw runner logs: `gh run view {os.environ.get('GITHUB_RUN_ID', 'RUN_ID')} --log`."])

    if any(step.get("outcome") == "failure" for step in steps.values()):
        # Keep the summary small; the artifact retains the complete command output.
        logs = sorted(Path("work/ci-logs").glob("*.log"), key=lambda path: path.stat().st_mtime)
        if logs:
            log = logs[-1]
            text = log.read_text(encoding="utf-8-sig", errors="replace")
            text = re.sub(r"(?:\x1b|\^\[)\[[0-9;]*[a-zA-Z]", "", text)
            tail = "\n".join(text.splitlines()[-30:])[-12000:]
            lines.extend(["", f"Last command output ({log.name}):", ""])
            lines.extend("    " + line for line in tail.splitlines())
    summary = "\n".join(lines) + "\n"
    destination = os.environ.get("GITHUB_STEP_SUMMARY")
    if destination:
        with Path(destination).open("a", encoding="utf-8") as output:
            output.write(summary)
    else:
        print(summary, end="")


if __name__ == "__main__":
    main()
