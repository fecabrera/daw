"""Build and package native unit plugins from source. Does not install them; signs macOS bundles ad hoc."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import tempfile
from urllib.parse import urlsplit
import zipfile

from package_sources import revision
from plugin_bundle_layout import bundle_info, preview_binary

ROOT = Path(__file__).resolve().parents[1]
TARGETS = {
    "aarch64-apple-darwin": ("macos", "arm64", "lib{}.dylib"),
    "x86_64-pc-windows-msvc": ("windows", "x64", "{}.dll"),
    "x86_64-unknown-linux-gnu": ("linux", "x64", "lib{}.so"),
}


def copy_preview_notices(root, notices, format_name, *, merge=False):
    notices.mkdir(parents=True, exist_ok=merge)
    shutil.copy2(root / "LICENSE", notices / "DAW-LICENSE.txt")
    shutil.copy2(root / "NOTICE", notices / "DAW-NOTICE.txt")
    shutil.copy2(root / "crates/audio-analysis/LICENSE", notices / "DAW-BSD-2-Clause.txt")
    for crate, name in ((f"plugin-{format_name}", format_name), ("equalizer", "dsp"),
                        ("oscillator", "oscillator-dsp"), ("plugin-window", "window"),
                        ("unit-ui", "fallback-fonts"), ("ui", "ui")):
        shutil.copytree(root / f"crates/{crate}/assets/licenses", notices / name, dirs_exist_ok=merge)
    for source_name, destination in (("OFL.txt", "Outfit-OFL.txt"),
                                     ("Hack-LICENSE.txt", "Hack-LICENSE.txt")):
        shutil.copy2(root / "crates/unit-ui/assets/fonts" / source_name, notices / destination)


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


def available_units(root=ROOT):
    """Unit manifests register products; each product supplies native format targets."""
    return {path.parent.name: path for path in sorted((root / "plugins").glob("*/unit.toml"))}


def select_units(selection, root=ROOT):
    available = available_units(root)
    if selection.strip() == "all":
        if not available:
            raise ValueError("no plugin unit manifests found")
        return list(available)
    names = [name.strip() for name in selection.split(",")]
    if any(not name for name in names):
        raise ValueError("--units must be a comma-separated list of unit names or all")
    unknown = [name for name in names if name not in available]
    if unknown:
        raise ValueError(f"unknown units: {', '.join(unknown)}; available: {', '.join(available)}")
    return list(dict.fromkeys(names))


APIS = ("clap", "vst3", "auv2")

def supported_apis(target):
    return APIS if TARGETS[target][0] == "macos" else APIS[:2]



def select_apis(selection, target="aarch64-apple-darwin"):
    names = [name.strip() for name in selection.split(",")]
    if any(name not in (*APIS, "all") for name in names):
        raise ValueError("--api must contain clap, vst3, auv2 or all, separated by commas")
    if "auv2" in names and "auv2" not in supported_apis(target):
        raise ValueError("AUv2 exports require a macOS target")
    return [api for api in supported_apis(target) if "all" in names or api in names]


def target_metadata(root, unit, api):
    import tomllib
    path = root / "plugins" / f"{unit}-{api}" / "Cargo.toml"
    return tomllib.loads(path.read_text(encoding="utf-8"))


def build_plugins(target, target_dir, selected, apis, root, *, release):
    """Build exactly the selected format packages, using declared Cargo names."""
    if "auv2" in apis and "DAW_AUV2_VERIFY_SDK" in os.environ:
        raise ValueError("SDK verification candidates cannot be packaged; approve their provenance first")
    packages = [target_metadata(root, unit, api)["package"]["name"]
                for unit in selected for api in apis]
    cargo = shutil.which("cargo")
    if cargo is None:
        raise ValueError("Cargo is required to build plugins")
    command = [cargo, "build", "--locked", "--lib", "--manifest-path", str(root / "Cargo.toml"),
               "--target", target, "--target-dir", str(target_dir)]
    if release:
        command.append("--release")
    for name in dict.fromkeys(packages):
        command.extend(["-p", name])
    subprocess.run(command, cwd=root, check=True)


def stage_unit(target, target_dir, staging, unit, root, *, profile, apis, corresponding_source):
    """Assemble one product without replacing any published archives."""
    # The checkers import finalization on Python 3.10; only packaging needs 3.11.
    import tomllib

    system, architecture, library_pattern = TARGETS[target]
    manifest = tomllib.loads((root / f"plugins/{unit}/unit.toml").read_text(encoding="utf-8"))
    version = manifest["version"]
    product = manifest["product"]
    archive_name = product.replace(" ", "-")
    suffix = "-debug" if profile == "debug" else ""
    file_product = product + suffix
    archives = []
    for format_name in apis:
        content = staging / format_name
        content.mkdir()
        extension = "component" if format_name == "auv2" else format_name
        bundle = content / f"{file_product}.{extension}"
        native_system = {"macos": "Darwin", "windows": "Windows", "linux": "Linux"}[system]
        binary = preview_binary(bundle, file_product, format_name, native_system)
        binary.parent.mkdir(parents=True, exist_ok=True)
        cargo = target_metadata(root, unit, format_name)
        package_name = cargo["package"]["name"]
        library_name = cargo.get("lib", {}).get("name", package_name.replace("-", "_"))
        source = target_dir / target / profile / library_pattern.format(library_name)
        shutil.copy2(source, binary)
        # Preserve executable permissions in ZIPs, including when staged on Windows.
        binary.chmod(0o755)
        if system == "macos":
            identifier = manifest["clap_id"] + ("." + format_name if format_name != "clap" else "")
            info = bundle_info(file_product, identifier, name=product, version=version)
            if format_name == "auv2":
                au = manifest["auv2"]
                info["AudioComponents"] = [{"type": au["type"], "subtype": au["subtype"],
                    "manufacturer": au["manufacturer"], "name": f"{manifest['vendor']}: {product}",
                    "description": product, "factoryFunction": au["factory"],
                    "version": au["component_version"]}]
                info["LSMinimumSystemVersion"] = "12.0"
                info["CFBundleIdentifier"] = au["bundle_id"]
            (bundle / "Contents/Info.plist").write_bytes(plistlib.dumps(info))
        notices = (bundle / "Contents/Resources/licenses" if bundle.is_dir()
                   else content / "licenses")
        copy_preview_notices(root, notices, format_name)
        if system == "macos":
            # Sign after the plist and every resource are final, before hashing/ZIP.
            finalize_macos_bundle(bundle, binary)
        metadata = {"product": manifest["product"], "vendor": manifest["vendor"],
                    "version": version, "target": target, "format": format_name, "profile": profile,
                    "package": package_name, "license": "GPL-3.0-or-later",
                    "development_preview": True,
                    "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest()}
        if corresponding_source:
            metadata["corresponding_source"] = corresponding_source
            (content / "SOURCE-ACCESS.txt").write_text(
                f"Corresponding source for {product}\n"
                f"Build commit: {corresponding_source['revision']}\n"
                f"Source download: {corresponding_source['url']}\n"
                f"DAW-source.zip SHA-256: {corresponding_source['sha256']}\n\n"
                "Read DAW/SOURCE-BUILD.txt in the source archive. Keep the source\n"
                "available with the binary when redistributing this package.\n", encoding="utf-8")
        if format_name == "auv2":
            approved = json.loads((root / "crates/plugin-auv2/approved.json").read_text())
            metadata["binding_generation"] = {key: value for key, value in approved.items() if key != "headers"}
        (content / "build.json").write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
        (content / "README.txt").write_text(
            f"{product} development preview. macOS bundles are ad-hoc signed, not notarized.\n"
            "Windows/Linux binaries have no distribution signature.\n"
            f"Extract the ZIP before use. Copy {file_product}." + extension + " to a plugin search\n"
            "folder supported by your DAW, then rescan. Keep the licenses with it.\n"
            "To remove, close the DAW and remove that file or bundle.\n"
            + ("AUv2: use ~/Library/Audio/Plug-Ins/Components/ or /Library/Audio/Plug-Ins/Components/.\n"
               if format_name == "auv2" else "Logic Pro requires the separate AUv2 export.\n")
            + "Debug and release share identities; install one profile at a time.\n"
            + ("See SOURCE-ACCESS.txt for the corresponding source.\n" if corresponding_source else
               "Local build. Supply matching corresponding source when redistributing it.\n")
            + "Native accessibility, external-host/platform checks and a complete release\n"
            "dependency/notice audit remain open. This is not a release package.\n",
            encoding="utf-8")
        archive = staging / f"{archive_name}-{version}-{system}-{architecture}-{format_name}{suffix}.zip"
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
    # Subset runs must not replace checksums for a previously packaged full set.
    api_suffix = "" if tuple(apis) == supported_apis(target) else "-" + "-".join(apis)
    checksums = staging / f"{archive_name}-{version}-{system}-{architecture}{api_suffix}-SHA256SUMS{suffix}.txt"
    checksums.write_text("".join(
        f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}\n" for path in archives
    ), encoding="utf-8")
    return [*archives, checksums]


def package(target, target_dir, output_dir, root=ROOT, *, units, release=False, api="all",
            source_url=None, source_sha256=None):
    """Build first, then stage every selected unit before publishing artifacts."""
    if target not in TARGETS:
        raise ValueError(f"unsupported plugin target: {target}")
    root = Path(root).resolve()
    target_dir = Path(target_dir).resolve()
    output_dir = Path(output_dir).resolve()
    selected = select_units(units, root)
    apis = select_apis(api, target)
    corresponding_source = None
    if source_url is not None or source_sha256 is not None:
        url = urlsplit(source_url or "")
        if (url.scheme != "https" or not url.netloc or not source_sha256
                or any(c.isspace() for c in source_url)
                or len(source_sha256) != 64 or any(c not in "0123456789abcdef" for c in source_sha256)):
            raise ValueError("corresponding source requires an HTTPS URL and a SHA-256 digest")
        corresponding_source = {"revision": revision(root), "url": source_url, "sha256": source_sha256}
    build_plugins(target, target_dir, selected, apis, root, release=release)
    profile = "release" if release else "debug"
    output_dir.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".plugins-", dir=output_dir) as temporary:
        staging = Path(temporary)
        artifacts = []
        for unit in selected:
            unit_staging = staging / unit
            unit_staging.mkdir()
            artifacts.extend(stage_unit(target, target_dir, unit_staging, unit, root, profile=profile,
                                        apis=apis, corresponding_source=corresponding_source))
        names = [path.name for path in artifacts]
        if len(names) != len(set(names)):
            raise ValueError("selected units produce conflicting archive names")
        if corresponding_source and revision(root) != corresponding_source["revision"]:
            raise RuntimeError("tracked sources changed during plugin packaging")
        # Stage a rollback copy of every overwritten file before publication.
        backup = staging / "previous"
        backup.mkdir()
        published = []
        try:
            for path in artifacts:
                destination = output_dir / path.name
                old = backup / path.name
                if destination.exists():
                    shutil.copy2(destination, old)
                path.replace(destination)
                published.append((destination, old if old.exists() else None))
        except OSError:
            for destination, old in reversed(published):
                if old is None:
                    destination.unlink(missing_ok=True)
                else:
                    old.replace(destination)
            raise
        for destination, _ in published:
            print(destination)


def main():
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--units", required=True, help="Comma-separated unit names (for example drive,drive-v2), or all")
    parser.add_argument("--release", action="store_true", help="Build optimized plugins (default: debug)")
    parser.add_argument("--api", default="all", help="Comma-separated clap, vst3, auv2 or all (default: all)")
    parser.add_argument("--target", required=True, choices=TARGETS)
    parser.add_argument("--target-dir", type=Path, default=ROOT / "target")
    parser.add_argument("--output-dir", type=Path, default=ROOT / "target/plugin-artifacts")
    parser.add_argument("--source-url", help="HTTPS download URL for the matching corresponding source")
    parser.add_argument("--source-sha256", help="SHA-256 of the matching DAW-source.zip")
    args = parser.parse_args()
    try:
        package(args.target, args.target_dir, args.output_dir, units=args.units, release=args.release,
                api=args.api, source_url=args.source_url, source_sha256=args.source_sha256)
    except (ValueError, RuntimeError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Packaging failed: {error}\n")


if __name__ == "__main__":
    main()
