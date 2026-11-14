"""Check deliverable layouts, notice retention, and packaging failure behavior."""

import contextlib
import hashlib
import io
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile

from package_plugins import (ROOT, TARGETS, finalize_macos_bundle, package as build_and_package,
                             select_units, available_units, select_apis, build_plugins)


def package(*args, **kwargs):
    """Legacy layout fixtures supply prebuilt release bytes, not Cargo projects."""
    kwargs.setdefault("release", True)
    with patch("package_plugins.build_plugins"):
        return build_and_package(*args, **kwargs)


def fake_cargo(base):
    """A child-process fixture keeps CLI tests independent of native toolchains."""
    directory = base / "fake-bin"
    directory.mkdir()
    log = base / "cargo-calls.jsonl"
    script = directory / "cargo_fixture.py"
    script.write_text("import json,sys\nfrom pathlib import Path\n"
                      f"with Path({str(log)!r}).open('a') as output: output.write(json.dumps(sys.argv[1:])+'\\n')\n")
    if sys.platform == "win32":
        (directory / "cargo.bat").write_text(f'@"{sys.executable}" "{script}" %*\n')
    else:
        command = directory / "cargo"
        command.write_text(f'#!{sys.executable}\n' + script.read_text())
        command.chmod(0o755)
    return dict(os.environ, PATH=str(directory) + os.pathsep + os.environ.get("PATH", "")), log



class PackagingTests(unittest.TestCase):
    def test_checker_imports_do_not_require_python311_tomllib(self):
        subprocess.run([sys.executable, "-c",
                        "import sys; sys.modules['tomllib'] = None; "
                        "import check_drive_clap; import check_drive_vst3"],
                       cwd=ROOT / "tools", check=True, capture_output=True)

    @unittest.skipUnless(shutil.which("git"), "requires Git checkout support")
    def test_runtime_notice_bytes_survive_crlf_checkout(self):
        inventory = json.loads((ROOT / "tools/runtime_notices.json").read_text())
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            checkout = base / "checkout"
            checkout.mkdir()
            (checkout / ".gitattributes").write_bytes((ROOT / ".gitattributes").read_bytes())
            paths = [".gitattributes"]
            for entry in inventory["notices"]:
                target = checkout / entry["notice"]
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes((ROOT / entry["notice"]).read_bytes())
                paths.append(entry["notice"])
            # Exercise actual Git conversion without a commit or a Windows host.
            git = ["git", "-c", "core.autocrlf=true", "-c", "core.eol=crlf",
                   "-c", "core.safecrlf=false"]
            def run(*args):
                subprocess.run(git + list(args), cwd=checkout, check=True, capture_output=True)
            run("init", "--quiet", "--template=")
            run("add", "--", *paths)
            exported = base / "exported"
            run("checkout-index", "--all", "--prefix=" + exported.as_posix() + "/")
            for entry in inventory["notices"]:
                with self.subTest(notice=entry["notice"]):
                    actual = (exported / entry["notice"]).read_bytes()
                    self.assertEqual(actual, (ROOT / entry["notice"]).read_bytes())
                    self.assertEqual(hashlib.sha256(actual).hexdigest(), entry["sha256"])

    def test_platform_archives(self):
        for target, (system, architecture, pattern) in TARGETS.items():
            with self.subTest(target=target), tempfile.TemporaryDirectory() as temporary:
                base = Path(temporary)
                release = base / "target" / target / "release"
                release.mkdir(parents=True)
                for format_name in ("clap", "vst3"):
                    (release / pattern.format(f"daw_drive_{format_name}")).write_bytes(format_name.encode())
                output = base / "artifacts"
                def sign_fixture(bundle, binary):
                    self.assertTrue((bundle / "Contents/Resources/licenses/DAW-LICENSE.txt").is_file())
                    binary.write_bytes(binary.read_bytes() + b"_signed")
                    seal = bundle / "Contents/_CodeSignature/CodeResources"
                    seal.parent.mkdir()
                    seal.write_bytes(b"fixture resource seal")
                with contextlib.redirect_stdout(io.StringIO()), patch(
                    "package_plugins.finalize_macos_bundle", side_effect=sign_fixture
                ):
                    package(target, base / "target", output, units="drive")
                archives = sorted(output.glob("*.zip"))
                self.assertEqual(len(archives), 2)
                for archive in archives:
                    format_name = archive.stem.rsplit("-", 1)[1]
                    with zipfile.ZipFile(archive) as zipped:
                        if system == "macos":
                            binary = f"Drive.{format_name}/Contents/MacOS/Drive"
                            info = plistlib.loads(zipped.read(f"Drive.{format_name}/Contents/Info.plist"))
                            self.assertEqual(info["CFBundleExecutable"], "Drive")
                            identifier = "io.github.fecabrera.daw.drive" + (".vst3" if format_name == "vst3" else "")
                            self.assertEqual(info["CFBundleIdentifier"], identifier)
                        elif format_name == "clap":
                            binary = "Drive.clap"
                        elif system == "windows":
                            binary = "Drive.vst3/Contents/x86_64-win/Drive.vst3"
                        else:
                            binary = "Drive.vst3/Contents/x86_64-linux/Drive.so"
                        payload = zipped.read(binary)
                        self.assertEqual(payload, format_name.encode() + (b"_signed" if system == "macos" else b""))
                        self.assertTrue(zipped.getinfo(binary).external_attr >> 16 & 0o111)
                        metadata = json.loads(zipped.read("build.json"))
                        self.assertEqual(metadata["target"], target)
                        self.assertEqual(metadata["binary_sha256"], hashlib.sha256(payload).hexdigest())
                        self.assertTrue(metadata["development_preview"])
                        license_prefix = (f"Drive.{format_name}/Contents/Resources/licenses/"
                                          if "/" in binary else "licenses/")
                        self.assertEqual(zipped.read(license_prefix + "DAW-LICENSE.txt"), (ROOT / "LICENSE").read_bytes())
                        for notice in ("Outfit-OFL.txt", "Hack-LICENSE.txt", "window/baseview-LICENSE-MIT.txt",
                                       "fallback-fonts/egui-fonts-Hack-Regular.txt"):
                            self.assertIn(license_prefix + notice, zipped.namelist())
                        inventory = json.loads((ROOT / "tools/runtime_notices.json").read_text())
                        for entry in inventory["notices"]:
                            source = ROOT / entry["notice"]
                            if "plugins" not in entry["deliverables"]:
                                continue
                            expected = source.read_bytes()
                            self.assertEqual(hashlib.sha256(expected).hexdigest(), entry["sha256"], entry["notice"])
                            self.assertEqual(zipped.read(license_prefix + "ui/" + source.name), expected)
                        self.assertIn("README.txt", zipped.namelist())
                        if system == "macos":
                            self.assertIn(f"Drive.{format_name}/Contents/_CodeSignature/CodeResources", zipped.namelist())
                    self.assertEqual((release / pattern.format(f"daw_drive_{format_name}")).read_bytes(), format_name.encode())
                    self.assertIn(f"-{system}-{architecture}-", archive.name)
                checksums = next(output.glob("*SHA256SUMS.txt")).read_text()
                for archive in archives:
                    self.assertIn(f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n", checksums)
                self.assertFalse(list(output.glob(".plugins-*")))

    def test_drive_v2_platform_archives(self):
        for target, (system, architecture, pattern) in TARGETS.items():
            with self.subTest(target=target), tempfile.TemporaryDirectory() as temporary:
                base = Path(temporary)
                release = base / "target" / target / "release"
                release.mkdir(parents=True)
                for format_name in ("clap", "vst3"):
                    (release / pattern.format(f"daw_drive_v2_{format_name}")).write_bytes(format_name.encode())
                output = base / "artifacts"
                def sign_fixture(bundle, binary):
                    self.assertTrue((bundle / "Contents/Resources/licenses/DAW-LICENSE.txt").is_file())
                    binary.write_bytes(binary.read_bytes() + b"_signed")
                    seal = bundle / "Contents/_CodeSignature/CodeResources"
                    seal.parent.mkdir()
                    seal.write_bytes(b"fixture resource seal")
                with contextlib.redirect_stdout(io.StringIO()), patch(
                    "package_plugins.finalize_macos_bundle", side_effect=sign_fixture
                ):
                    package(target, base / "target", output, units="drive-v2")
                archives = sorted(output.glob("*.zip"))
                self.assertEqual(len(archives), 2)
                for archive in archives:
                    format_name = archive.stem.rsplit("-", 1)[1]
                    with zipfile.ZipFile(archive) as zipped:
                        if system == "macos":
                            binary = f"Drive v2.{format_name}/Contents/MacOS/Drive v2"
                            info = plistlib.loads(zipped.read(f"Drive v2.{format_name}/Contents/Info.plist"))
                            self.assertEqual(info["CFBundleExecutable"], "Drive v2")
                            identifier = "io.github.fecabrera.daw.drive-v2" + (".vst3" if format_name == "vst3" else "")
                            self.assertEqual(info["CFBundleIdentifier"], identifier)
                        elif format_name == "clap":
                            binary = "Drive v2.clap"
                        elif system == "windows":
                            binary = "Drive v2.vst3/Contents/x86_64-win/Drive v2.vst3"
                        else:
                            binary = "Drive v2.vst3/Contents/x86_64-linux/Drive v2.so"
                        payload = zipped.read(binary)
                        self.assertEqual(payload, format_name.encode() + (b"_signed" if system == "macos" else b""))
                        self.assertTrue(zipped.getinfo(binary).external_attr >> 16 & 0o111)
                        metadata = json.loads(zipped.read("build.json"))
                        self.assertEqual(metadata["target"], target)
                        self.assertEqual(metadata["binary_sha256"], hashlib.sha256(payload).hexdigest())
                        self.assertTrue(metadata["development_preview"])
                        license_prefix = (f"Drive v2.{format_name}/Contents/Resources/licenses/"
                                          if "/" in binary else "licenses/")
                        self.assertEqual(zipped.read(license_prefix + "DAW-LICENSE.txt"), (ROOT / "LICENSE").read_bytes())
                        for notice in ("Outfit-OFL.txt", "Hack-LICENSE.txt", "window/baseview-LICENSE-MIT.txt",
                                       "fallback-fonts/egui-fonts-Hack-Regular.txt"):
                            self.assertIn(license_prefix + notice, zipped.namelist())
                        self.assertIn("README.txt", zipped.namelist())
                        if system == "macos":
                            self.assertIn(f"Drive v2.{format_name}/Contents/_CodeSignature/CodeResources", zipped.namelist())
                    self.assertEqual((release / pattern.format(f"daw_drive_v2_{format_name}")).read_bytes(), format_name.encode())
                    self.assertIn(f"-{system}-{architecture}-", archive.name)
                checksums = next(output.glob("*SHA256SUMS.txt")).read_text()
                for archive in archives:
                    self.assertIn(f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n", checksums)
                self.assertFalse(list(output.glob(".plugins-*")))

    def test_every_registered_product_has_both_targets_and_a_ci_entry(self):
        import re
        workflow = (ROOT / ".github/workflows/plugins.yml").read_text()
        matrix = set(re.findall(r"slug: ([a-z0-9-]+)", workflow))
        self.assertEqual(matrix, set(available_units()))
        for unit in available_units():
            for fmt in ("clap", "vst3"):
                self.assertTrue((ROOT / f"plugins/{unit}-{fmt}/Cargo.toml").is_file())

    def test_registered_products_have_distinct_public_identities(self):
        import tomllib
        identities = []
        for path in sorted((ROOT / "plugins").glob("*/unit.toml")):
            manifest = tomllib.loads(path.read_text())
            identities.extend(manifest[key] for key in ("clap_id", "processor_id", "controller_id"))
        self.assertEqual(len(identities), len(set(identities)))

    def test_selection_rejects_invalid_names_and_deduplicates(self):
        self.assertEqual(select_units(" drive-v2,drive,drive-v2 "), ["drive-v2", "drive"])
        self.assertEqual(select_units("all"), sorted(available_units()))
        self.assertEqual(len(select_units("all")),20)
        for value in ("", "drive,", ",drive", "drive,,drive-v2", "unknown", "all,drive", "../drive"):
            with self.subTest(value=value), self.assertRaises(ValueError):
                select_units(value)

    def test_multiple_units_and_all_package_both_products_on_each_platform(self):
        for target, (_, _, pattern) in TARGETS.items():
            for selection in ("drive,drive-v2", "saturator,bitcrusher", "all"):
                with self.subTest(target=target, selection=selection), tempfile.TemporaryDirectory() as temporary:
                    base = Path(temporary)
                    release = base / "target" / target / "release"
                    release.mkdir(parents=True)
                    for unit in (slug.replace("-","_") for slug in select_units("all")):
                        for fmt in ("clap", "vst3"):
                            (release / pattern.format(f"daw_{unit}_{fmt}")).write_bytes(f"{unit}:{fmt}".encode())
                    output = base / "artifacts"
                    with contextlib.redirect_stdout(io.StringIO()), patch("package_plugins.finalize_macos_bundle"):
                        package(target, base / "target", output, units=selection)
                    self.assertEqual(len(list(output.glob("*.zip"))), 2 * len(select_units(selection)))
                    self.assertEqual(len(list(output.glob("*SHA256SUMS.txt"))), len(select_units(selection)))
                    for archive in output.glob("*.zip"):
                        with zipfile.ZipFile(archive) as zipped:
                            metadata = json.loads(zipped.read("build.json"))
                            self.assertTrue(metadata["product"])
                        checksum = next(output.glob((metadata["product"].replace(" ", "-") + "-0.1.0-") + "*SHA256SUMS.txt")).read_text()
                        self.assertIn(f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}", checksum)
                    self.assertFalse(list(output.glob(".plugins-*")))

    def test_later_unit_failure_preserves_all_existing_products(self):
        for failure_kind in ("missing_library", "signing"):
            with self.subTest(failure=failure_kind), tempfile.TemporaryDirectory() as temporary:
                base = Path(temporary)
                target = "aarch64-apple-darwin"
                release = base / "target" / target / "release"
                release.mkdir(parents=True)
                for unit in (slug.replace("-","_") for slug in select_units("all")):
                    for fmt in ("clap", "vst3"):
                        (release / f"libdaw_{unit}_{fmt}.dylib").write_bytes(b"old")
                output = base / "artifacts"
                with contextlib.redirect_stdout(io.StringIO()), patch("package_plugins.finalize_macos_bundle"):
                    package(target, base / "target", output, units="all")
                previous = {p.name: p.read_bytes() for p in output.iterdir()}
                (release / "libdaw_drive_clap.dylib").write_bytes(b"new")
                if failure_kind == "missing_library":
                    (release / "libdaw_drive_v2_vst3.dylib").unlink()
                    effects = None
                    error = FileNotFoundError
                else:
                    effects = [None, None, None, subprocess.CalledProcessError(1, ["codesign"])]
                    error = subprocess.CalledProcessError
                with patch("package_plugins.finalize_macos_bundle", side_effect=effects), self.assertRaises(error):
                    package(target, base / "target", output, units="all")
                self.assertEqual({p.name: p.read_bytes() for p in output.iterdir()}, previous)

    def test_new_manifest_and_custom_library_names_need_no_packager_changes(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            root = base / "repo"
            unit = root / "plugins/example"
            unit.mkdir(parents=True)
            (unit / "unit.toml").write_text('product = "Example"\nversion = "1.2.3"\nvendor = "Test"\nclap_id = "org.example.effect"\n')
            target = "x86_64-unknown-linux-gnu"
            release = base / "target" / target / "release"
            release.mkdir(parents=True)
            for fmt in ("clap", "vst3"):
                manifest = root / f"plugins/example-{fmt}/Cargo.toml"
                manifest.parent.mkdir()
                manifest.write_text(f'[package]\nname = "other-{fmt}"\n[lib]\nname = "custom_{fmt}"\n')
                (release / f"libcustom_{fmt}.so").write_bytes(fmt.encode())
            shutil.copy2(ROOT / "LICENSE", root / "LICENSE")
            for crate in ("plugin-clap", "plugin-vst3", "equalizer", "oscillator", "plugin-window", "unit-ui", "ui"):
                shutil.copytree(ROOT / f"crates/{crate}/assets/licenses", root / f"crates/{crate}/assets/licenses")
            shutil.copytree(ROOT / "crates/unit-ui/assets/fonts", root / "crates/unit-ui/assets/fonts")
            output = base / "artifacts"
            with contextlib.redirect_stdout(io.StringIO()):
                package(target, base / "target", output, root=root, units="all")
            self.assertEqual(select_units("all", root), ["example"])
            self.assertEqual(len(list(output.glob("Example-1.2.3-*.zip"))), 2)
            with zipfile.ZipFile(output / "Example-1.2.3-linux-x64-clap.zip") as zipped:
                self.assertEqual(zipped.read("Example.clap"), b"clap")
            with zipfile.ZipFile(output / "Example-1.2.3-linux-x64-vst3.zip") as zipped:
                self.assertEqual(zipped.read("Example.vst3/Contents/x86_64-linux/Example.so"), b"vst3")

    def test_cli_accepts_lists_and_all_and_reports_unknown_units(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            target = "x86_64-unknown-linux-gnu"
            release = base / "target" / target / "release"
            release.mkdir(parents=True)
            for unit in (slug.replace("-","_") for slug in select_units("all")):
                for fmt in ("clap", "vst3"):
                    (release / f"libdaw_{unit}_{fmt}.so").write_bytes(b"fixture")
            env, calls = fake_cargo(base)
            command = [sys.executable, str(ROOT / "tools/package_plugins.py"), "--release", "--target", target,
                       "--target-dir", str(base / "target")]
            for selection in ("drive,drive-v2", "saturator,bitcrusher", "all"):
                output = base / selection
                result = subprocess.run(command + ["--units", selection, "--output-dir", str(output)], capture_output=True, text=True, env=env)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(len(list(output.glob("*.zip"))), 2 * len(select_units(selection)))
            output = base / "invalid"
            result = subprocess.run(command + ["--units", "drive,unknown", "--output-dir", str(output)], capture_output=True, text=True, env=env)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("unknown units", result.stderr)
            self.assertNotIn("Traceback", result.stderr)
            self.assertFalse(output.exists())

    def test_macos_finalization_relocates_before_signing_complete_bundle(self):
        bundle = Path("Drive.vst3")
        binary = bundle / "Contents/MacOS/Drive"
        with patch("package_plugins.subprocess.run") as run:
            finalize_macos_bundle(bundle, binary)
        commands = [call.args[0] for call in run.call_args_list]
        self.assertEqual(commands, [
            ["install_name_tool", "-id", "@rpath/Drive.vst3/Contents/MacOS/Drive", str(binary)],
            ["codesign", "--force", "--sign", "-", "--timestamp=none", str(bundle)],
            ["codesign", "--verify", "--strict", str(bundle)],
        ])
        self.assertTrue(all(call.kwargs["check"] for call in run.call_args_list))

    def test_failed_signing_preserves_existing_archives(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            target = "aarch64-apple-darwin"
            release = base / "target" / target / "release"
            release.mkdir(parents=True)
            for format_name in ("clap", "vst3"):
                (release / f"libdaw_drive_{format_name}.dylib").write_bytes(format_name.encode())
            output = base / "artifacts"
            with contextlib.redirect_stdout(io.StringIO()), patch("package_plugins.finalize_macos_bundle"):
                package(target, base / "target", output, units="drive")
            before = {path.name: path.read_bytes() for path in output.iterdir()}
            failure = subprocess.CalledProcessError(1, ["codesign"])
            with patch("package_plugins.finalize_macos_bundle", side_effect=[None, failure]):
                with self.assertRaises(subprocess.CalledProcessError):
                    package(target, base / "target", output, units="drive")
            self.assertEqual({path.name: path.read_bytes() for path in output.iterdir()}, before)

    @unittest.skipUnless(sys.platform == "darwin", "requires native Mach-O tools")
    def test_native_signature_survives_zip_and_seals_resources(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            target = "aarch64-apple-darwin"
            release = base / "target" / target / "release"
            release.mkdir(parents=True)
            source = base / "fixture.c"
            source.write_text("int drive_packaging_fixture(void) { return 1; }\n")
            library = release / "libdaw_drive_clap.dylib"
            subprocess.run(["clang", "-dynamiclib", str(source), "-o", str(library)], check=True)
            shutil.copy2(library, release / "libdaw_drive_vst3.dylib")
            original = library.read_bytes()
            output = base / "artifacts"
            with contextlib.redirect_stdout(io.StringIO()):
                package(target, base / "target", output, units="drive")
            self.assertEqual(library.read_bytes(), original)
            for archive in output.glob("*.zip"):
                extracted = base / archive.stem
                with zipfile.ZipFile(archive) as zipped:
                    zipped.extractall(extracted)
                    for entry in zipped.infolist():
                        (extracted / entry.filename).chmod(entry.external_attr >> 16 & 0o777)
                bundle = next(extracted.glob("Drive.*"))
                binary = bundle / "Contents/MacOS/Drive"
                subprocess.run(["codesign", "--verify", "--strict", str(bundle)], check=True)
                identifier = subprocess.check_output(["otool", "-D", str(binary)], text=True)
                self.assertIn(f"@rpath/{bundle.name}/Contents/MacOS/Drive", identifier)
                (bundle / "Contents/Resources/licenses/DAW-LICENSE.txt").write_text("tampered")
                result = subprocess.run(["codesign", "--verify", "--strict", str(bundle)], capture_output=True)
                self.assertNotEqual(result.returncode, 0)

    def test_missing_second_library_preserves_existing_artifacts(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            target = "x86_64-unknown-linux-gnu"
            release = base / "target" / target / "release"
            release.mkdir(parents=True)
            (release / "libdaw_drive_clap.so").write_bytes(b"clap")
            vst3 = release / "libdaw_drive_vst3.so"
            vst3.write_bytes(b"vst3")
            output = base / "artifacts"
            with contextlib.redirect_stdout(io.StringIO()):
                package(target, base / "target", output, units="drive")
            previous = {path.name: path.read_bytes() for path in output.iterdir()}
            vst3.unlink()
            (release / "libdaw_drive_clap.so").write_bytes(b"new clap")
            with self.assertRaises(FileNotFoundError):
                package(target, base / "target", output, units="drive")
            self.assertEqual({path.name: path.read_bytes() for path in output.iterdir()}, previous)


class SourceBuildPackagingTests(unittest.TestCase):
    def test_api_lists_normalize_deduplicate_and_reject_invalid_values(self):
        for value in ("all", "clap,vst3", "vst3, clap,clap", "all,clap"):
            self.assertEqual(select_apis(value), ["clap", "vst3"])
        self.assertEqual(select_apis("vst3"), ["vst3"])
        for value in ("", "clap,", ",clap", "CLAP", "au", "all,unknown"):
            with self.subTest(value=value), self.assertRaises(ValueError):
                select_apis(value)

    def test_cargo_build_uses_manifest_package_names_and_explicit_paths(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "source tree"
            for api in ("clap", "vst3"):
                manifest = root / f"plugins/example-{api}/Cargo.toml"
                manifest.parent.mkdir(parents=True)
                manifest.write_text(f'[package]\nname = "custom-{api}"\n')
            for release in (False, True):
                with patch("package_plugins.subprocess.run") as run:
                    build_plugins("aarch64-apple-darwin", root / "cache", ["example"], ["vst3"], root, release=release)
                command = run.call_args.args[0]
                self.assertEqual(command[command.index("--target") + 1], "aarch64-apple-darwin")
                self.assertEqual(command[command.index("--target-dir") + 1], str(root / "cache"))
                self.assertEqual(command[command.index("--manifest-path") + 1], str(root / "Cargo.toml"))
                self.assertIn("--locked", command)
                self.assertIn("--lib", command)
                self.assertEqual("--release" in command, release)
                self.assertEqual(command[-2:], ["-p", "custom-vst3"])
                self.assertNotIn("custom-clap", command)
                self.assertEqual(run.call_args.kwargs, {"cwd": root, "check": True})

    def test_debug_and_release_coexist_with_consistent_bundle_and_archive_names(self):
        for target, (system, _, pattern) in TARGETS.items():
            with self.subTest(target=target), tempfile.TemporaryDirectory() as temporary:
                base = Path(temporary)
                output = base / "artifacts"
                for release, profile, suffix in ((True, "release", ""), (False, "debug", "-debug")):
                    libraries = base / "cache" / target / profile
                    libraries.mkdir(parents=True)
                    for api in ("clap", "vst3"):
                        (libraries / pattern.format(f"daw_drive_{api}")).write_bytes(f"{profile}:{api}".encode())
                    with patch("package_plugins.build_plugins") as build, patch("package_plugins.finalize_macos_bundle"), contextlib.redirect_stdout(io.StringIO()):
                        build_and_package(target, base / "cache", output, units="drive", release=release)
                    self.assertEqual(build.call_args.kwargs, {"release": release})
                    archives = sorted(p for p in output.glob("*.zip") if p.name.endswith("-debug.zip") == (not release))
                    self.assertEqual(len(archives), 2)
                    for archive in archives:
                        with zipfile.ZipFile(archive) as zipped:
                            metadata = json.loads(zipped.read("build.json"))
                            api = metadata["format"]
                            self.assertEqual(metadata["profile"], profile)
                            name = f"Drive{suffix}"
                            if system == "macos":
                                binary = f"{name}.{api}/Contents/MacOS/{name}"
                                plist = plistlib.loads(zipped.read(f"{name}.{api}/Contents/Info.plist"))
                                self.assertEqual(plist["CFBundleExecutable"], name)
                                self.assertEqual(plist["CFBundleName"], "Drive")
                                self.assertEqual(plist["CFBundleIdentifier"], "io.github.fecabrera.daw.drive" + (".vst3" if api == "vst3" else ""))
                            elif api == "clap":
                                binary = f"{name}.clap"
                            elif system == "windows":
                                binary = f"{name}.vst3/Contents/x86_64-win/{name}.vst3"
                            else:
                                binary = f"{name}.vst3/Contents/x86_64-linux/{name}.so"
                            self.assertEqual(zipped.read(binary), f"{profile}:{api}".encode())
                            self.assertIn(f"Copy {name}.{api}", zipped.read("README.txt").decode())
                    checksum = next(output.glob(f"*SHA256SUMS{suffix}.txt")).read_text()
                    self.assertEqual(len(checksum.splitlines()), 2)
                    for archive in archives:
                        self.assertIn(hashlib.sha256(archive.read_bytes()).hexdigest() + "  " + archive.name, checksum)
                self.assertEqual(len(list(output.glob("*.zip"))), 4)
                self.assertEqual(len(list(output.glob("*.txt"))), 2)

    def test_subset_builds_need_only_selected_library_and_keep_other_artifacts(self):
        target = "x86_64-unknown-linux-gnu"
        for api in ("clap", "vst3"):
            with self.subTest(api=api), tempfile.TemporaryDirectory() as temporary:
                base = Path(temporary)
                library = base / "cache" / target / "debug" / f"libdaw_drive_{api}.so"
                library.parent.mkdir(parents=True)
                library.write_bytes(b"chosen format")
                output = base / "artifacts"
                output.mkdir()
                (output / "other.zip").write_bytes(b"unchanged")
                full_checksums = output / "Drive-0.1.0-linux-x64-SHA256SUMS-debug.txt"
                full_checksums.write_bytes(b"previous full-format checksums")
                with patch("package_plugins.build_plugins") as build, contextlib.redirect_stdout(io.StringIO()):
                    build_and_package(target, base / "cache", output, units="drive", api=api)
                self.assertEqual(build.call_args.args[3], [api])
                self.assertEqual(build.call_args.kwargs, {"release": False})
                self.assertEqual((output / "other.zip").read_bytes(), b"unchanged")
                self.assertEqual(full_checksums.read_bytes(), b"previous full-format checksums")
                self.assertTrue((output / f"Drive-0.1.0-linux-x64-{api}-debug.zip").is_file())
                self.assertTrue((output / f"Drive-0.1.0-linux-x64-{api}-SHA256SUMS-debug.txt").is_file())
                self.assertEqual(len(list(output.glob("*.zip"))), 2)

    def test_failed_source_build_does_not_package_stale_binaries(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            output = base / "artifacts"
            output.mkdir()
            archive = output / "previous.zip"
            archive.write_bytes(b"previous")
            stale = base / "cache/x86_64-unknown-linux-gnu/debug"
            stale.mkdir(parents=True)
            for api in ("clap", "vst3"):
                (stale / f"libdaw_drive_{api}.so").write_bytes(b"stale library")
            with patch("package_plugins.build_plugins", side_effect=subprocess.CalledProcessError(1, ["cargo"])), patch("package_plugins.stage_unit") as stage:
                with self.assertRaises(subprocess.CalledProcessError):
                    build_and_package("x86_64-unknown-linux-gnu", base / "cache", output, units="drive")
            stage.assert_not_called()
            self.assertEqual({p.name: p.read_bytes() for p in output.iterdir()}, {"previous.zip": b"previous"})

    def test_default_debug_cli_filters_cargo_and_resolves_relative_paths(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            target = "x86_64-unknown-linux-gnu"
            library = base / "cache" / target / "debug" / "libdaw_drive_clap.so"
            library.parent.mkdir(parents=True)
            library.write_bytes(b"debug source build")
            env, calls = fake_cargo(base)
            command = [sys.executable, str(ROOT / "tools/package_plugins.py"), "--units", "drive", "--api", "clap",
                       "--target", target, "--target-dir", "cache", "--output-dir", "output"]
            result = subprocess.run(command, cwd=base, env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            cargo = json.loads(calls.read_text().strip())
            self.assertNotIn("--release", cargo)
            self.assertEqual(cargo[-2:], ["-p", "daw-drive-clap"])
            self.assertNotIn("daw-drive-vst3", cargo)
            self.assertEqual(cargo[cargo.index("--target-dir") + 1], str((base / "cache").resolve()))
            archive = base / "output/Drive-0.1.0-linux-x64-clap-debug.zip"
            with zipfile.ZipFile(archive) as zipped:
                self.assertEqual(zipped.read("Drive-debug.clap"), b"debug source build")
            before = calls.read_bytes()
            command[command.index("--api") + 1] = "clap,unknown"
            result = subprocess.run(command, cwd=base, env=env, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("--api", result.stderr)
            self.assertNotIn("Traceback", result.stderr)
            self.assertEqual(calls.read_bytes(), before)


if __name__ == "__main__":
    unittest.main()
