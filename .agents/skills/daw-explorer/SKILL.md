---
name: daw-explorer
description: Explore a DAW feature, bugfix, or improvement request through configurable independent planning agents and a separate evaluator. Deliver complete implementation alternatives, requirements checks, size and scope rankings, and one recommended integral plan in Markdown. Use for implementation planning, not branch reviews or implementation work.
---

# DAW implementation explorer

Turn a request of any size into an evidence-based implementation plan. Each
explorer proposes a complete solution independently. A separate evaluator checks
the proposals and recommends one integral plan. Do not implement the request,
edit accepted specifications, commit, push, or publish external comments.

## Configure the run

Read the repository's `AGENTS.md` and relevant nested instructions. Use concise
Plain Language and ASD-STE100 principles without claiming formal compliance.

Resolve the following before launching agents:

- The feature, bugfix, or improvement request and its acceptance requirements.
  Include explicit corrections and constraints from the caller. Ask about
  ambiguity that changes required behavior; resolve routine design choices
  within scope. Do not use this skill's creation request as a feature request.
- The number of independent explorers. Use the caller's count; ask if absent.
  Accept any positive integer and use waves when capacity is lower. A caller
  selecting one explorer still gets a separate evaluator, but no independent
  proposal comparison. Do not reduce the count for small requests or increase it
  for large ones.
- Optional model and focus per explorer, plus an optional evaluator model.
  Unspecified models inherit the runtime configuration. Verify requested models
  are available; do not silently substitute. Focus adds depth, not permission to
  submit only one part of the solution.
- Source target and report destination. Default to the current working tree,
  including relevant local edits. For a supplied ref, inspect that ref without
  changing the user's checkout. Default output is a new ignored
  `work/explorations/<unique-run>/` directory, or a temporary directory if that
  path is not ignored. Honor an explicit Markdown destination without
  overwriting unrelated content.

Do not ask again for inputs already supplied. There is no feature-size limit and
no requirement to simplify a solution merely to reduce its size.

## Freeze the shared brief

Create `brief.md` and `run.md` in the run directory. Give requirements stable
IDs, distinguish mandatory behavior from preferences, and record constraints,
non-goals, known unknowns, target revision, local changes, configured agents,
model information actually exposed by the runtime, and output paths. User
requirements take precedence over older documentation; record contradictions.

Give all agents the same source view. Prefer a disposable source snapshot when
practical. For a live working tree, record Git status and content hashes for
evidence files and recheck them before evaluation and delivery. If relevant
inputs change, revalidate the affected proposals against one consistent baseline
or mark the run incomplete. Never mix evidence silently or copy secrets, private
media, caches, or build outputs into a snapshot.

Read [references/protocol.md](references/protocol.md) for the agent contracts and
decision rules. Use [assets/report-template.md](assets/report-template.md) for
the final report. Pass both resources to the evaluator.

## Run independent explorers

Launch the configured count using actual subagents with no inherited
conversation (`fork_turns="none"` when available). Give each only the shared
brief, source location and governing instructions, protocol, its own configured
focus, and its own output path. Do not seed a preferred solution or pass sibling
identities, messages, proposals, or coordinator conclusions. Each agent must
inspect the relevant source and requirements itself, cover the full request,
and return `proposal-N.md` using the protocol.

Explorers must not contact peers, enumerate agents, or read sibling artifacts.
Shared files provide workflow isolation, not a security sandbox. Keep source
read-only. Any permitted diagnostic checks run in disposable scratch space and
must not modify user projects. Report actual checks separately from proposed
tests. Do not inspect Rubber Band implementation sources; the repository's
independent implementation rule applies to planning too.

Wait for all explorers. Ask the original author to complete missing proposal
sections without sharing other proposals. Preserve failed or missing outputs as
coverage limits. If real subagents are unavailable, report that limitation; do
not simulate independence in one context.

## Evaluate and deliver

After the explorer stage ends, launch a new independent evaluator with no
inherited conversation. It must not be one of the explorers. Give it the shared
brief, same source view, all available raw proposals, protocol, report template,
and its own output path. Do not pass a coordinator ranking or preferred winner.
The evaluator investigates claims directly and does not contact explorers.

Require `evaluation.md` with a disposition for every proposal and a completed
`report.md`. Follow the protocol's requirements gate before ranking. The final
report contains viable alternatives and one integral recommended plan, or an
explicit explanation that no plan can yet be recommended. Do not silently
convert rejected or unresolved proposals into accepted recommendations.

Check proposal counts, dispositions, requirement coverage, rankings, report
links, baseline consistency, and preservation of user files. Return the report
as Markdown with a local file link and a short result. Keep raw proposals and
evaluation available for inspection. Implementation still requires a separate
user request; exploration does not approve product requirements or authorize
implementation.
