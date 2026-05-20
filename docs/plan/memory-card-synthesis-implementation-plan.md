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
- SQLite FTS/embedding history index.
- Direct Skill file mutation without Memory Card review.

## Agent Runtime Reuse Constraint

When the implementation grows beyond this first synthesis contract, do not hand-roll a loose agent loop. Use `earendil-works/pi` as the reference shape:

- stateful agent context instead of ad hoc prompt strings
- context transform before provider calls
- typed tools with preflight and postprocess hooks
- event/trace stream for UI and audit
- explicit stop conditions instead of fixed tool-call limits
- read-only tools by default for synthesis

This slice should prepare compatible metadata (`synthesis_trace`, value tools, stop reasons) without prematurely implementing the full runtime.

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
