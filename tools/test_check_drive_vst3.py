"""Do not turn absent, partial, duplicate, or failed SDK evidence into success."""
import importlib.util
from pathlib import Path
import unittest
SPEC = importlib.util.spec_from_file_location("check_drive_vst3", Path(__file__).with_name("check_drive_vst3.py"))
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)

class ValidatorEvidence(unittest.TestCase):
    def test_missing_partial_and_duplicate_summaries_fail(self):
        for text in ["", "[Succeeded]", "Result: 46 tests passed, 0 tests failed", "Result: 47 tests passed, 0 tests failed\n" * 2]:
            self.assertFalse(CHECK.assess_validator(text)["all_passed"])
    def test_full_success_and_any_failure(self):
        self.assertTrue(CHECK.assess_validator("Result: 47 tests passed, 0 tests failed")["all_passed"])
        self.assertFalse(CHECK.assess_validator("Result: 46 tests passed, 1 tests failed")["all_passed"])
    def test_nonzero_exit_cannot_be_overridden_by_successful_summary(self):
        self.assertFalse(CHECK.assess_validator("Result: 47 tests passed, 0 tests failed", 255)["all_passed"])
    def test_rate_diagnostics_are_retained_without_overriding_sdk_summary(self):
        text = "ERROR: IAudioProcessor::setupProcessing (..) failed for samplerate 22050.000 Hz!\nResult: 47 tests passed, 0 tests failed"
        report = CHECK.assess_validator(text)
        self.assertTrue(report["all_passed"])
        self.assertEqual(len(report["unsupported_rate_diagnostics"]), 1)
    def test_scratch_parent_and_external_paths_are_rejected(self):
        for path in [CHECK.ROOT / "work", CHECK.ROOT / "target"]:
            with self.assertRaises(ValueError): CHECK.scratch(path)

if __name__ == "__main__": unittest.main()
