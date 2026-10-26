"""Check deliverable layouts, notice retention, and packaging failure behavior."""

import contextlib
import hashlib
import io
import json
from pathlib import Path
import plistlib
import tempfile
import unittest
import zipfile

from package_drive_plugins import ROOT, TARGETS, package


class PackagingTests(unittest.TestCase):
    def test_platform_archives(self):
        for target, (system, architecture, pattern) in TARGETS.items():
            with self.subTest(target=target), tempfile.TemporaryDirectory() as temporary:
                base = Path(temporary)
                release = base / "target" / target / "release"
                release.mkdir(parents=True)
                for format_name in ("clap", "vst3"):
                    (release / pattern.format(f"daw_drive_{format_name}")).write_bytes(format_name.encode())
                output = base / "artifacts"
                with contextlib.redirect_stdout(io.StringIO()):
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
                        self.assertEqual(payload, format_name.encode())
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
                    self.assertIn(f"-{system}-{architecture}-", archive.name)
                checksums = next(output.glob("*SHA256SUMS.txt")).read_text()
                for archive in archives:
                    self.assertIn(f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n", checksums)
                self.assertFalse(list(output.glob(".drive-*")))

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
