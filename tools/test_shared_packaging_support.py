"""Check the platform layout and live CI log helpers without native binaries."""
import os
import plistlib
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

from plugin_bundle_layout import preview_binary, stage_test_bundle

ROOT = Path(__file__).resolve().parents[1]
# Use the CI step's Bash or resolve it before Windows searches system directories.
BASH = os.environ.get("DAW_TEST_BASH") or shutil.which("bash")


class SharedPackagingSupportTests(unittest.TestCase):
    def test_owned_fixture_paths_and_payloads(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "fixture"
            source.write_bytes(b"owned fixture")
            for system, machine, path in [
                ("Darwin", "arm64", "Contents/MacOS/Payload"),
                ("Linux", "aarch64", "Contents/aarch64-linux/Fixture.so"),
                ("Windows", "aarch64", "Contents/arm64-win/Fixture.vst3"),
                ("Windows", "ARM64", "Contents/arm64-win/Fixture.vst3"),
                ("Windows", "AMD64", "Contents/x86_64-win/Fixture.vst3"),
            ]:
                with self.subTest(system=system, machine=machine):
                    bundle = root / f"{system}-{machine}.vst3"
                    stage_test_bundle(bundle, source, "Fixture", "vst3", system,
                                      machine, "Payload", "org.daw.fixture")
                    self.assertEqual((bundle / path).read_bytes(), source.read_bytes())
                    if system == "Darwin":
                        info = plistlib.loads((bundle / "Contents/Info.plist").read_bytes())
                        self.assertEqual(info, {
                            "CFBundleExecutable": "Payload",
                            "CFBundleIdentifier": "org.daw.fixture",
                            "CFBundlePackageType": "BNDL",
                        })
                    else:
                        self.assertEqual(preview_binary(bundle, "Fixture", "clap", system), bundle)

    @unittest.skipUnless(BASH and shutil.which("tee"), "Bash and tee required")
    def test_capture_logs_both_streams_and_preserves_failures(self):
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "with spaces.log"
            script = [BASH, (ROOT / "tools/capture_ci_log.sh").as_posix()]
            command = [BASH, "-c", 'printf "%s\\n" "$1"; printf "diagnostic\\n" >&2; exit 7',
                       "bash", "argument with spaces"]
            result = subprocess.run([*script, log.as_posix(), *command], capture_output=True)
            self.assertEqual(result.returncode, 7, (result.stdout, result.stderr))
            self.assertEqual(result.stdout, b"argument with spaces\ndiagnostic\n")
            self.assertEqual(log.read_bytes(), result.stdout)
            subprocess.run([*script, "--append", log.as_posix(), "printf", "another"], check=True,
                           capture_output=True)
            self.assertEqual(log.read_bytes(), result.stdout + b"another")
            failed = subprocess.run([*script, (log / "missing").as_posix(), "printf", "still live"],
                                    capture_output=True)
            self.assertNotEqual(failed.returncode, 0)
            self.assertEqual(failed.stdout, b"still live")
