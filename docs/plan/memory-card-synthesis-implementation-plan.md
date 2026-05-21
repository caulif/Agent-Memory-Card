# Memory Card Synthesis Implementation Plan

Date: 2026-05-20
Tracking: GitHub issue #38

## Confirmed Task

Implement the first production slice of value-directed Memory Card synthesis:

- Memory Cards remain the single user-facing concept.
- Each generated card must explain its target value, not merely summarize history.
- Skill-targeted Memory Cards should show how they improve an existing Skill/workflow.
- Duplicate or already-covered content should merge or be rejected rather than generate clutter.

## Scope

### In Scope

- Add value metadata to extraction records:
  - `card_function`
  - `value_claim`
  - `value_delta`
  - `target_context`
  - `synthesis_trace`
- Upgrade candidate maturation provider prompt and deterministic fallback.
- Store value metadata on approved Memory Cards.
- Surface value claim/delta in Drafts review.
- Surface Skill-targeted before/after context in Skills.
- Add writing guide prompt resource.
- Add focused Rust and UI/contract tests.

### Out of Scope For This Slice

- Full autonomous ReAct runtime.
- Arbitrary shell/web tool execution from the app.
- SQLite FTS/embedding history index for synthesis runtime global observation retrieval.
- Direct Skill file mutation without Memory Card review.

## Agent Runtime Reuse Constraint

When the implementation grows beyond this first synthesis contract, do not hand-roll a loose agent loop. Use `earendil-works/pi` as the reference shape:

- stateful agent context instead of ad hoc prompt strings
- context transform before provider calls
- typed tools with preflight and postprocess hooks
- event/trace stream for UI and audit
- explicit stop conditions instead of fixed tool-call limits
- read-only tools by default for synthesis
- progressive Skill discovery: list metadata first, load full `SKILL.md` only for likely project-level targets
- append-only session/trace entries so retries and merge decisions are auditable

This slice should prepare compatible metadata (`synthesis_trace`, value tools, stop reasons) without prematurely implementing the full runtime.

Before implementing a full agent runtime, make an explicit build-or-adapt decision:

1. Prefer embedding or wrapping `@earendil-works/pi-agent-core` if the runtime can stay in the TypeScript/Tauri boundary without weakening Rust ownership of Memory Card writes.
2. If Rust must own the loop, port the pi contracts directly: `SynthesisSession`, `SynthesisMessage`, `transform_context`, `convert_to_provider_messages`, typed read-only tools, pre/post tool hooks, event stream, and stop-condition evaluator.
3. Avoid a one-off loop that mixes retrieval, prompting, tool execution, validation, and persistence in one service. That would violate the project's S.U.P.E.R boundaries and make later replacement expensive.

## Phase Breakdown

### Phase 1: Metadata Contract

- Extend Rust `ExtractionMetadata`.
- Extend TypeScript domain types.
- Add stable defaulting/backward compatibility.

Acceptance:

- Existing YAML without new fields still loads.
- New approved Memory Cards persist value fields.

### Phase 2: Value-Oriented Synthesis

- Provider rewrite prompt requests value-directed output.
- Deterministic fallback renders `Use When / Instructions / Boundaries / Value Delta`.
- Merge/already-covered semantics are reflected in metadata and UI text.

Acceptance:

- No chatty `/goal` or "this candidate" wording in mature cards.
- `value_delta` explains existing behavior, missing part, new behavior, and duplicate boundary.

### Phase 3: Review UX

- Drafts detail shows value claim and value delta.
- Merge/already-covered recommendation is visible.
- Skills page shows before/after-oriented context for mounted Skill-targeted Memory Cards.

Acceptance:

- User can see why a card is useful before approving or mounting.
- UI still follows one-page-one-job reduction principles.

### Phase 4: Verification

- Unit tests for deterministic metadata.
- Existing frontend contract/build tests.
- Rust fmt/clippy/lib tests.
- Update issue #38 with evidence.

## Execution Notes

- Keep the write boundary unchanged: candidate approval writes Memory Cards only after user action.
- Prefer additive metadata over schema-breaking changes.
- Preserve current PR #35 and push additional commits to the same branch.

## Phase 5: Tool-Backed Runtime Foundation

Tracking: GitHub issue #39

Goal: move beyond prompt-only maturation by adding a typed read-only synthesis runtime foundation.

Implemented scope:

- Add `src/synthesis_agent.rs` with `SynthesisReview`, `SynthesisContextPack`, typed tool events, proposal, value delta, target context, and stop reason.
- Implement read-only local tools:
  - observation search
  - project/global Memory Card comparison
  - project/global Skill metadata comparison
  - built-in writing guide read
  - duplicate target classification
  - project Skill gap classification
- Integrate `SynthesisReview` into candidate maturation:
  - provider prompt receives `synthesis_context`
  - deterministic fallback uses runtime value metadata
  - runtime merge recommendations can become approval-time `merge_into_existing`
  - compact trace is appended to existing synthesis trace
- Preserve human approval as the only write boundary.

Acceptance:

- Runtime does not write files.
- Project Skills can be targeted; global Skills are reference-only.
- Near-duplicate Memory Cards recommend merge.
- Skill-targeted feedback recommends the matching project Skill.
- Existing provider and local fallback paths remain compatible.

Deferred but now bounded:

- Provider-driven tool-call loop using the same `SynthesisReview` contract. Tracking: GitHub issue #41.
- Web search/read tools with citations and privacy-safe abstract queries. Tracking: GitHub issue #45.
- FTS/Embedding-backed global Observation retrieval is explicitly paused by user decision. Tracking issue #42 is closed; reopen only if lexical/read-only history windows prove insufficient in real use.

### Phase 6: Global Vision Context Quality

Tracking: GitHub issue #39 continuation

Goal: make the read-only runtime explain broader project history before proposing, merging, or rejecting a Memory Card.

Implemented scope:

- Split observation retrieval into direct evidence and broader history context.
- Add `search_global_history` events for same-source context and related historical feedback.
- Add `summarize_workflow_failures` events that reuse the existing failure-flow detector.
- Extend `SynthesisContextPack` with `related_observations` and `workflow_failures`.
- Keep no-context proposals conservative: if no direct or broader local context exists, stop as `needs_human`.

Acceptance:

- Runtime trace can explain when it read broader history rather than only the current candidate.
- Repeated workflow failure signals become structured review context.
- Same-source historical observations are available even when they are not strong direct evidence.
- No file writes, project config mutation, or web/provider loop is introduced in this slice.

### Phase 7: Explainable Skill and Memory Card Comparison

Tracking: GitHub issue #38 continuation

Goal: make read-only comparison tools explain the reason for their matches, not only return ids and scores.

Implemented scope:

- Extend `MemoryCardMatch` with `overlap_summary`, `gap_summary`, and `merge_hint`.
- Extend `SkillMatch` with `coverage_summary`, `gap_summary`, and `target_role`.
- Update `search_memory_cards` trace summaries to name the top match and whether it is a merge, already-covered, related-reference, or reference-only result.
- Update `search_skills` trace summaries to distinguish project Skill targets from global/referenced Skill context.
- Move comparison wording helpers into `src/synthesis_agent/explain.rs` so the runtime file remains below the project file-size limit.

Acceptance:

- Provider-facing `synthesis_context` can explain why a Memory Card is a duplicate, merge candidate, related reference, or global-only reference.
- Provider-facing `synthesis_context` can explain whether a Skill is a project-level target or only a global reference.
- Runtime still cannot write files or mutate Skills/Memory Cards.
- Focused synthesis tests cover the new explanation fields.

### Phase 8: Provider Tool-Call Loop

Tracking: GitHub issue #41

Goal: replace the deterministic one-pass planner with an observable provider-driven read-only tool-call loop while preserving the `SynthesisReview` output contract.

Acceptance:

- Provider loop can produce the same review actions as the deterministic planner.
- Tool calls are typed, capped, traceable, and read-only.
- Deterministic fallback remains available.
- No write boundary is moved out of Review Inbox approval.

### Phase 9: Privacy-Safe Web Tools

Tracking: GitHub issue #45

Goal: add optional Web search/read tools for external grounding without leaking private conversation excerpts.

Acceptance:

- Web queries are abstracted, not raw user quotes.
- Web citations include URL and retrieval date.
- Web evidence cannot be the sole basis for a project preference Memory Card.
- Web traces remain compact in Review Inbox/provider context.

### Phase 10: Paused FTS / Embedding Observation Search

Tracking: GitHub issue #42, closed by user preference

Decision: do not implement this now. The synthesis runtime should first rely on explainable read-only tools: direct observation evidence, broader lexical/history windows, workflow failure summaries, Memory Card comparison, and Skill comparison.

Acceptance:

- No FTS/Embedding retrieval work is included in the current roadmap.
- No UI, provider prompt, or runtime behavior should imply this feature is required.
- If future real usage shows lexical/read-only history context is insufficient, create a new Chinese GitHub issue with concrete missed-recall examples before implementation.

### Phase 11: Review Inbox Trace Presentation

Tracking: GitHub issue #43

Goal: expose synthesis trace explanations in Review Inbox without turning the page back into a diagnostics cockpit.

Acceptance:

- Users can see value, evidence, duplicate/merge reasoning, and target context in one screen.
- No chain-of-thought is displayed.
- verify-ui prevents diagnostics-panel regressions and horizontal overflow.

### Phase 12: Skill-Targeted Counterfactual Evaluation

Tracking: GitHub issue #44

Goal: verify that Skill-targeted Memory Cards improve a target Skill instead of restating it.

Acceptance:

- Before/after checks reject proposals without meaningful future behavior change.
- Duplicate Skill content routes to `already_covered`, `merge`, `ignore`, or `needs_human`.
- Metrics stay in advanced/eval surfaces, not the default review flow.
