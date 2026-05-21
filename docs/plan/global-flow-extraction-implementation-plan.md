# Global Flow Extraction Implementation Plan

Date: 2026-05-18

Goal: add global conversation-flow awareness to the extraction pipeline so Memory Card candidates can be inferred from the whole task arc, not only from isolated high-signal lines or chunks.

Privacy rule: real local histories and `C:\obsidian` notes are validation-only inputs. Do not commit them, upload them to GitHub, or add them to Golden Set. Convert any discovered failure into synthetic tests.

## Phase 1: Flow Model And Segmenter

Files:

- `src/observation/chunked.rs`
- optional new module: `src/observation/flow.rs`

Tasks:

1. Add typed flow structures:

   ```text
   ConversationFlowSummary
   ConversationFlowSegment
   ConversationStage = startup | exploration | implementation | review | closure
   FlowSignal = user_direct | accepted_offer | correction | repeated_pattern | validation_feedback
   ```

2. Build a deterministic segmenter over `ObservationRecord[]`:

   - preserve chronological order
   - identify early/middle/late windows
   - mark high-signal user turns, assistant offers followed by user acceptance, corrections, and final quality judgments
   - keep only compact snippets and observation IDs in memory

3. Add unit tests with synthetic conversations:

   - startup planning preference appears near the beginning
   - implementation correction appears in the middle
   - delivery acceptance appears near the end
   - subagent briefs and `/goal` execution chatter are excluded

Acceptance:

- Flow summary can be generated without invoking an LLM.
- The summary does not require or store raw private history outside the current run.
- Existing chunk tests still pass.

## Phase 2: Flow Candidate Factory

Files:

- `src/extract/candidate_factory.rs`
- optional new module: `src/extract/flow_candidate.rs`
- `src/extract/shared_impl.rs`

Tasks:

1. Add `flow_inferred_candidate` creation from `ConversationFlowSummary`.

2. Encode conservative templates for:

   - project startup planning
   - visual planning aids
   - reference/open-source research before planning
   - multi-agent/GitHub plan deduplication
   - real-history validation for extraction changes
   - delivery acceptance beyond green tests

3. Attach metadata:

   ```text
   matched_template = "global-flow:<name>"
   reason = "Inferred from cross-stage conversation flow with evidence."
   memory_tier = CollaborationPreference or CrossProjectPrinciple
   confidence lower than direct user-rule candidates unless evidence is strong
   ```

4. Add synthetic tests that prove the candidate body is a future rule, not a transcript summary.

Acceptance:

- A multi-turn startup conversation yields visual/reference planning candidates.
- A multi-turn extraction-quality conversation yields a validation workflow candidate.
- Current task execution instructions do not become candidates.

## Phase 3: Gate And Ranking Integration

Files:

- `src/extract/gate.rs`
- `src/extract/quality_gate.rs`
- `src/extract/shared_impl.rs`
- `src/observation/chunked.rs`

Tasks:

1. Add quality dispositions for global-flow candidates:

   - keep
   - review-only inferred
   - reject unsupported inference
   - reject current-task chatter

2. Require at least one of:

   - direct user preference
   - accepted assistant offer
   - repeated cross-stage pattern
   - final acceptance/quality correction

3. Update balanced ranking:

   - do not let local methodology snippets occupy every slot
   - preserve at least a small quota for startup/review/closure flow candidates when present
   - dedupe local and global versions by normalized body and intent

4. Add regression tests for false positives:

   - "改 prompt/render 后抽样看卡片" style internal process chatter
   - local thresholds such as "top 10 at least 3"
   - agent status narration
   - generated instruction artifacts

Acceptance:

- Global-flow candidates can survive the merge path.
- Weak global inference is review-only or rejected.
- Known noise remains filtered.

## Phase 4: Evaluation Workflow

Files:

- `tests/extract_quality_v2.rs`
- `tests/fixtures/extract_quality_v2.yml`
- optional CLI/report helpers under `src/observation/`

Tasks:

1. Add synthetic fixtures for whole-conversation flow cases.

2. Add a local-only validation checklist to docs:

   ```powershell
   cargo fmt
   cargo test --lib
   cargo test --test extract_quality_v2
   cargo run --quiet -- eval --golden-set --project "C:\Users\15893\Documents\New project"
   cargo run --quiet -- observe replay --project "C:\Users\15893\Documents\New project" --home $env:USERPROFILE --target codex --target claude-code --engine local --dry-run
   cargo run --quiet -- observe replay --project "C:\QianCi" --home $env:USERPROFILE --target codex --target claude-code --engine local --dry-run
   ```

3. When a real-history dry-run finds a miss or bad candidate, add a synthetic regression fixture instead of committing real text.

4. Optional: improve replay rendering to show full title/body/brief for manual inspection.

Acceptance:

- Synthetic metrics stay healthy.
- Real-history dry-runs are manually inspected before quality-related changes are called complete.
- No real history, Obsidian note body, or private conversation excerpt is added to Git.

## Execution Order

1. Implement Phase 1 flow summary first. This is the architectural boundary.
2. Add Phase 2 templates using synthetic tests.
3. Integrate Phase 3 gates and ranking.
4. Run Phase 4 verification and manually inspect generated cards.

## GitHub Tracking

Use a dedicated milestone: `Global Flow Extraction Quality`.

Suggested issues:

- `T1 Flow summary and stage segmenter`
- `T2 Flow-inferred candidate factory`
- `T3 Flow-aware quality gate and balanced ranking`
- `T4 Synthetic plus local real-history validation workflow`

These tasks are intentionally separate from the closed `Agent Memory Kernel Production Tooling` project plan.

