"""Build the native desktop executable and report Cargo's verified artifact path."""
import argparse
import json
from pathlib import Path
import subprocess
import sys


def build(profile: str, platform: str) -> Path:
    root = Path(__file__).resolve().parent.parent
    version = subprocess.check_output(["rustc", "-vV"], cwd=root, text=True)
    hosts = [line.removeprefix("host: ") for line in version.splitlines() if line.startswith("host: ")]
    if len(hosts) != 1:
        raise ValueError("Cannot identify the native Rust target")
    host = hosts[0]
    if not ((platform == "macos" and host.endswith("-apple-darwin")) or
            (platform == "linux" and "-linux-" in host)):
        raise ValueError(f"The {platform} packager requires a matching native Rust toolchain")
    if host.split("-")[0] not in ("aarch64", "x86_64"):
        raise ValueError(f"Unsupported desktop target: {host}")
    # An explicit target overrides both CARGO_BUILD_TARGET and Cargo build.target.
    command = ["cargo", "build", "--locked", "--target-dir", str(root / "target"),
               "--target", host, "-p", "daw-desktop", "--message-format=json-render-diagnostics"]
    if profile == "release":
        command.append("--release")
    artifacts = []
    with subprocess.Popen(command, cwd=root, stdout=subprocess.PIPE, text=True) as process:
        for line in process.stdout:
            message = json.loads(line)
            if message.get("reason") == "compiler-message":
                rendered = message.get("message", {}).get("rendered")
                if rendered:
                    print(rendered, end="", file=sys.stderr)
            if (message.get("reason") == "compiler-artifact" and
                    message.get("target", {}).get("name") == "daw-desktop" and
                    "bin" in message.get("target", {}).get("kind", []) and
                    message.get("executable")):
                artifacts.append(Path(message["executable"]))
        if process.wait() != 0:
            raise ValueError("Cargo build failed; the existing package was not changed")
    expected = root / "target" / host / profile / "daw-desktop"
    if len(artifacts) != 1 or artifacts[0].absolute() != expected or not expected.is_file():
        raise ValueError("Cargo did not report the expected native daw-desktop executable")
    return expected


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("profile", choices=("debug", "release"))
    parser.add_argument("platform", choices=("macos", "linux"))
    args = parser.parse_args()
    try:
        print(build(args.profile, args.platform))
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"Desktop build: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
