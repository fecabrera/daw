#!/usr/bin/env python3
"""Inspect the repository's own Drive exports in disposable native bundles."""
import argparse
import json
from pathlib import Path
import platform
import subprocess
import tempfile

from plugin_bundle_layout import library_filename, stage_test_bundle


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scanner", type=Path, default=Path("target/debug/daw-plugin-scan" + (".exe" if platform.system() == "Windows" else "")))
    parser.add_argument("--embedded", action="store_true", help="Use desktop or CLI internal scanner entry point")
    parser.add_argument("--clap-library", type=Path)
    parser.add_argument("--vst3-library", type=Path)
    parser.add_argument("--editor-check", type=Path, help="Owned native editor lifecycle check")
    parser.add_argument("--graph-check", type=Path, help="Optional owned-plugin engine graph check executable")
    parser.add_argument("--host-check", type=Path, help="Native processing check executable, built with cargo build -p daw-plugin-host --example native_host_check")
    args = parser.parse_args()
    system = platform.system()
    libraries = {
        "clap": args.clap_library or Path("target/debug") / library_filename(system, "daw_drive_clap"),
        "vst3": args.vst3_library or Path("target/debug") / library_filename(system, "daw_drive_vst3"),
    }
    scanner = args.scanner.resolve(strict=True)
    command = [str(scanner)] + (["--daw-plugin-scan"] if args.embedded else [])
    with tempfile.TemporaryDirectory(prefix="daw-scanner-check-") as directory:
        root = Path(directory)
        for kind, library in libraries.items():
            bundle = root / f"Drive.{kind}"
            stage_test_bundle(bundle, library, "Drive", kind, system, platform.machine(),
                              "DrivePayload", f"org.daw.scanner-check.{kind}")
            result = root / f"{kind}.json"
            subprocess.run(command + ["--format", kind, "--plugin", str(bundle), "--result", str(result)], check=True, timeout=12)
            response = json.loads(result.read_text())
            assert response["version"] == 1, response
            records = response["plugins"]
            assert len(records) == 1, records
            record = records[0]
            assert record["name"] == "Drive" and record["role"] == "effect", record
            expected = "io.github.fecabrera.daw.drive" if kind == "clap" else [0xF3, 0x99, 0xB9, 0x2B, 0x5D, 0x26, 0x4C, 0x7D, 0x98, 0xC6, 0xDE, 0x3C, 0x26, 0x54, 0xD5, 0x34]
            assert record["identity"] == {"format": kind, "id": expected}, record
            print(f"PASS: {kind.upper()} native module entry, factory enumeration, identity and unload", flush=True)
            if args.editor_check:
                subprocess.run([str(args.editor_check.resolve(strict=True)), kind, str(bundle)], check=True, timeout=60)
            if args.graph_check:
                subprocess.run([str(args.graph_check.resolve(strict=True)), kind, str(bundle)], check=True, timeout=120)
            if args.host_check:
                subprocess.run([str(args.host_check.resolve(strict=True)), kind, str(bundle)], check=True, timeout=60)
        damaged = root / "Damaged.clap"
        damaged.write_bytes(b"not an executable")
        result = root / "damaged.json"
        failure = subprocess.run(command + ["--format", "clap", "--plugin", str(damaged), "--result", str(result)], capture_output=True, timeout=12)
        assert failure.returncode != 0 and not result.exists()
        print("PASS: damaged native module fails without publishing metadata")


if __name__ == "__main__":
    main()
