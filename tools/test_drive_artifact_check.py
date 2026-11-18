"""Verify shared checker paths and allocation gates without building binaries."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from drive_artifact_check import build_artifact, check_artifact


class DriveArtifactCheckTests(unittest.TestCase):
    def test_each_format_uses_its_own_artifact_and_environment(self):
        for fmt in ("clap", "vst3"):
            with self.subTest(format=fmt), tempfile.TemporaryDirectory() as directory:
                work = Path(directory)
                artifact = work / "target/release" / f"libdaw_drive_{fmt}.dylib"
                artifact.parent.mkdir(parents=True)
                prefix = f"DRIVE_{fmt.upper()}"
                library = prefix + "_TEST_LIBRARY"
                allocation_gate = prefix + "_REQUIRE_ALLOCATION_CHECK"
                calls = {}

                def run(command, log, env):
                    calls[log.name] = (command, env)
                    if log.name == "instrumented-build.log":
                        artifact.write_bytes(b"instrumented")
                    if log.name == "targeted-build.jsonl":
                        artifact.write_bytes(b"normal")
                        return json.dumps({
                            "reason": "compiler-artifact",
                            "target": {"name": f"daw_drive_{fmt}"},
                        })
                    if log.name == "metadata.json":
                        return json.dumps({
                            "packages": [{"name": "daw-drive-plugin", "version": "1.2.3"}],
                        })
                    return ""

                env = {allocation_gate: "1"}
                with patch("drive_artifact_check.platform.system", return_value="Darwin"):
                    binary, tests, names, version = build_artifact(fmt, work, env, False, set(), run)
                check_artifact(fmt, work, env, binary, tests, run)

                self.assertEqual(binary, artifact)
                self.assertEqual(binary.read_bytes(), b"normal")
                self.assertEqual(names, {f"daw-drive-{fmt}"})
                self.assertEqual(version, "1.2.3")
                self.assertIn("--offline", tests)
                self.assertIn(f"daw-drive-{fmt}", tests)
                _, instrumented = calls["allocation-parity-tests.log"]
                self.assertEqual(Path(instrumented[library]).read_bytes(), b"instrumented")
                self.assertEqual(instrumented[allocation_gate], "1")
                _, normal = calls["artifact-parity-tests.log"]
                self.assertEqual(normal[library], str(artifact))
                self.assertNotIn(allocation_gate, normal)
                command, benchmark = calls["callback-budget-tests.log"]
                self.assertEqual(command[-3:], ["performance::callback_cpu_budget", "--", "--ignored"])
                self.assertEqual(benchmark[prefix + "_BENCH_REPORT"], str(work / "callback-budgets.json"))


if __name__ == "__main__":
    unittest.main()
