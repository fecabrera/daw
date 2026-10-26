"""The runner must retain upstream failures and reject absent/incomplete evidence."""
import importlib.util
from pathlib import Path
import unittest
SPEC = importlib.util.spec_from_file_location("check_drive_clap", Path(__file__).with_name("check_drive_clap.py"))
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)

def rows():
    return [{"test": {"plugin-instance": {"test": f"test-{i}"}}, "status": {"code": "success", "details": None}} for i in range(44)]

class ValidatorEvidence(unittest.TestCase):
    def test_absent_and_partial_results_are_not_a_pass(self):
        for value in [[], rows()[:43]]:
            self.assertFalse(CHECK.assess_validator(value)["all_passed"])
    def test_complete_success_passes(self):
        self.assertTrue(CHECK.assess_validator(rows())["all_passed"])
    def test_expected_rate_failure_is_retained_as_a_failure(self):
        value = rows()
        value[0] = {"test": {"plugin-instance": {"test": "process-varying-sample-rates"}}, "status": {"code": "failed", "details": "Error with 8000.00hz; 'clap_plugin::activate()' returned false."}}
        result = CHECK.assess_validator(value)
        self.assertFalse(result["all_passed"])
        self.assertEqual(result["counts"]["failed"], 1)
        self.assertTrue(result["only_failure_is_outside_six_rate_contract"])
        value[1]["status"]["code"] = "failed"
        self.assertFalse(CHECK.assess_validator(value)["only_failure_is_outside_six_rate_contract"])
    def test_duplicate_or_unknown_failure_cannot_pass(self):
        value = rows();value[0] = value[1]
        self.assertFalse(CHECK.assess_validator(value)["all_passed"])
        value = rows();value[0]["status"]["code"] = "crashed"
        self.assertFalse(CHECK.assess_validator(value)["only_failure_is_outside_six_rate_contract"])
    def test_scratch_refuses_parent_and_external_paths(self):
        for path in [CHECK.ROOT / "work", CHECK.ROOT / "target"]:
            with self.assertRaises(ValueError): CHECK.scratch(path)

if __name__ == "__main__": unittest.main()
