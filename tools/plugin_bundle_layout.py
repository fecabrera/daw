"""Paths and metadata for plugin packages and native test fixtures."""
import plistlib
import shutil


def library_filename(system, stem):
    if system == "Windows":
        return f"{stem}.dll"
    return f"lib{stem}" + (".dylib" if system == "Darwin" else ".so")


def bundle_binary(bundle, name, kind, system, machine, *, macos_executable=None):
    if system == "Darwin":
        return bundle / "Contents/MacOS" / (macos_executable or name)
    if system == "Windows":
        arch = "arm64" if machine.lower() in ("arm64", "aarch64") else "x86_64"
        return bundle / "Contents" / f"{arch}-win" / f"{name}.{kind}"
    return bundle / "Contents" / f"{machine}-linux" / f"{name}.so"


def preview_binary(bundle, name, kind, system):
    if kind == "clap" and system != "Darwin":
        return bundle
    return bundle_binary(bundle, name, kind, system, "x86_64")


def bundle_info(executable, identifier, *, name=None, version=None):
    info = {"CFBundleExecutable": executable, "CFBundleIdentifier": identifier,
            "CFBundlePackageType": "BNDL"}
    if name is not None:
        info["CFBundleName"] = name
    if version is not None:
        info.update(CFBundleVersion=version, CFBundleShortVersionString=version)
    return info


def stage_test_bundle(bundle, library, name, kind, system, machine, executable, identifier):
    binary = bundle_binary(bundle, name, kind, system, machine, macos_executable=executable)
    binary.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(library.resolve(strict=True), binary)
    if system == "Darwin":
        (bundle / "Contents/Info.plist").write_bytes(plistlib.dumps(bundle_info(executable, identifier)))
