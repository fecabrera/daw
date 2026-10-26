"""Checks that feasibility diagnostics cannot become false production claims."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest import mock
import subprocess

SPEC = importlib.util.spec_from_file_location("probe_drive_plugins", Path(__file__).with_name("probe_drive_plugins.py"))
PROBE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROBE)


class ProductionGates(unittest.TestCase):
    def test_missing_measurements_do_not_pass(self):
        self.assertFalse(PROBE.assess_framework({})["production_contract_satisfied"])

    def test_validator_or_editor_success_cannot_waive_curve_contract(self):
        result = PROBE.assess_framework({"editor_cycles": 3, "curve_interpolated": False,
                                        "dense_event_allocations": 0, "dense_event_deallocations": 0})
        self.assertFalse(result["production_contract_satisfied"])

    def test_allocations_and_deallocations_are_independent_failures(self):
        for allocations, deallocations in [(1, 0), (0, 1), (1, 1)]:
            result = PROBE.assess_framework({"curve_interpolated": True,
                                            "dense_event_allocations": allocations,
                                            "dense_event_deallocations": deallocations})
            self.assertFalse(result["production_contract_satisfied"])

    def test_complete_evidence_passes(self):
        self.assertTrue(PROBE.assess_framework({"curve_interpolated": True,
                         "dense_event_allocations": 0, "dense_event_deallocations": 0})["production_contract_satisfied"])

    def test_dependency_audit_uses_reachable_graph_not_unused_metadata(self):
        metadata = {"resolve": {"root": "probe", "nodes": [
            {"id": "probe", "deps": [{"pkg": "unit"}]}, {"id": "unit", "deps": []},
            {"id": "unused", "deps": []}]}, "packages": [
            {"id": "probe", "name": "probe"}, {"id": "unit", "name": "daw-unit-core"},
            {"id": "unused", "name": "daw-desktop"}]}
        self.assertEqual([p["name"] for p in PROBE.reachable_packages(metadata)], ["probe", "daw-unit-core"])


class BaselineGates(unittest.TestCase):
    def test_missing_baseline_is_not_a_pass(self):
        budget = {"proposed_tail_policy_seconds": 1, "callback_p99_budget_fraction_of_period": 0.5}
        self.assertFalse(any(PROBE.assess_baseline({}, budget).values()))

    def test_over_budget_callback_or_tail_fails(self):
        budget = {"proposed_tail_policy_seconds": 1, "callback_p99_budget_fraction_of_period": 0.5}
        baseline = {"tails": [{"rate": 48000, "last_above_threshold_frame": 1000}] * 6,
                    "direct_drive_timings": [{"period_ns": 1000, "p99_ns": 100}] * 4}
        self.assertTrue(all(PROBE.assess_baseline(baseline, budget).values()))
        baseline["tails"][0] = {"rate": 48000, "last_above_threshold_frame": 48000}
        self.assertFalse(PROBE.assess_baseline(baseline, budget)["tail_survey_within_policy"])
        baseline["direct_drive_timings"][0] = {"period_ns": 1000, "p99_ns": 501}
        self.assertFalse(PROBE.assess_baseline(baseline, budget)["direct_baseline_within_callback_budget"])


class ScratchOwnership(unittest.TestCase):
    def test_refuses_unmarked_contents_and_paths_outside_work(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            existing = root / "work" / "existing"
            existing.mkdir(parents=True)
            recording = existing / "recording.wav"
            recording.write_bytes(b"retain user data")
            for path in [root / "elsewhere", root / "work", existing]:
                with self.assertRaises(ValueError):
                    PROBE.prepare_work_directory(path, root)
            self.assertEqual(recording.read_bytes(), b"retain user data")

    def test_new_and_owned_scratch_is_reusable(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "work" / "phase0"
            self.assertEqual(PROBE.prepare_work_directory(path, root), path.resolve())
            (path / "generated.log").write_text("prior run")
            self.assertEqual(PROBE.prepare_work_directory(path, root), path.resolve())


class SourceBootstrap(unittest.TestCase):
    def test_new_no_checkout_clone_is_checked_out_even_at_matching_head(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = {"url": "https://example.invalid/source.git", "revision": "a" * 40}
            commands = []
            def run(command, *_args, **_kwargs):
                commands.append(command)
                if command[:2] == ["git", "clone"]:
                    (root / "source").mkdir()
                return ""
            head = subprocess.CompletedProcess([], 0, stdout=source["revision"] + "\n")
            with mock.patch.object(PROBE, "run", side_effect=run), mock.patch.object(PROBE.subprocess, "run", return_value=head):
                PROBE.prepare_source("source", source, root, {}, True)
            self.assertIn(["git", "checkout", "--detach", source["revision"]], commands)

    def test_same_revision_does_not_allow_discarding_tracked_source_changes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "source").mkdir()
            source = {"url": "https://example.invalid/source.git", "revision": "a" * 40}
            with mock.patch.object(PROBE.subprocess, "check_output", return_value=" M header.h\n"), mock.patch.object(PROBE, "run") as run:
                with self.assertRaises(RuntimeError):
                    PROBE.prepare_source("source", source, root, {}, True)
                run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
