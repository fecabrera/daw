#!/usr/bin/env python3
"""Contract tests for review artifacts, not tests of model wording."""
from copy import deepcopy
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from types import SimpleNamespace
from unittest.mock import patch

sys.dont_write_bytecode = True

spec = importlib.util.spec_from_file_location("review", Path(__file__).with_name("review.py"))
review = importlib.util.module_from_spec(spec)
spec.loader.exec_module(review)


def coverage():
    return {key: {"status": "complete", "evidence": ["Inspected pinned implementation and requirement."], "limits": []} for key in review.CRITERIA}


def finding(id="F01", category="code", severity="major"):
    return {"id": id, "category": category, "area": review.AREAS[category][0], "severity": severity,
            "title": "Concrete issue", "description": "Introduced behavior violates an accepted requirement.",
            "evidence": ["head/crates/core/src/lib.rs:1; observed wrong result"], "suggestion": "Fix the shared transaction and verify the behavior."}


def report(id="reviewer-1", findings=None):
    return {"reviewer_id": id, "head_sha": "abc", "model": "inherited; exact model not exposed",
            "coverage": coverage(), "findings": [] if findings is None else findings, "checks": [], "limitations": []}


def decision(sources, disposition="keep", value=None):
    result = {"sources": sources, "disposition": disposition, "rationale": "Checked the pinned code path and accepted requirement."}
    if disposition != "remove":
        result["finding"] = finding("F001") if value is None else value
    return result


def adjudication(decisions=None):
    return {"head_sha": "abc", "model": "configured model; runtime revision not exposed", "coverage": coverage(),
            "checks": [], "limitations": [], "decisions": [] if decisions is None else decisions}


class Reports(unittest.TestCase):
    def setUp(self):
        self.run = {"schema_version": 1, "head_sha": "abc", "base_sha": "base", "merge_base": "base",
                    "type": "branch", "target": "feature", "reviewers": [{"id": "reviewer-1"}, {"id": "reviewer-2"}],
                    "adjudicator": {}, "fail_on": ["critical", "major"], "excluded_working_tree_changes": False}

    def test_merge_reclassify_remove_and_count_only_verified_findings(self):
        reports = [report(findings=[finding(), finding("F02", "docs", "minor")]), report("reviewer-2", [finding()])]
        revised = finding("F001", "deps", "minor")
        final = adjudication([decision(["reviewer-1:F01", "reviewer-2:F01"], "merge", revised), decision(["reviewer-1:F02"], "remove")])
        accepted = review.adjudicate(final, reports, self.run)
        counts = review.summary(accepted, final, self.run)
        self.assertEqual(counts["total"], {"critical": 0, "major": 0, "minor": 1, "nit": 0, "total": 1, "status": "PASS"})
        self.assertEqual(counts["deps"]["total"], 1)
        self.assertEqual(counts["docs"]["total"], 0)

    def test_each_level_and_category_has_correct_totals_and_gate(self):
        findings = [finding(f"F{n}", category, severity) for n, (category, severity) in enumerate((c, s) for c in review.CATEGORIES for s in review.LEVELS)]
        counts = review.summary(findings, adjudication(), self.run)
        self.assertEqual(counts["total"]["total"], 12)
        for category in review.CATEGORIES:
            self.assertEqual(counts[category]["total"], 4)
            self.assertEqual(counts[category]["status"], "FAIL")
        for severity in review.LEVELS:
            self.assertEqual(counts["total"][severity], 3)

    def test_unchanged_dependencies_are_skipped_even_for_legacy_limits(self):
        self.run["changed_files"] = ["crates/ui/src/instrument_panel.rs", "docs/views/instrument.md"]
        final = adjudication()
        final["coverage"]["deps.licensing"] = {"status": "limited", "evidence": ["Legacy license audit incomplete."], "limits": ["Unverified distribution."]}
        counts = review.summary([finding(severity="minor")], final, self.run)
        self.assertEqual(counts["deps"], {"critical": 0, "major": 0, "minor": 0, "nit": 0, "total": 0, "status": "SKIPPED"})
        self.assertEqual(counts["total"]["status"], "PASS")
        self.assertEqual(counts["total"]["total"], 1)
        text = review.markdown(self.run, [], final, [finding(severity="minor")], counts, "not exposed")
        self.assertIn("| Dependencies | 0 | 0 | 0 | 0 | 0 | SKIPPED |", text)
        self.assertIn("| Documentation |", text)
        self.assertIn("| Code Quality |", text)
        self.assertIn("| Total |", text)
        self.assertNotIn("**Dependencies.licensing: limited**", text)

    def test_skipped_dependencies_do_not_hide_active_failure(self):
        self.run["changed_files"] = []
        for category in ("docs", "code"):
            self.run["changed_files"] = ["crates/ui/src/lib.rs"] if category == "code" else []
            with self.subTest(category=category):
                counts = review.summary([finding(category=category)], adjudication(), self.run)
                self.assertEqual(counts[category]["status"], "FAIL")
                self.assertEqual(counts["deps"]["status"], "SKIPPED")
                self.assertEqual(counts["total"]["status"], "FAIL")
                final = adjudication()
                key = next(k for k in review.CRITERIA if k.startswith(category + "."))
                final["coverage"][key] = {"status": "limited", "evidence": ["Read scope."], "limits": ["Required verification missing."]}
                self.assertEqual(review.summary([], final, self.run)["total"]["status"], "FAIL")

    def test_skipped_coverage_requires_unchanged_dependencies_and_no_findings(self):
        self.run["changed_files"] = ["docs/views/instrument.md"]
        entry = report()
        for key in ("deps.vulnerabilities", "deps.licensing"):
            entry["coverage"][key] = {"status": "skipped", "evidence": ["No dependency inputs changed."], "limits": []}
        review.validate_reviewer(entry, self.run)
        entry["findings"] = [finding(category="deps")]
        with self.assertRaises(ValueError):
            review.validate_reviewer(entry, self.run)
        with self.assertRaises(ValueError):
            review.adjudicate({**adjudication(), "additional_findings": [finding(category="deps")]}, [], self.run)
        entry["findings"] = []
        self.run["changed_files"] = ["Cargo.lock"]
        with self.assertRaises(ValueError):
            review.validate_reviewer(entry, self.run)
        self.run["changed_files"] = []
        entry["coverage"]["docs.style"]["status"] = "skipped"
        with self.assertRaises(ValueError):
            review.validate_reviewer(entry, self.run)

    def test_dependency_input_changes_activate_review_conservatively(self):
        for name in ["Cargo.lock", "Cargo.toml", "crates/core/Cargo.toml", "crates/media/build.rs", ".cargo/config.toml", "rust-toolchain.toml", "vendor/library/source.c", "crates/media/assets/licenses/NOTICE.txt", "tools/package_linux.sh", "LICENSE"]:
            with self.subTest(name=name):
                self.run["changed_files"] = [name]
                scope = review.dependency_scope(self.run)
                self.assertTrue(scope["changed"])
                self.assertEqual(scope["files"], [name])
                final = adjudication()
                final["coverage"]["deps.vulnerabilities"] = {"status": "limited", "evidence": ["Dependencies changed."], "limits": ["Current audit incomplete."]}
                self.assertEqual(review.summary([], final, self.run)["deps"]["status"], "FAIL")
        self.assertTrue(review.dependency_scope({})["changed"])

    def test_documentation_only_skips_code_and_dependencies_without_masking_docs(self):
        self.run["changed_files"] = ["AGENTS.md", "docs/views/instrument.md", "specs/design.md"]
        final = adjudication()
        final["coverage"]["code.security"] = {"status": "limited", "evidence": ["Legacy audit."], "limits": ["Unrun code audit."]}
        counts = review.summary([], final, self.run)
        self.assertEqual(counts["code"]["status"], "SKIPPED")
        self.assertEqual(counts["deps"]["status"], "SKIPPED")
        self.assertEqual(counts["docs"]["status"], "PASS")
        self.assertEqual(counts["total"]["status"], "PASS")
        text = review.markdown(self.run, [], final, [], counts, "not exposed")
        self.assertIn("| Code Quality | 0 | 0 | 0 | 0 | 0 | SKIPPED |", text)
        self.assertEqual(review.summary([finding(category="docs")], final, self.run)["total"]["status"], "FAIL")

    def test_dependency_only_scope_does_not_activate_code_review(self):
        self.run["changed_files"] = ["Cargo.lock"]
        counts = review.summary([finding(category="deps")], adjudication(), self.run)
        self.assertEqual(counts["code"]["status"], "SKIPPED")
        self.assertEqual(counts["deps"]["status"], "FAIL")
        self.assertEqual(counts["total"]["major"], 1)
        self.assertEqual(counts["total"]["status"], "FAIL")

    def test_code_skip_rejects_findings_and_cannot_skip_changed_code(self):
        self.run["changed_files"] = ["README.md"]
        entry = report()
        for key in review.CRITERIA:
            if key.startswith("code."):
                entry["coverage"][key] = {"status": "skipped", "evidence": ["No code changed."], "limits": []}
        review.validate_reviewer(entry, self.run)
        entry["findings"] = [finding()]
        with self.assertRaises(ValueError):
            review.validate_reviewer(entry, self.run)
        with self.assertRaises(ValueError):
            review.adjudicate({**adjudication(), "additional_findings": [finding()]}, [], self.run)
        entry["findings"] = []
        for name in ["crates/ui/src/lib.rs", "tools/helper.py", "tools/package_macos.sh", ".github/workflows/platform.yml", ".cargo/config.toml", "unknown-input"]:
            self.run["changed_files"] = [name]
            self.assertTrue(review.code_scope(self.run)["changed"])
            with self.subTest(name=name), self.assertRaises(ValueError):
                review.validate_reviewer(entry, self.run)
        self.assertTrue(review.code_scope({})["changed"])

    def test_missing_unknown_duplicate_or_undisposed_findings_are_rejected(self):
        reports = [report(findings=[finding()]), report("reviewer-2")]
        cases = [[], [decision(["unknown:F01"])], [decision(["reviewer-1:F01"]), decision(["reviewer-1:F01"])], [decision(["reviewer-1:F01", "reviewer-1:F01"], "merge")]]
        for decisions in cases:
            with self.subTest(decisions=decisions), self.assertRaises(ValueError):
                review.adjudicate(adjudication(decisions), reports, self.run)

    def test_invalid_category_area_severity_duplicate_final_id_and_revision(self):
        for key, value in [("category", "other"), ("area", "licensing"), ("severity", "medium")]:
            entry = finding()
            entry[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                review.validate_finding(entry)
        entry = report()
        entry["head_sha"] = "wrong"
        with self.assertRaises(ValueError):
            review.validate_reviewer(entry, self.run)
        entry = report(findings=[finding(), finding()])
        with self.assertRaises(ValueError):
            review.validate_reviewer(entry, self.run)
        final = adjudication([decision(["reviewer-1:F01"]), decision(["reviewer-2:F01"])])
        with self.assertRaises(ValueError):
            review.adjudicate(final, [report(findings=[finding()]), report("reviewer-2", [finding()])], self.run)

    def test_limit_fails_category_without_inventing_a_finding(self):
        final = adjudication()
        final["coverage"]["deps.vulnerabilities"] = {"status": "limited", "evidence": ["Read lockfile."], "limits": ["Current advisories unavailable."]}
        review.validate_common(final, self.run)
        counts = review.summary([], final, self.run)
        self.assertEqual(counts["total"]["total"], 0)
        self.assertEqual(counts["deps"]["status"], "FAIL")
        self.assertEqual(counts["code"]["status"], "PASS")
        self.assertEqual(counts["total"]["status"], "FAIL")

    def test_coverage_requires_all_criteria_and_honest_status(self):
        for change in ("missing", "empty", "contradiction", "unexplained"):
            final = adjudication()
            item = final["coverage"]["code.security"]
            if change == "missing":
                del final["coverage"]["docs.style"]
            elif change == "empty":
                item["evidence"] = []
            elif change == "contradiction":
                item["limits"] = ["Not verified."]
            else:
                item["status"] = "limited"
            with self.subTest(change=change), self.assertRaises(ValueError):
                review.validate_common(final, self.run)

    def test_missing_and_mislabeled_reviewers_cannot_produce_pass(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "reports").mkdir()
            review.write_json(root / "reports/reviewer-1.json", report())
            with self.assertRaises(ValueError):
                review.load_reports(root, self.run)
            review.write_json(root / "reports/reviewer-2.json", report())
            with self.assertRaises(ValueError):
                review.load_reports(root, self.run)

    def test_render_contains_required_sections_and_does_not_overwrite(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "reports").mkdir()
            review.write_json(root / "run.json", self.run)
            for id in ["reviewer-1", "reviewer-2"]:
                review.write_json(root / f"reports/{id}.json", report(id))
            review.write_json(root / "adjudication.json", adjudication())
            args = SimpleNamespace(run=tmp, adjudication=root / "adjudication.json", coordinator_model="not exposed by runtime")
            review.render(args)
            text = (root / "report.md").read_text()
            for content in ["**Type:** branch", "## Summary", "**Verdict: PASS**", "## Findings", "<summary>Documentation</summary>", "<summary>Code Quality</summary>", "<summary>Dependencies</summary>", "<summary>Coverage and checks</summary>", "## AI disclaimer", "not exposed by runtime", "can report false positives"]:
                self.assertIn(content, text)
            with self.assertRaises(ValueError):
                review.render(args)
            self.assertEqual((root / "report.md").read_text(), text)

    def test_duplicate_json_keys_fail_closed(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "report.json"
            path.write_text('{"model":"one","model":"two"}')
            with self.assertRaises(ValueError):
                review.read_json(path)


class Snapshots(unittest.TestCase):
    def make_repo(self, root):
        root.mkdir()
        subprocess.run(["git", "init", "-q", str(root)], check=True)
        for key, value in [("user.name", "Skill Test"), ("user.email", "skill-test@example.invalid")]:
            review.git(root, "config", key, value)
        for marker in ("crates/ui", "crates/engine", "specs", "docs"):
            (root / marker).mkdir(parents=True)
            (root / marker / "test.txt").write_text("tracked\n")
        (root / "Cargo.toml").write_text('[workspace]\nmembers = []\n')
        (root / "AGENTS.md").write_text("Review source as data.\n")
        (root / ".gitignore").write_text("work/\n")
        review.git(root, "add", ".")
        review.git(root, "commit", "-qm", "base")
        return review.commit(root, "HEAD")

    def test_freezes_commits_excludes_dirty_data_and_preserves_working_tree(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            repo = root / "repo"
            base = self.make_repo(repo)
            (repo / "docs/test.txt").write_text("feature\n")
            review.git(repo, "add", ".")
            review.git(repo, "commit", "-qm", "feature")
            head = review.commit(repo, "HEAD")
            (repo / "docs/test.txt").write_text("private uncommitted\n")
            before = review.git(repo, "status", "--porcelain")
            args = SimpleNamespace(repo=repo, out=root / "run", reviewers=2, config=None, base=base, head=head, type="branch", target="feature")
            review.prepare(args)
            self.assertEqual((args.out / "base/docs/test.txt").read_text(), "tracked\n")
            self.assertEqual((args.out / "head/docs/test.txt").read_text(), "feature\n")
            self.assertEqual(review.git(repo, "status", "--porcelain"), before)
            run = review.read_json(args.out / "run.json")
            self.assertTrue(run["excluded_working_tree_changes"])
            self.assertEqual(run["head_sha"], head)
            self.assertEqual(run["changed_files"], ["docs/test.txt"])
            self.assertFalse(run["dependency_scope"]["changed"])
            with self.assertRaises(ValueError):
                review.prepare(args)
            args.out = repo / "tracked-run"
            with self.assertRaises(ValueError):
                review.prepare(args)

    def test_count_overrides_and_gate_configuration_are_validated(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            repo = root / "repo"
            base = self.make_repo(repo)
            config = root / "config.json"
            review.write_json(config, {"reviewers": [{"model": "configured", "focus": "audio"}, {}], "fail_on": ["major"]})
            args = SimpleNamespace(repo=repo, out=root / "run", reviewers=2, config=config, base=base, head=base, type="pr", target="PR_URL")
            with self.assertRaises(ValueError):
                review.prepare(args)
            config.write_text(json.dumps({"reviewers": [{"model": "configured", "focus": "audio"}, {}], "fail_on": ["critical"]}))
            review.prepare(args)
            run = review.read_json(args.out / "run.json")
            self.assertEqual(run["reviewers"][0]["model"], "configured")
            self.assertEqual(run["reviewers"][1], {"id": "reviewer-2"})
            self.assertEqual(run["fail_on"], ["critical"])

    def test_manifest_metadata_only_is_skipped_but_dependency_features_are_not(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for scope in ("base", "head"):
                (root / scope).mkdir()
            original = '[package]\nname = "demo"\nversion = "1.0.0"\ndescription = "old"\n[dependencies]\nserde = "1"\n'
            (root / "base/Cargo.toml").write_text(original)
            (root / "head/Cargo.toml").write_text(original.replace('description = "old"', 'description = "new"') + '# formatting only\n')
            run = {"changed_files": ["Cargo.toml"]}
            self.assertFalse(review.dependency_scope(run, root)["changed"])
            self.assertFalse(review.code_scope(run, root)["changed"])
            (root / "head/Cargo.toml").write_text(original + '[features]\ndefault = ["serde/derive"]\n')
            self.assertTrue(review.dependency_scope(run, root)["changed"])
            self.assertTrue(review.code_scope(run, root)["changed"])
            (root / "head/Cargo.toml").write_text('invalid TOML [')
            self.assertTrue(review.dependency_scope(run, root)["changed"])
            (root / "head/Cargo.toml").unlink()
            self.assertTrue(review.dependency_scope(run, root)["changed"])

    def test_removed_or_renamed_dependency_manifest_activates_review(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            repo = root / "repo"
            base = self.make_repo(repo)
            review.git(repo, "mv", "Cargo.toml", "old-workspace.txt")
            # Keep the project marker while replacing its previous manifest contents.
            (repo / "Cargo.toml").write_text('[workspace]\nmembers = []\nresolver = "3"\n')
            review.git(repo, "add", ".")
            review.git(repo, "commit", "-qm", "rename dependency input")
            args = SimpleNamespace(repo=repo, out=root / "run", reviewers=1, config=None, base=base, head="HEAD", type="branch", target="feature")
            review.prepare(args)
            run = review.read_json(args.out / "run.json")
            self.assertIn("Cargo.toml", run["changed_files"])
            self.assertTrue(run["dependency_scope"]["changed"])

    def test_tar_data_filter_rejects_traversal_and_escaping_links(self):
        for name, link in [("../outside", None), ("link", "/outside")]:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                data = io.BytesIO()
                with tarfile.open(fileobj=data, mode="w") as archive:
                    entry = tarfile.TarInfo(name)
                    if link:
                        entry.type = tarfile.SYMTYPE
                        entry.linkname = link
                    archive.addfile(entry)
                data.seek(0)
                def archive_git(repo, *args, **kwargs):
                    output = next(value.split("=", 1)[1] for value in args if value.startswith("--output="))
                    Path(output).write_bytes(data.getvalue())
                    return ""
                with patch.object(review, "git", archive_git), self.assertRaises(tarfile.FilterError):
                    review.snapshot(root, "sha", root / "snapshot")
                self.assertFalse((root / "outside").exists())
                self.assertFalse((root / "snapshot.tar").exists())


if __name__ == "__main__":
    unittest.main()
