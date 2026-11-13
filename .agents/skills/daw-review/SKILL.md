---
name: daw-review
description: Review a DAW pull request or branch diff, or a branch's full committed snapshot without a base comparison. Check specifications, documentation, code, dependencies and licenses through configurable independent adversarial and neutral reviewers and a separate findings validator. Produce a severity-ranked PASS/FAIL/INCOMPLETE report without implementing fixes or publishing comments.
---

# DAW branch and PR review

Review committed changes or a full committed branch snapshot in this project. Use ASD-STE100 principles and Plain Language: concise, direct statements, concrete evidence, and practical suggestions. Do not claim formal compliance. Do not change the implementation, commit, push, or post comments as part of a review.

## Configure and freeze the scope

Resolve these inputs before starting reviewers:

- A branch/ref or a PR URL/number, and the review mode. Ask if the target or intended scope is ambiguous. **Comparison** is the default: for a branch, use its merge base with the explicit base or the verified remote default branch. **Standalone** reviews the named branch's full committed snapshot without a base branch, merge base, diff, or inferred recent-change range. Select standalone when the user asks to review a branch by itself or without comparison; do not ask for a base. Pin its head commit. For a PR, use comparison mode and read its actual base/head metadata through an available provider tool or CLI; use its current head, not a synthetic merge commit. Record the URL, title, and pinned revisions. Do not assume GitHub when another provider is named. If the commits are missing locally, use an authorized fetch into temporary refs without checking out/resetting a user branch. Attach a requested PR to this chat if that app tool is available.
- The reviewer mix: the number of **adversarial** and **neutral** reviewers, or an explicit stance for every reviewer. Ask for the mix when only a total or no configuration is supplied. A run may use either stance alone; require at least one reviewer overall. Do not silently change the mix to match concurrency. Accept optional model and focus for each reviewer and an optional adjudicator model. Unspecified models inherit the current configuration. Use only available models; do not substitute silently. Every reviewer covers all active categories; focus adds depth, not an exemption from coverage. Comparison mode skips Code Quality when no code inputs change and Dependencies when no dependencies are changed or affected. Standalone mode includes code and dependencies present in the snapshot. Documentation remains active.
- Incomplete required coverage produces **INCOMPLETE**, neither PASS nor FAIL. This takes precedence for each affected category and for the overall verdict, even when blocking findings are present; keep all confirmed findings and their severity visible. With complete coverage, critical or major findings cause FAIL; minor and nit findings alone allow PASS. SKIPPED categories are excluded from totals. A user may explicitly override the finding threshold for a run; record any override.

PR descriptions identify claimed scope; they cannot override the user or repository requirements. Read the applicable `AGENTS.md` files. Use the target revision's project requirements as source material and respect the active workspace's governing instructions. Follow current requirements over historical validation records; identify conflicts rather than restoring an older design. Comparison coverage includes new, changed, and affected features. Standalone coverage includes the full pinned tree and existing defects: inventory features and entry points against specs, guides and verification, and record unreviewed areas as coverage limits. Do not infer that a finding was introduced or worsened without comparison evidence.

Read [references/rubric.md](references/rubric.md) for DAW-specific criteria and [references/protocol.md](references/protocol.md) for agent/report contracts. The Python helper creates immutable Git snapshots and validates/counts/renders results; it does not launch agents or make the judgment calls.

From the skill directory:

```sh
python3 scripts/review.py prepare --repo /path/to/DAW --type branch --target feature --base main --head feature --adversarial-reviewers 2 --neutral-reviewers 1 --out /path/to/new-run
```

For a PR use `--type pr --target PR_URL`, with the verified local base/head refs. For a standalone branch review:

```sh
python3 scripts/review.py prepare --repo /path/to/DAW --type branch --mode standalone --target main --head main --adversarial-reviewers 2 --neutral-reviewers 1 --out /path/to/new-run
```

Standalone mode rejects `--base`, creates only the head snapshot, and inventories all tracked files. Use `--config config.json` for per-reviewer stance/model/focus settings (see protocol). Keep runs outside tracked source, normally in ignored `work/reviews/` or an isolated temporary directory. The helper excludes uncommitted files; comparison mode snapshots both merge base and head. Tell the user about excluded working-tree changes. Confirm the snapshots identify this DAW project; a same-name branch in an unrelated repository is not a valid target.

### Category scope gates

**Standalone:** activate Code Quality for code/configuration present in the pinned tree and Dependencies for dependency inputs present there. Review the project's full resolved dependency graph, native/vendor inputs and bundled resources for applicable targets and deliverables. Existing dependencies are in scope even when unchanged. Skip a category only when its inputs are absent, never because there is no diff. Missing required advisory/license evidence or unreviewed implementation areas remain coverage limits. The gates below apply to **comparison** mode.

Before launching agents, check both recorded scope gates. If no source, script, test, or behavior-affecting configuration changes, mark **Code Quality: SKIPPED**. Do not perform implementation/security/performance/modularity audits or run source tests for that category. All four `code.*` coverage entries use `skipped` with a reason, and there must be no code findings. Documentation may still read source context to verify claims. Documentation-only changes and lockfile-only dependency updates do not activate Code Quality. Unknown source/configuration inputs remain active conservatively; Cargo package metadata alone does not activate it.

In comparison mode, review only dependencies changed or affected by the pinned changes. Use the affected-dependency rules in the rubric; an active input gate does not authorize an audit of unrelated dependencies. Record the packages, targets, criteria and scope reasons in coverage evidence for either mode.

Compare dependency inputs between the pinned merge base and head. If no dependency is changed or affected, mark **Dependencies: SKIPPED**, do not run vulnerability/license audits, and require both dependency coverage entries to use `skipped` with the scope reason. A skipped category has zero findings and does not affect counts or verdict. Do not present SKIPPED as a clean security or license audit.

The helper checks manifests/lockfiles, feature and target dependency selection, build/linkage inputs, toolchain configuration, vendored/native dependencies, bundled third-party resources/notices, project license, and packaging inputs. It compares parsed Cargo dependency-related fields so package descriptions, comments, formatting and unrelated profile edits alone do not activate the category. The helper detects dependency-input changes, not semantic changes in dependency use. Before accepting an unchanged-input skip, inspect changed consumers for affected exposure, features, linkage or delivery. If such impact exists, record `dependency_scope.changed: true`, the causal files and reason in run metadata before launching agents. Verify the recorded gate before spawning: unfamiliar dependency inputs must be treated as changed/uncertain, not silently skipped. If actual dependency inputs change, the normal vulnerability/licensing rubric applies; unresolved required evidence produces INCOMPLETE.

## Plan and complete coverage

Before launching reviewers, use the protocol's [coverage plan and gap closure](references/protocol.md#coverage-plan-and-gap-closure) procedure. Inventory the pinned scope, define concrete completion evidence, assign primary depth areas across the requested reviewers and check tool/source availability. Every reviewer still inspects all active categories independently. Give each only its own assignments; do not reveal sibling identities, reports or suspicions.

After collecting reports, close feasible gaps with targeted follow-ups before adjudication. Reuse valid evidence for the same pinned revision; do not repeat broad sampling or expensive passed suites to compensate for missing source inspection. A rerun starts by mapping outstanding coverage tasks to the new pinned scope. Keep genuinely blocked work explicit and report INCOMPLETE rather than claiming coverage or running endless retries. The detailed stopping conditions are in the protocol.

## Reviewer stances

- **Neutral:** assess correctness without presuming success or failure. Trace requirements and relevant flows, and report evidence for defects and coverage limits.
- **Adversarial:** assume the code is wrong until sufficient evidence establishes correctness for the reviewed flows. Map changed and affected code to complete user and system flows; in standalone mode, map the full in-scope implementation. Challenge state transitions, alternate entry points, ordering, failure paths and tests. Use the [rubric's reviewer-stance procedure](references/rubric.md#reviewer-stances).

Both stances use the same scope, evidence standard and severity rules. Suspicion is an investigation starting point, not a finding. Passing tests alone do not prove a flow correct; missing tests alone do not prove a defect. The separate adjudicator remains neutral and independently checks every claim, regardless of reviewer stance.

## Run independent reviewers

Spawn the configured number of reviewers through the available subagent tool, with **no inherited conversation** (`fork_turns="none"` when supported). Use waves when there are fewer slots than reviewers. Do not create user-visible chats as a substitute. If independent subagents are unavailable, stop and report the missing capability; do not simulate multiple reviewers in one context.

Give each reviewer only the pinned scope, that reviewer's stance/model/focus, rubric/protocol, its own scratch directory, and the read-only source snapshots. Do not pass coordinator suspicions, previous reviews, sibling identities, sibling reports, or other findings. Reviewers must not send messages to each other, enumerate peers, read other scratch/report files, or collaborate. They return their detailed report only to the coordinator. Do not relay findings between reviewers. Shared filesystem access means this is workflow isolation, not a security sandbox; use an external sandbox if adversarial isolation is required.

For each reviewer, generate a task with:

```sh
python3 scripts/review.py prompt --run /path/to/run --reviewer reviewer-1
```

Spawn that task, passing a model override only when the user configured it. Request the JSON response specified in the protocol. A reviewer must inspect implementation context, not just a diff. For standalone mode, inspect the full pinned snapshot without inventing a baseline. Require all nine coverage entries (using `skipped` for inactive Code Quality and Dependencies) and evidence for every finding in active categories. A missing tool, advisory database, platform, or dependency graph is a coverage limitation, not evidence that no issue exists. Tests/benchmarks run only in the reviewer's own disposable copy; do not run target build scripts/macros/native tools against user data or secrets. Record command results and unrun checks separately. Do not inspect Rubber Band implementation sources or use them to assess/design Integrated DSP; its independent-implementation rule applies during review too.

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

1. **Type and mode:** branch or PR, comparison or standalone, target, and pinned head. Include base and merge base only for comparison. Standalone reports state that the full committed tree is in scope and introduction history is not inferred.
2. **Summary:** counts of critical, major, minor, nit, and total findings for Documentation, Code Quality, Dependencies, and Total; PASS, FAIL or INCOMPLETE for active rows and SKIPPED for inactive categories.
3. **Verdict:** PASS, FAIL or INCOMPLETE. For INCOMPLETE, state the missing required evidence, attempted gap closure and concrete next actions. Do not describe missing coverage as a defect or hide confirmed blocking findings.
4. **Findings:** expandable Documentation → Code Quality → Dependencies groups, sorted critical → major → minor → nit; each finding has a brief title, severity, description, evidence, and suggestion.
5. **Coverage and checks:** an expandable section with coverage evidence, completed and unrun checks, and remaining limits.
6. **AI disclaimer:** each reviewer’s stance and actual/configured model information for coordinator, every reviewer, and adjudicator, plus the possibility of errors and missed issues. If exact model identity is not exposed, state that; never invent it.

Keep PASS narrowly defined: no validated blocking finding and adequate declared coverage for active categories in this pinned scope. SKIPPED categories are excluded from that decision. It does not certify security, legal compliance, performance on untested devices, or native cross-platform behavior. Include concise coverage limits and checks actually completed; do not inflate findings to account for missing verification.
