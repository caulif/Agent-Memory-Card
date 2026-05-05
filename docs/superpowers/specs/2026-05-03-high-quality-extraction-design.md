# High Quality Extraction Design

## Goal

Make high-quality extraction the core Agent-Kernel capability: from local Claude Code and Codex usage traces, produce a small daily Inbox of durable, evidence-backed Candidate/Draft records that can materially improve future agent behavior after human review.

The product optimizes for one advanced local developer. It should prefer silence over noise. A successful run may produce zero candidates when nothing durable was learned.

## Product Position

Agent-Kernel is a local-first Agent instructions and Skills governance compiler. Its core loop is:

```text
Local evidence -> High-value extraction -> Review Inbox -> Skilllet -> Target assignment -> Safe artifact compile
```

High-quality extraction is the kernel of that loop. The desktop UI should open to Candidate/Draft review, but that Inbox is only valuable if extraction is selective, explainable, and resistant to one-off task chatter.

## Durable Knowledge Definition

A record is durable knowledge when it can improve future agent behavior beyond the current task. It may come from user text, agent output, project artifacts, or AI synthesis, but it must satisfy all of these:

- It is actionable: a future agent can follow it.
- It is reusable: it applies across more than the current one-off edit.
- It is grounded: it has source evidence or a clear derivation from evidence.
- It is safe to review: secrets and irrelevant local noise are redacted.
- It is not already represented by an existing Skilllet or Draft.

AI-generated content may become a Candidate/Draft when it creates a high-value project improvement, such as a clearer workflow, a reusable architectural rule, a strong correction, or a normalized version of repeated local evidence. AI output must not bypass review or write directly to Skilllet.

## Extraction Funnel

The extraction engine should become a layered funnel:

```text
Observation
  -> EvidenceChunk
  -> Noise Gate
  -> Signal Gate
  -> Value Scoring
  -> Novelty/Dedup
  -> Evidence Bundle
  -> Candidate
```

### EvidenceChunk

An `EvidenceChunk` is the smallest unit that can be judged. It should carry:

- `id`
- `text`
- `source_kind`
- `source_observations`
- `agent`
- `project_path`
- `timestamp`
- `origin`, one of `user`, `assistant`, `artifact`, `ai-synthesis`, or `unknown`

The first implementation can derive chunks from existing observations by splitting paragraphs and nearby turns. It does not need to rewrite storage.

### Noise Gate

The noise gate rejects obvious non-knowledge before scoring:

- one-off task requests
- unresolved feature asks
- transient progress reports
- shell output, stack traces, logs, and command transcripts
- system/developer prompt material
- local command wrappers
- secret-heavy or path-heavy fragments after redaction
- generic praise or frustration without a reusable correction

Rejected chunks should be counted in quality reports, not shown in the daily Inbox.

### Signal Gate

The signal gate classifies retained chunks into durable signal types:

- `preference`: stable tool, library, style, or process preference
- `constraint`: prohibited or risky behavior
- `procedure`: reusable workflow or checklist
- `correction`: user correction that should prevent repeated failure
- `decision`: project architecture or product decision with future impact
- `supplement`: useful addition to an existing Skill
- `ai_project_improvement`: high-quality AI output that materially improves project behavior

The existing `preference`, `constraint`, and `procedure` kinds remain valid storage values. `decision`, `supplement`, and `ai_project_improvement` can initially normalize to `procedure` while preserving `matched_signal` metadata.

### Value Scoring

Every candidate should receive a transparent score. The initial score model is deterministic and local:

```text
score =
  durability
  + actionability
  + specificity
  + recurrence
  + correction_strength
  + evidence_quality
  + novelty
  - task_specific_penalty
  - vague_penalty
  - unresolved_request_penalty
  - duplicate_penalty
  - sensitive_content_penalty
```

The score should produce three dispositions:

- `reject`: do not create a Candidate.
- `borderline`: optionally send to an LLM provider when enabled; otherwise reject or keep in debug quality output.
- `candidate`: create a Candidate.

The default run should cap Candidate creation at 10 items, sorted by score and confidence.

### Novelty And Dedup

Novelty is computed against:

- existing Skilllets
- existing Drafts
- visible Candidates from the same run
- approved artifact-derived knowledge

The first implementation should keep Jaccard-based matching, but compare only compatible kind/scope/signal groups. A future embedding backend can replace the matcher behind the existing `SemanticMatcher` trait without changing product behavior.

### Evidence Bundle

Each generated Candidate should explain itself:

- source evidence excerpt
- source observation ids
- matched signal
- confidence
- reason
- score breakdown
- duplicate or similar record id when applicable
- origin, including `ai-synthesis` when the content was generated by AI

When the record is promoted to Draft and later approved to Skilllet, enough provenance should remain for the user to understand why it exists.

## AI Output Policy

AI output can be high-value source material. It should be eligible for extraction when it meets one of these cases:

- The AI proposes a durable project rule that the user accepts or builds on.
- The AI synthesizes repeated evidence into a clearer reusable workflow.
- The AI identifies a recurring failure pattern and proposes a prevention rule.
- The AI writes a high-quality Skill supplement that should be reused.
- The AI states an architectural/product principle that matches project direction and is later reinforced by user action.

AI output should be rejected when it is only:

- a task-specific implementation explanation
- a speculative suggestion not accepted by the user
- a long generic best-practice list
- a hallucinated rule without local evidence
- a summary of what just happened without future actionability

AI-origin Candidates require human approval before becoming Skilllets. The UI should label them clearly.

## Current Code Alignment

The existing code already contains useful anchors:

- `src/observation.rs` imports local conversations and synthesizes observations.
- `src/observation/conversation.rs` parses JSONL and filters prompt/command noise.
- `src/observation/incremental.rs` tracks append-only local import state.
- `src/extract/signals.rs` detects durable signal shapes.
- `src/extract.rs` creates Drafts from local and optional LLM extraction.
- `src/extract/embedding.rs` provides a `SemanticMatcher` and Jaccard default.
- `src/candidate.rs` stores reviewable system suggestions.
- `src/draft.rs` supports edit, merge, approve, and reject.
- `src/kernel/policy.rs` treats approval and non-preview compile as high-risk.

The improvement should extend these boundaries instead of replacing them.

## Data Model Changes

Add a small extraction metadata structure to Candidate and Draft records:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtractionMetadata {
    #[serde(default)]
    pub origin: String,
    #[serde(default)]
    pub matched_signal: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub source_observations: Vec<String>,
    #[serde(default)]
    pub score_breakdown: BTreeMap<String, f32>,
    #[serde(default)]
    pub similar_record: Option<String>,
}
```

Existing top-level fields such as `evidence`, `confidence`, `matched_template`, and `source_observations` should be preserved for compatibility. The metadata can mirror existing fields during migration.

## Quality Suite

Extraction quality should be tested with checked-in bilingual fixtures. The first suite should include:

- durable preferences
- durable constraints
- reusable procedures
- recurring corrections
- project decisions
- AI-origin project improvements
- Skill supplements
- one-off task requests
- unresolved requests
- shell/log noise
- system/developer prompt noise
- generic AI explanations

The quality report should include:

- true positives
- false positives
- false negatives
- precision
- recall
- precision at 10
- candidate count

Initial acceptance gates:

- `precision_at_10 >= 0.80`
- default candidate count per evolve run is at most 10
- every generated Candidate has evidence, reason, matched signal, and origin
- duplicate existing Skilllets do not enter the visible Inbox
- accepted AI-output fixtures become Candidates only when evidence-backed and actionable

## UI Impact

Inbox-first remains the default product entry. The Inbox should highlight why a record deserves attention:

- title
- body preview
- confidence and matched signal
- origin label
- reason
- evidence excerpt
- similar record warning

Deep inspection can remain in the editor. The first implementation can expose metadata through existing read models before adding visual polish.

## Non-Goals

- Do not add a mandatory vector database.
- Do not add MCP/A2A work in this phase.
- Do not make LLM extraction mandatory.
- Do not let AI output write directly to Skilllet.
- Do not broaden the product to team collaboration.

## Acceptance Criteria

- High-quality extraction has a documented deterministic funnel.
- AI-origin durable knowledge is eligible for Candidate/Draft creation but must go through review.
- Quality fixtures and tests measure precision, recall, and precision at 10.
- Existing local extraction remains the default.
- Optional LLM/provider behavior is bounded to prefiltered or borderline material.
- The Inbox receives fewer, better Candidates with explainable evidence.
