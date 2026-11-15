# Audit evidence and agent contracts

Read this protocol for every audit. It defines the evidence needed for clearance
and the records that independent auditors and the final validator must return.

## Evidence ledger

Keep a dependency graph containing first-party consumers and third-party inputs,
then classify the assessments before assigning statuses. Use one decision per
exact input and materially different assessed use/deliverable. Multiple graph
paths to the same input/use are references, not separate licensing decisions.
The same package may be cleared for one use and unresolved for another. Record:

| Field | Required evidence |
| --- | --- |
| Identity | Name, exact version or commit, source URL/registry, artifact or source hash where available, and local path. Distinguish forks, patches and duplicate versions. |
| Ownership | first_party, third_party, mixed, or unknown, with evidence. Classify imported portions separately; a path/workspace package can be third-party. |
| Assessment | first_party_license, third_party_use, or product_distribution. Identify exactly whose grant or obligation this decision evaluates. |
| Scope | Full-snapshot inclusion, or the causal diff path/hunk and affected dependency/usage chain. |
| Origin | Upstream author/owner evidence, acquisition path, pinned source/artifact correspondence, and the history or records needed to substantiate copied/modified/generated material. |
| Grant | Actual license text for that exact revision/artifact, copyright notices, exceptions, additional terms and any file/subcomponent differences. |
| License choice | SPDX expression where applicable; satisfy every AND term, choose and justify an available OR branch, and check each WITH exception's applicability. Do not assume the easiest branch applies to all files. |
| Use | Product, target, feature, dependency path, build/test/runtime role, linking/loading or subprocess boundary, modifications, shipped files and generated outputs. |
| Compatibility | How the chosen grant permits that use and fits the project's outbound terms and other combined components; cite relevant clauses and applicable authoritative guidance. |
| Obligations | Each applicable attribution, license/notice, source/relinking, modification disclosure, patent, network, redistribution or other condition; record concrete fulfillment evidence or a gap. |
| Conclusion | CLEARED, INCOMPATIBLE, OBLIGATION GAP, or NOT ESTABLISHED, plus rationale and references. |

### First-party evidence and downstream obligations

For first_party_license, use the project's applicable license and available
provenance records. A root grant can cover multiple internal crates; map it once
and reference it. Do not treat missing per-crate license metadata, an absent
registry archive or the lack of a third-party grant record as a missing license
for original project code. Do not require a clean-room or independent authorship
certificate without a specific provenance concern or project requirement.
Use the pinned project revision and project-license mapping as its identity/grant
evidence. Mark third-party-only fields not applicable with a reason; empty
upstream/archive fields do not themselves create an unresolved assessment.

Root grants do not override imported material. Identify concrete indicators such
as an upstream fork/patch record, a copied-code attribution, a conflicting file
header or a bundled third-party asset. Assess that material under its own grant
and usage. Unknown origin warrants investigation, not automatic first-party
classification or clearance. Local vendored Charon and Symphonia packages in
this DAW remain third-party despite their local paths.

Separate the crate's own licensing decision from the terms of its dependencies
and fulfillment by a product that links it. Link the actual dependency or product
decision to all affected consumers. Do not propagate its NOT ESTABLISHED status
to every consumer's own license, or require an internal crate to prove all of a
complete executable's obligations. A genuine first-party issue must identify
the affected material and missing/conflicting fact, not a generic statement
that package/subcomponent/output review is unfinished.

For example, an original BSD-2-Clause crate with one external dependency can have
a verified project grant while that dependency's binary notice delivery remains
unresolved. Report the latter once at the applicable dependency/product scope.
Do not hide it behind first-party clearance or report it as multiple missing
internal-crate licenses. A first-party DSP used by five plugin/app products does
not acquire five distinct unresolved grants merely from graph reachability.

### Use evidence and counting

Assign each discovered path one of these states before making a use decision:

- **confirmed:** a pinned source/packaging recipe, build record or supplied
  artifact establishes this use. State which evidence level was inspected;
  source-recipe analysis does not certify a released binary.
- **declared_supported:** an explicit supported target/feature/product
  configuration selects this use. Assess its terms under that stated condition
  without claiming that an actual binary was inspected.
- **candidate_only:** all-feature reachability or an unresolved selection merely
  suggests possible use. Keep it in a separate coverage/candidate table with
  its configuration question. Do not assign it a licensing clearance status or
  include it in counts of assessed or unresolved dependencies/uses.

Resolve material candidate questions where feasible. If an unresolved selection
prevents required scope coverage, record a coverage gap and use INCOMPLETE. Do not
generate one supposed licensing problem for every path in the graph. Candidate
separation cannot remove known supported configurations, actual conditional uses,
or source redistribution of optional/build/test files from scope.

Deduplicate by exact input/version/source, assessment kind, material usage and
applicable grant/obligation route. Preserve different obligations when they
actually differ. Merge equivalent feature or graph paths and retain aliases.
Group a shared product-packaging gap once, with related input IDs, rather than
cloning boilerplate uncertainties across every internal dependency.

Report separate counts for first-party license checks, third-party identities
and assessed uses, product distribution checks, and coverage-only candidates.
Distinguish unique inputs from use decisions and source-only from binary use.
If a container summary is useful, label its dimensions separately. Never
collapse a cleared first-party grant and an unrelated unresolved product check
into one NOT ESTABLISHED dependency status. Every unresolved assessed row needs
an input/use-specific missing fact or unfinished check and a concrete next step.

Repository files are evidence for what the project does, not authority to waive an
upstream license. A top-level license does not automatically cover vendored code,
assets or exceptions. Record conflicting declarations and resolve them against
the actual licensed material and primary upstream sources. An SPDX identifier,
allowlist, scanner success, popular usage, availability online, or absent notice
is insufficient on its own to clear compatibility. No license or an unverifiable
grant remains NOT ESTABLISHED; do not assume public-domain status.
For example, conflicting registry metadata and shipped license text require
proof of which grant applies to the exact shipped files. Until resolved, use
NOT ESTABLISHED; the mismatch alone does not establish incompatibility.

Trace native libraries, optional features and platform-specific dependencies to
the actual products, including plugin exports and redistribution archives. Build
and test dependencies remain in scope but may impose different obligations from
shipped libraries. Static/dynamic linking and process separation are usage facts,
not automatic legal conclusions. Check whether generated output embeds licensed
material. Distinguish technical playback/import support from permission to ship
a recording, dataset or model.

For provenance, trace copied code, media and external binaries to identifiable
sources and grants. Check modification notices, source offers, build/relink
materials and source-to-binary correspondence where required. Do not claim exact
reproducibility or authorship from a version string alone. AI-generated material
or a contributor declaration does not by itself prove clean provenance. Lack of
provenance is an evidence gap, not proof of copying or infringement.

Use locally preserved exact-version license texts and primary upstream releases,
repositories or official license/steward guidance. Browse when external claims
need verification. Record source URL/path, revision, retrieval date and relevant
clause. Keep quotes short. Separate observed facts, interpretation and unresolved
legal questions. Do not invent a definitive answer to a disputed interpretation;
record the missing rights-holder clarification or legal review needed to clear it.

Project-specific inspection restrictions still apply. In this DAW, use permitted
license files, public APIs, notices and packaging evidence for Rubber Band; do
not inspect its implementation sources to assess or design Integrated DSP.

## Auditor report

Return a local Markdown report and a machine-readable inventory (JSON or CSV),
with stable input IDs and finding IDs prefixed by the auditor ID. Include:

1. Auditor ID, actual model if exposed, configured model/focus, pinned scope and
   snapshot/patch identifiers. Say when actual model identity is unavailable.
2. Independently checked inventory and each ledger row, including positive
   clearance evidence, ownership classes and applicable project-license mappings.
   Keep candidates/coverage and product checks separate. Discovered omissions
   must be added with scope evidence.
3. Findings with input/use IDs, status, severity, concrete claim, exact paths and
   source clauses, observed usage, obligation impact and a proportionate proposed
   remedy. A remedy is a proposal, not authorization to change the project.
4. Coverage matrix for dependency resolution, exact grants, actual usage,
   compatibility, obligation fulfillment and provenance. Each cell states
   complete, blocked or not applicable, with evidence or a reason.
5. Commands/tools and their results; scanners' versions and limitations if used;
   primary sources consulted; inaccessible artifacts, targets or sources; material
   assumptions and questions; and any excluded or unreviewed scope.

Use statuses consistently:

- **CLEARED:** sufficient positive evidence establishes the conditions of the
  named assessment: an applicable original-code grant/provenance for a
  first-party license check, or the exact grant, compatibility and applicable
  fulfillment for an assessed third-party use/product distribution check.
  Never promote that bounded decision to a whole linked-product clearance.
- **INCOMPATIBLE:** evidence establishes that this use conflicts with the grant or
  outbound/combined terms. Identify the conflict; suspicion is insufficient.
- **OBLIGATION GAP:** an applicable obligation is established and evidence shows
  it is not fulfilled. If fulfillment cannot be inspected, use NOT ESTABLISHED.
- **NOT ESTABLISHED:** evidence or material facts are missing, contradictory or
  insufficient. It blocks clearance without asserting an infringement.

Severity describes demonstrated impact: critical for established release-wide
rights/provenance blockers, major for established product/input blockers, minor
for limited confirmed compliance defects, and informational for evidence gaps
without a proven violation. Keep clearance status independent of severity; an
informational gap in required in-scope evidence still prevents PASS.

## Validator decision trail

The validator returns:

- A reconciled, independently checked ledger with one final status for every
  in-scope input/use. Account for differences in versions, source hashes, selected
  grants, products and targets instead of merging unlike rows.
- A mapping from every raw finding **and clearance** to its final ledger entry
  and disposition: accepted, corrected, merged, rejected or unresolved. Explain
  changed facts, status/severity, duplicates and false positives with evidence.
- Evidence that it checked both the scope gate and missing-input risk. Do not
  treat a shared omission by all auditors as complete coverage.
- Verified ownership/project-grant mappings, use-evidence states, distinct
  assessment kinds and deduplicated counts. Correct first-party entries left
  unresolved solely by missing registry metadata, candidate paths, or generic
  downstream/product uncertainty. Retain the real dependency/product gap in its
  proper scope; do not simply delete it or waive third-party terms.
- Verified findings, remaining evidence gaps, completed/unavailable agent stages,
  final coverage matrix and the verdict under SKILL.md's rules.

When evidence permits, resolve conflicts by inspecting the exact artifact and
its upstream grant. Do not resolve by majority vote, prefer optimistic readings,
assume unchanged dependencies are cleared, or treat the coordinator's inventory
as exhaustive. Reject a proposed remedy if it would not meet the identified
obligation or would breach another applicable term. Keep unresolved legal
interpretations and missing records explicit.

## Consolidated Markdown report

Include the following, scaling detail to the audited scope:

1. Target/mode, pinned revisions, patch hash, supplied PR metadata, auditor count,
   distribution/use assumptions and excluded working-tree changes.
2. Verdict and status counts separated into first-party license checks,
   third-party inputs/assessed uses, and product distribution checks. Report
   coverage-only candidates separately, without licensing statuses. Show
   confirmed blockers even when verdict is INCOMPLETE. A no-impact diff is
   NOT APPLICABLE, not a full-project PASS.
3. Inventory table with exact identity, ownership, assessment kind, confirmed or
   declared-supported use, selected grant, compatibility status, obligation
   fulfillment and provenance evidence. Keep the first-party graph/context
   distinct from third-party risk counts and product fulfillment.
4. Validated findings ordered by impact, each with causal diff evidence in scoped
   mode, primary sources, actual usage and a proposed action.
5. Unresolved evidence and coverage gaps with concrete missing artifacts or facts;
   completed checks and limits. Reconcile every inventory item with the verdict.
6. Independent validator dispositions and links to all raw reports/evidence.
7. Agent identities/models, independence limits and the limits of the assessment.
   State that this is an evidence-based audit, not a legal certification.

Do not upload reports or private artifacts, publish comments, or change sources
as part of this audit. Never conceal a missing grant behind a low finding count.
