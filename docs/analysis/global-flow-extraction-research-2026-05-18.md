# Global Flow Extraction Research

Date: 2026-05-18

Scope: improve Memory Card extraction quality by adding a conversation-level view above the existing line/chunk-level extraction path. This document contains distilled engineering principles only. It does not include private conversation text, local history excerpts, or raw Obsidian note content.

## Current Gap

The current extraction path already has useful local mechanisms:

- methodology-line prefiltering in `src/observation/chunked.rs`
- deterministic high-value templates in `src/extract/candidate_factory.rs`
- local gates in `src/extract/gate.rs` and `src/extract/quality_gate.rs`
- LLM induction and deterministic crystallization in `src/extract/induce.rs` and `src/extract/crystallize.rs`

The missing layer is a global conversation-flow model. Some durable memories are not expressed as one reusable sentence. They emerge from the whole collaboration arc: how a task starts, how the user corrects the agent during implementation, and how the user judges final quality. Local chunks can miss those patterns or reduce them to noisy task-status sentences.

## Inputs Reviewed

Local architecture and quality docs:

- `docs/archives/agent-kernel-production-tooling/plan/extraction-system-optimization-roadmap.md`
- `docs/extraction-function-optimization-analysis-2026-05-18.md`
- `docs/superpowers/plans/2026-05-03-high-quality-extraction.md`
- current extraction modules under `src/observation/` and `src/extract/`

Local Obsidian notes, distilled only:

- `C:\obsidian\AI与Agent\Agent基础\Workflow vs Agent.md`
- `C:\obsidian\AI与Agent\Agent基础\Agent Loop.md`
- `C:\obsidian\AI与Agent\Agent基础\Agent Evals.md`
- `C:\obsidian\AI与Agent\Agent基础\结构化输出.md`
- `C:\obsidian\项目与产品\工程方法\工程验证闭环.md`
- `C:\obsidian\项目与产品\工程方法\模块边界.md`

External references:

- [Anthropic, "Building effective agents"](https://resources.anthropic.com/building-effective-ai-agents): workflows are predefined code paths; agents dynamically direct tools/processes. The recommendation is to start simple and compose predictable workflow patterns before adding autonomy.
- Anthropic, "Demystifying evals for AI agents": agent evals should make behavioral changes visible before they hit users, and should evaluate multi-turn trajectories, not only final output.
- [OpenAI, "Structured Outputs"](https://platform.openai.com/docs/guides/structured-outputs): schema-constrained outputs are preferable when model output becomes a downstream engineering interface.
- [GitHub Blog, "Agent pull requests are everywhere. Here's how to review them."](https://github.blog/ai-and-ml/generative-ai/agent-pull-requests-are-everywhere-heres-how-to-review-them/): agent-generated work should be reviewed for CI/test changes, overbroad edits, hidden risks, and evidence of real verification.
- [RAGAS paper](https://arxiv.org/abs/2309.15217): evaluation of retrieval/generation systems should separate evidence relevance, grounding, and answer quality rather than compressing everything into one score.

## Design Principles

1. Workflow-first extraction.

   Conversation-flow extraction should be a deterministic workflow with bounded LLM judgment, not a free-form agent loop. The stages should be explicit: segment, summarize, infer candidate, gate, merge, render, evaluate.

2. Structured output is an interface, not formatting.

   The global layer must emit a typed structure such as `ConversationFlowSummary` and `FlowInferredCandidate`. If an LLM participates, its output must be schema-validated and business-validated before entering the candidate pool.

3. Separate local evidence from global inference.

   A local candidate says "this sentence looks like a rule." A global candidate says "this cross-stage pattern suggests a durable workflow." They should have different metadata and different confidence behavior.

4. Evidence-bound inference.

   A global candidate can be inspired by the full conversation, but it still needs supporting spans. Inference without evidence should become `review_only` or be rejected as unsupported.

5. Start/middle/end positions have different semantic priors.

   In many project conversations:

   - early turns often contain task framing, planning style, reference-search expectations, and long-term constraints
   - middle turns often contain implementation preferences, correction patterns, and collaboration cadence
   - later turns often contain testing expectations, acceptance criteria, and quality judgments

   The extractor should preserve these priors instead of treating every line as equivalent.

6. Quality is a human-facing outcome.

   Metrics are necessary but insufficient. The final generated cards must be read as future agent instructions: clear trigger, concrete action, boundary, evidence support, and no leakage of current execution chatter.

7. Privacy boundary is strict.

   Real local histories and Obsidian notes are validation inputs only. They must not be uploaded to GitHub, committed to Golden Set, or embedded in public fixtures. Failures discovered from real data should be converted into synthetic or fully anonymized tests.

## Target Architecture

```text
ObservationRecord[]
  -> ConversationFlowSegmenter
  -> ConversationFlowSummary
  -> FlowCandidateFactory
  -> FlowCandidateQualityGate
  -> merge with local chunk candidates
  -> balanced ranking
  -> existing Candidate/Draft/Card path
  -> synthetic tests + local real-history dry-run
```

## Candidate Types

`local_signal_candidate`

- Source: one sentence or compact chunk.
- Strength: high when user directly states a reusable rule.
- Risk: misses multi-turn patterns.

`flow_inferred_candidate`

- Source: cross-stage summary.
- Strength: captures startup workflow, development correction patterns, and delivery acceptance standards.
- Risk: can over-infer from one task unless gated.

`accepted_offer_candidate`

- Source: assistant proposes a method and user accepts it.
- Strength: captures collaboration preferences that are not in the user's sentence alone.
- Risk: needs explicit acceptance evidence.

## Quality Rubric

A global-flow card should pass:

- Trigger: states when the future agent should apply it.
- Action: says what the agent should do or avoid.
- Boundary: says when not to over-apply it.
- Evidence: has at least one user direct span, accepted-offer span, or repeated behavior signal.
- Abstraction: neither too low-level task detail nor generic advice.
- Field consistency: kind/scope/memory tier align with body.
- Privacy: contains no raw local history, path-specific private detail, or generated agent-control text.

## Expected New Memories

The global layer should be able to surface memories like:

- At project startup or feature planning, establish the planning frame and inspect existing context before implementation.
- For UI, layout, architecture, and process discussions, use lightweight visual aids when they improve discussion quality.
- When relevant, research comparable open-source projects or products before proposing a plan.
- When another agent or GitHub plan exists, check current issues/PRs/plans first and fill gaps rather than duplicating work.
- When changing extraction quality code, validate with synthetic tests and local real-history dry-runs, then inspect final card text from the user's perspective.
- At delivery, treat green tests as evidence but also inspect whether the result satisfies the user's actual quality bar.

## Non-Goals

- Do not build a fully autonomous extraction agent.
- Do not upload real conversation data or Obsidian note contents to GitHub.
- Do not add broad UI work.
- Do not replace existing local chunk extraction; augment and merge with it.
- Do not make Golden Set depend on real local histories.
