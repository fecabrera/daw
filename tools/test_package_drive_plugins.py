"""Check deliverable layouts, notice retention, and packaging failure behavior."""

import contextlib
import hashlib
import io
import json
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile

from package_drive_plugins import ROOT, TARGETS, finalize_macos_bundle, package


class PackagingTests(unittest.TestCase):
    def test_checker_imports_do_not_require_python311_tomllib(self):
        subprocess.run([sys.executable, "-c",
                        "import sys; sys.modules['tomllib'] = None; "
                        "import check_drive_clap; import check_drive_vst3"],
                       cwd=ROOT / "tools", check=True, capture_output=True)

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
                    "package_drive_plugins.finalize_macos_bundle", side_effect=sign_fixture
                ):
                    package(target, base / "target", output)
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
                        self.assertIn("README.txt", zipped.namelist())
                        if system == "macos":
                            self.assertIn(f"Drive.{format_name}/Contents/_CodeSignature/CodeResources", zipped.namelist())
                    self.assertEqual((release / pattern.format(f"daw_drive_{format_name}")).read_bytes(), format_name.encode())
                    self.assertIn(f"-{system}-{architecture}-", archive.name)
                checksums = next(output.glob("*SHA256SUMS.txt")).read_text()
                for archive in archives:
                    self.assertIn(f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n", checksums)
                self.assertFalse(list(output.glob(".drive-*")))

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
                    "package_drive_plugins.finalize_macos_bundle", side_effect=sign_fixture
                ):
                    package(target, base / "target", output, unit="drive-v2")
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
                self.assertFalse(list(output.glob(".drive-*")))

    def test_macos_finalization_relocates_before_signing_complete_bundle(self):
        bundle = Path("Drive.vst3")
        binary = bundle / "Contents/MacOS/Drive"
        with patch("package_drive_plugins.subprocess.run") as run:
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
            with contextlib.redirect_stdout(io.StringIO()), patch("package_drive_plugins.finalize_macos_bundle"):
                package(target, base / "target", output)
            before = {path.name: path.read_bytes() for path in output.iterdir()}
            failure = subprocess.CalledProcessError(1, ["codesign"])
            with patch("package_drive_plugins.finalize_macos_bundle", side_effect=[None, failure]):
                with self.assertRaises(subprocess.CalledProcessError):
                    package(target, base / "target", output)
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
                package(target, base / "target", output)
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
                package(target, base / "target", output)
            previous = {path.name: path.read_bytes() for path in output.iterdir()}
            vst3.unlink()
            (release / "libdaw_drive_clap.so").write_bytes(b"new clap")
            with self.assertRaises(FileNotFoundError):
                package(target, base / "target", output)
            self.assertEqual({path.name: path.read_bytes() for path in output.iterdir()}, previous)


if __name__ == "__main__":
    unittest.main()
