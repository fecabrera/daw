"""Package already-built native Drive plugins. Does not install or sign them."""

import argparse
import hashlib
import json
from pathlib import Path
import plistlib
import shutil
import subprocess
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = {
    "aarch64-apple-darwin": ("macos", "arm64", "lib{}.dylib"),
    "x86_64-pc-windows-msvc": ("windows", "x64", "{}.dll"),
    "x86_64-unknown-linux-gnu": ("linux", "x64", "lib{}.so"),
}


def finalize_macos_bundle(bundle, binary):
    """Relocate and seal a complete development bundle, including its resources.

    Rust's linker signature covers the bare dylib, not the enclosing plugin bundle.
    Hosts such as Live reject that signature after the dylib is placed in a bundle.
    Ad-hoc signing supports local development; it is not Developer ID notarization.
    """
    subprocess.run(["install_name_tool", "-id",
                    f"@rpath/{bundle.name}/Contents/MacOS/{binary.name}", str(binary)], check=True)
    subprocess.run(["codesign", "--force", "--sign", "-", "--timestamp=none",
                    str(bundle)], check=True)
    subprocess.run(["codesign", "--verify", "--strict", str(bundle)], check=True)


def package(target, target_dir, output_dir, root=ROOT):
    """Read release libraries and publish one ZIP per format after staging succeeds."""
    # The checkers import finalization on Python 3.10; only packaging needs 3.11.
    import tomllib

    system, architecture, library_pattern = TARGETS[target]
    manifest = tomllib.loads((root / "plugins/drive/unit.toml").read_text(encoding="utf-8"))
    version = manifest["version"]
    output_dir.mkdir(parents=True, exist_ok=True)
    # Stage next to the destination so completed archives can replace it atomically.
    with tempfile.TemporaryDirectory(prefix=".drive-", dir=output_dir) as temporary:
        staging = Path(temporary)
        archives = []
        for format_name in ("clap", "vst3"):
            content = staging / format_name
            content.mkdir()
            bundle = content / f"Drive.{format_name}"
            if system == "macos":
                binary = bundle / "Contents/MacOS/Drive"
            elif format_name == "clap":
                binary = bundle
            else:
                folder, filename = (("x86_64-win", "Drive.vst3") if system == "windows"
                                    else ("x86_64-linux", "Drive.so"))
                binary = bundle / "Contents" / folder / filename
            binary.parent.mkdir(parents=True, exist_ok=True)
            source = target_dir / target / "release" / library_pattern.format(f"daw_drive_{format_name}")
            shutil.copy2(source, binary)
            # Preserve executable permissions in ZIPs, including when staged on Windows.
            binary.chmod(0o755)
            if system == "macos":
                identifier = manifest["clap_id"] + (".vst3" if format_name == "vst3" else "")
                (bundle / "Contents/Info.plist").write_bytes(plistlib.dumps({
                    "CFBundleExecutable": "Drive", "CFBundleIdentifier": identifier,
                    "CFBundleName": manifest["product"], "CFBundlePackageType": "BNDL",
                    "CFBundleVersion": version, "CFBundleShortVersionString": version,
                }))
            notices = (bundle / "Contents/Resources/licenses" if bundle.is_dir()
                       else content / "licenses")
            notices.mkdir(parents=True)
            shutil.copy2(root / "LICENSE", notices / "DAW-LICENSE.txt")
            for crate, name in ((f"plugin-{format_name}", format_name), ("equalizer", "dsp"),
                                ("oscillator", "oscillator-dsp"), ("plugin-window", "window"),
                                ("unit-ui", "fallback-fonts"), ("ui", "ui")):
                shutil.copytree(root / f"crates/{crate}/assets/licenses", notices / name)
            for source_name, destination in (("OFL.txt", "Outfit-OFL.txt"),
                                             ("Hack-LICENSE.txt", "Hack-LICENSE.txt")):
                shutil.copy2(root / "crates/unit-ui/assets/fonts" / source_name, notices / destination)
            if system == "macos":
                # Sign after the plist and every resource are final, before hashing/ZIP.
                finalize_macos_bundle(bundle, binary)
            metadata = {"product": manifest["product"], "vendor": manifest["vendor"],
                        "version": version, "target": target, "format": format_name,
                        "development_preview": True,
                        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest()}
            (content / "build.json").write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
            (content / "README.txt").write_text(
                "Drive development preview. macOS bundles are ad-hoc signed, not notarized.\n"
                "Windows/Linux binaries have no distribution signature.\n"
                "Extract the ZIP before use. Copy Drive." + format_name + " to a plugin search\n"
                "folder supported by your DAW, then rescan. Keep the licenses with it.\n"
                "To remove, close the DAW and remove that file or bundle.\n"
                "Logic Pro requires Audio Units; these CLAP/VST3 builds do not support it.\n"
                "Native accessibility, external-host/platform checks and a complete release\n"
                "dependency/notice audit remain open. This is not a release package.\n",
                encoding="utf-8")
            archive = staging / f"Drive-{version}-{system}-{architecture}-{format_name}.zip"
            with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as output:
                for path in sorted(content.rglob("*")):
                    if path.is_file():
                        entry = zipfile.ZipInfo.from_file(path, path.relative_to(content))
                        # Windows stat/chmod cannot represent POSIX executable bits.
                        # Write Unix ZIP attributes explicitly for portable extraction.
                        entry.create_system = 3
                        entry.external_attr = (0o100755 if path == binary else 0o100644) << 16
                        entry.compress_type = zipfile.ZIP_DEFLATED
                        output.writestr(entry, path.read_bytes())
            archives.append(archive)
        checksums = staging / f"Drive-{version}-{system}-{architecture}-SHA256SUMS.txt"
        checksums.write_text("".join(
            f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}\n" for path in archives
        ), encoding="utf-8")
        for path in [*archives, checksums]:
            destination = output_dir / path.name
            path.replace(destination)
            print(destination)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True, choices=TARGETS)
    parser.add_argument("--target-dir", type=Path, default=ROOT / "target")
    parser.add_argument("--output-dir", type=Path, default=ROOT / "target/plugin-artifacts")
    args = parser.parse_args()
    package(args.target, args.target_dir, args.output_dir)


if __name__ == "__main__":
    main()
