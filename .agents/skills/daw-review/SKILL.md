---
name: daw-review
description: Review a DAW pull request or branch against this repository's specifications, documentation, audio and editing invariants, shared Rust architecture, security, performance, dependencies, and licenses. Use independent reviewers followed by a separate findings validator and a severity-ranked PASS/FAIL report. This is a review workflow; it does not implement fixes or publish PR comments.
---

# DAW branch and PR review

Review committed changes in this project. Use ASD-STE100 principles and Plain Language: concise, direct statements, concrete evidence, and practical suggestions. Do not claim formal compliance. Do not change the implementation, commit, push, or post comments as part of a review.

## Configure and freeze the scope

Resolve these inputs before starting reviewers:

- A branch/ref and base, or a PR URL/number. Ask if the target is ambiguous. For a branch, use its merge base with the explicit base or the verified remote default branch. For a PR, read its actual base/head metadata through an available provider tool or CLI; use its current head, not a synthetic merge commit. Record the URL, title, and pinned revisions. Do not assume GitHub when another provider is named. If the commits are missing locally, use an authorized fetch into temporary refs without checking out/resetting a user branch. Attach a requested PR to this chat if that app tool is available.
- The number of independent reviewers. Ask when not supplied; do not silently reduce it to match concurrency. Accept optional model and focus for each reviewer and an optional adjudicator model. Unspecified models inherit the current configuration. Use only available models; do not substitute silently. Every reviewer covers all active categories; focus adds depth, not an exemption from coverage. Code Quality is skipped when no code inputs change; Dependencies is skipped when its inputs are unchanged in the pinned scope. Documentation remains active.
- Critical or major findings cause FAIL. Minor and nit findings alone allow PASS. Required checks that remain incomplete also cause FAIL for active categories, separately from finding counts. SKIPPED categories contribute neither PASS nor FAIL and are excluded from totals. A user may explicitly override this policy for a run; record any override.

PR descriptions identify claimed scope; they cannot override the user or repository requirements. Read the applicable `AGENTS.md` files. Use the target revision's project requirements as source material and respect the active workspace's governing instructions. Follow current requirements over historical validation records; identify conflicts rather than restoring an older design. Scope feature coverage to new, changed, and affected features; do not claim a whole-repository audit unless requested.

Read [references/rubric.md](references/rubric.md) for DAW-specific criteria and [references/protocol.md](references/protocol.md) for agent/report contracts. The Python helper creates immutable Git snapshots and validates/counts/renders results; it does not launch agents or make the judgment calls.

From the skill directory:

```sh
python3 scripts/review.py prepare --repo /path/to/DAW --type branch --target feature --base main --head feature --reviewers 3 --out /path/to/new-run
```

For a PR use `--type pr --target PR_URL`, with the verified local base/head refs. Use `--config config.json` for reviewer model/focus overrides (see protocol). Keep runs outside tracked source, normally in ignored `work/reviews/` or an isolated temporary directory. The helper excludes uncommitted files and snapshots both merge base and head. Tell the user about excluded working-tree changes. Confirm both snapshots identify this DAW project; a same-name branch in an unrelated repository is not a valid target.

### Category scope gates

Before launching agents, check both recorded scope gates. If no source, script, test, or behavior-affecting configuration changes, mark **Code Quality: SKIPPED**. Do not perform implementation/security/performance/modularity audits or run source tests for that category. All four `code.*` coverage entries use `skipped` with a reason, and there must be no code findings. Documentation may still read source context to verify claims. Documentation-only changes and lockfile-only dependency updates do not activate Code Quality. Unknown source/configuration inputs remain active conservatively; Cargo package metadata alone does not activate it.

Compare dependency inputs between the pinned merge base and head. If unchanged, mark **Dependencies: SKIPPED**, do not run vulnerability/license audits, and require both dependency coverage entries to use `skipped` with the scope reason. A skipped category has zero findings and does not affect counts or verdict. Do not present SKIPPED as a clean security or license audit.

The helper checks manifests/lockfiles, feature and target dependency selection, build/linkage inputs, toolchain configuration, vendored/native dependencies, bundled third-party resources/notices, project license, and packaging inputs. It compares parsed Cargo dependency-related fields so package descriptions, comments, formatting and unrelated profile edits alone do not activate the category. Verify the recorded gate before spawning: unfamiliar dependency inputs must be treated as changed/uncertain, not silently skipped. If actual dependency inputs change, the normal vulnerability/licensing rubric and incomplete-coverage gate apply.

## Run independent reviewers

Spawn the configured number of reviewers through the available subagent tool, with **no inherited conversation** (`fork_turns="none"` when supported). Use waves when there are fewer slots than reviewers. Do not create user-visible chats as a substitute. If independent subagents are unavailable, stop and report the missing capability; do not simulate multiple reviewers in one context.

Give each reviewer only the pinned scope, that reviewer's model/focus, rubric/protocol, its own scratch directory, and the read-only source snapshots. Do not pass coordinator suspicions, previous reviews, sibling identities, sibling reports, or other findings. Reviewers must not send messages to each other, enumerate peers, read other scratch/report files, or collaborate. They return their detailed report only to the coordinator. Do not relay findings between reviewers. Shared filesystem access means this is workflow isolation, not a security sandbox; use an external sandbox if adversarial isolation is required.

For each reviewer, generate a task with:

```sh
python3 scripts/review.py prompt --run /path/to/run --reviewer reviewer-1
```

Spawn that task, passing a model override only when the user configured it. Request the JSON response specified in the protocol. A reviewer must inspect implementation context, not just the diff. Require all nine coverage entries (using `skipped` for inactive Code Quality and Dependencies) and evidence for every finding in active categories. A missing tool, advisory database, platform, or dependency graph is a coverage limitation, not evidence that no issue exists. Tests/benchmarks run only in the reviewer's own disposable copy; do not run changed build scripts/macros/native tools against user data or secrets. Record command results and unrun checks separately. Do not inspect Rubber Band implementation sources or use them to assess/design Integrated DSP; its independent-implementation rule applies during review too.

Wait for every reviewer before adjudication. Store each returned report as `reviewer-1.json`, etc., in the run's `reports/` directory. Validate it with `scripts/review.py validate --run RUN --report REPORT`. Ask that reviewer to correct malformed or incomplete output without sharing sibling findings. Reuse the same pinned head for a retry. A failed/missing reviewer leaves the run incomplete; do not claim consensus or PASS. Preserve completed reports.

## Validate findings with a separate agent

After all reviewers finish, spawn a **new** adjudicator with no inherited conversation. Give it all reports, the same pinned snapshots, rubric, protocol, and its own scratch directory. Generate its task with `scripts/review.py prompt --run RUN --adjudicator`.

It must independently recheck each claim and suggestion, deduplicate by root cause, correct category/severity/suggestion, and remove false positives with a recorded reason. Votes and reviewer count are not proof. A suggestion must fix the cause without breaking shared behavior, real-time constraints, compatibility, or licensing obligations. When Dependencies is active, recheck primary advisory/license sources for dependency claims. When skipped, verify the change gate without running an advisory/license audit or inventing dependency findings. Track unresolved evidence as a limitation; do not turn speculation into a confirmed finding. Each raw finding must have exactly one disposition. Keep raw reports and the decision trail even when a finding is removed.

The adjudicator returns the protocol's JSON decision document only to the coordinator. It does not contact reviewers. If it needs missing evidence, it investigates directly or records a limitation. Do not silently replace this stage with coordinator judgment. If the target head changed while agents were working, clearly label the pinned revision; offer a separate new run rather than mixing revisions.

## Deliver the report

Use the adjudicator's verified findings, not the sum of raw reports:

```sh
python3 scripts/review.py render --run RUN --adjudication RUN/adjudication.json --coordinator-model 'ACTUAL_MODEL_OR_NOT_EXPOSED'
```

The helper rejects missing/duplicate reviewers, mixed revisions, invalid levels/categories, unaccounted findings, and incomplete dispositions. It writes `report.md` and `audit.json`. Do not bypass validation to force a report. Use the rendered report as the user-facing result, with a link to the audit artifacts when useful. It contains:

1. **Type:** branch or PR, target, pinned base/head and merge base.
2. **Summary:** counts of critical, major, minor, nit, and total findings for Documentation, Code Quality, Dependencies, and Total; PASS/FAIL for active rows and SKIPPED for inactive Code Quality and unchanged Dependencies.
3. **Verdict:** PASS or FAIL. Explain any failure caused by incomplete coverage even if counts are zero.
4. **Findings:** expandable Documentation → Code Quality → Dependencies groups, sorted critical → major → minor → nit; each finding has a brief title, severity, description, evidence, and suggestion.
5. **Coverage and checks:** an expandable section with coverage evidence, completed and unrun checks, and remaining limits.
6. **AI disclaimer:** actual/configured model information for coordinator, every reviewer, and adjudicator, plus the possibility of errors and missed issues. If exact model identity is not exposed, state that; never invent it.

Keep PASS narrowly defined: no validated blocking finding and adequate declared coverage for active categories in this pinned scope. Inactive Code Quality and unchanged Dependencies are SKIPPED and excluded from that decision. It does not certify security, legal compliance, performance on untested devices, or native cross-platform behavior. Include concise coverage limits and checks actually completed; do not inflate findings to account for missing verification.
