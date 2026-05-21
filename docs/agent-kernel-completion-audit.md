# Agent Memory Kernel Goal Completion Audit

> Date: 2026-05-15  
> Goal: improve the project in the requested order: user loop -> Golden Set quality gate -> Memory Card governance -> module boundaries.  
> Status: not complete. The current state is materially better, but several requirements are still incomplete or weakly verified.

## Success Criteria

1. **User loop is reviewable end to end**: the user can move from extracted suggestions to evidence review, assignment, artifact diff preview, drift recovery, and sync without guessing the next step.
2. **Golden Set constrains quality**: extraction changes are gated by a deterministic golden regression with positive recall, negative precision, and enough representative cases to catch regressions.
3. **Memory Card governance prevents library rot**: cards expose source, assignment, duplicate/conflict signals, merge workflow, and lineage before future edits.
4. **Engineering boundaries are clearer**: artifact preview, review workbench, memory governance, and UI panels have separated responsibilities with focused tests.
5. **Verification covers the claims**: relevant frontend tests, Rust tests, Golden Set eval, and build all pass.

## Prompt-To-Artifact Checklist

| Requirement | Evidence | Status |
| --- | --- | --- |
| Rework user closure loop | `app/src/utils/review-workbench.ts` provides `buildReviewClosureState`, evidence summaries, artifact preview summaries, and drift recovery actions. `app/src/components/pages/Drafts.tsx` consumes these read models. | Mostly complete |
| Evidence review before approval | Candidate/draft evidence summaries are tested in `app/src/utils/review-workbench.test.ts`. | Complete for summary-level evidence; source drilldown still shallow |
| Artifact diff before write | Backend `src/build/artifact_preview.rs` emits structured `ArtifactPreviewRow` with `status`, hashes, `diff_preview`, `diff_lines`, and `diff_truncated`. Frontend `ArtifactPreviewStrip.tsx` renders expandable diffs. | Complete for generated artifact preview; large-file UX still basic |
| Drift recovery | `ArtifactPreviewSummary.recoveryActions` maps blocked drift to `import_artifact_drifts`, `keep_artifact_drifts`, and `discard_artifact_drifts`; discard is marked destructive and requires explicit browser confirmation. Structured `drifted` / `unmanaged` targets are grouped as blocking files with diff snippets, truncation hints, and page controls. `kernel-plan.test.ts` verifies the plans. Rust core tests verify keep/discard clear drift safely. | Mostly complete; per-file operations are still absent |
| Golden Set regression | `tests/golden/golden_set.yml` has 17 positives / 28 negatives. `tests/golden_set_regression.rs` gates minimum size plus recall/precision. | Mostly complete for deterministic layers |
| Golden Set CLI report | `cargo run --quiet -- eval --golden-set --project .` reports 100% positive recall, 100% negative precision, 0% one-off false positive, 29% duplicate cluster risk, and 88% final evidence validity via deterministic induce stub. `tests/eval_golden_report.rs` verifies report shape, minimum size, evidence validity floor, optional provider-induced evidence validity, and hallucinated provider evidence rejection. | Mostly complete for deterministic report; real configured provider run still should be captured |
| Memory Card governance summary | `app/src/utils/memory-governance.ts` computes assigned/unassigned, missing source, needs review, conflicts, dormant, and expired. UI shows these in `MemoryCards.tsx`, supports governance filters, and `MemoryGovernancePanel.tsx` exposes mark-dormant / mark-expired / restore-active actions with the target card id included. | Mostly complete |
| Duplicate/conflict handling | Governance detects likely duplicate cards, exposes conflict details with similarity labels, and provides `mergeDraftAction` using `fuse_memory_cards_to_draft`, verified by `memory-governance.test.ts` and `kernel-plan.test.ts`. | Mostly complete |
| Lineage visibility | Governance exposes approved source, evidence summary, merge history; `MemoryGovernancePanel.tsx` renders a source chain. | Mostly complete; no dedicated lineage page |
| Batch governance workflow | `buildMemoryGovernanceBatchActions` creates conservative batch restore-active and batch merge-draft steps for the current governance filter. `MemoryGovernanceBatchBar.tsx` renders the batch actions and still executes each step through existing `ProjectAction` / Kernel policy. | Mostly complete; execution feedback is still coarse |
| Module boundaries | Backend artifact preview split to `src/build/artifact_preview.rs`; artifact drift import/keep/discard split to `src/build/artifact_drift.rs`; Memory Card assignment read model split to `src/memory_card/target_matrix.rs`; Memory Card merge write path split to `src/memory_card/merge.rs`; global promote/install split to `src/memory_card/global.rs`; edit/update path split to `src/memory_card/edit.rs`. Frontend panels split to `components/review/ArtifactPreviewStrip.tsx`, `components/review/ArtifactDriftGroup.tsx`, `components/memory/MemoryGovernancePanel.tsx`, and `components/memory/MemoryGovernanceBatchBar.tsx`. Pure read-model logic lives in `utils/review-workbench.ts` and `utils/memory-governance.ts`. | Partial: extract and major page feature directories remain |
| Verification | Latest observed commands: cluster unit tests passed 19/19, Golden Set regression passed 3/3, Golden Set eval report passed 4/4, Golden Set CLI reported 100%/100% with 88% evidence validity, Rust lib passed 232/232, Tauri passed 42/42, frontend utility tests passed 27/27, and `bun run build` passed. | Strong for touched paths |

## Current Evidence Snapshot

- `cargo run --quiet -- eval --golden-set --project .`
  - positive recall: 100% (17/17)
  - negative precision: 100% (28/28)
  - one-off false positive: 0% (0/28)
  - duplicate cluster risk: 29% (5/17)
  - evidence validity: 88% (15/17)
- `cargo test extract::cluster::tests --lib -- --nocapture`
  - 19 pass
- `cargo test --test golden_set_regression -- --nocapture`
  - 3 pass
- `cargo test --test eval_golden_report -- --nocapture`
  - 4 pass
- `bun test ./src/utils/review-workbench.test.ts ./src/utils/memory-governance.test.ts ./src/utils/kernel-plan.test.ts`
  - 23 pass
- Recent full Rust lib verification:
  - `cargo test --lib -- --nocapture`
  - 232 pass
- Recent frontend verification:
  - `bun test ./src/utils/review-workbench.test.ts ./src/utils/memory-governance.test.ts ./src/utils/kernel-plan.test.ts`
  - 27 pass
  - `bun run build`
  - pass
- Recent desktop verification:
  - `cargo test --manifest-path src-tauri/Cargo.toml -- --nocapture`
  - 42 pass

## Missing Or Weakly Covered

1. **Golden Set size**: current 45 total cases is within the requested 40-60 stability band, but still needs more real rejected examples from user review history before it is representative.
2. **Evidence validity depth**: Golden Set now checks final crystallized-card `evidence_quotes` with a deterministic induce stub and has an optional provider-induced evidence validity path, but a real configured provider run has not yet been captured in this audit.
3. **Drift resolution UX**: import-as-draft, keep-current, and discard-manual-edit are implemented; discard has explicit destructive confirmation, and structured drift targets are grouped with snippets and pagination. Per-file drift actions are still missing.
4. **Batch governance workflow**: duplicate/lifecycle/lineage capabilities, governance filters, and conservative batch actions are present, but batch execution feedback is still coarse and there is no dedicated merge-review page.
5. **Module boundaries**: several large modules remain: extraction modules and page-level UI. Boundaries are improved but not finished.

## Decision

Do not mark the goal complete yet. The project now has a much stronger product loop and quality gate, but the audit still finds incomplete deliverables. The next highest-value work should be one of:

1. Run and capture Golden Set provider evidence validity with a real configured provider, then add any failures back as fixtures.
2. Add per-file operations for large drift sets.
3. Continue module boundary split around `memory_card` or frontend feature directories.
