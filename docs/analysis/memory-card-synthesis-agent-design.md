# Memory Card Synthesis Agent Design

Date: 2026-05-20
Status: production design, implemented in slices

## Goal

The extraction system should turn real collaboration history into mature, reviewable Memory Cards that improve existing project workflows and project-level Skills. It should not generate cards for their own sake. A new Memory Card is valuable only when it captures a concrete gap, correction, boundary, or optimization that the current Skill or workflow does not already express.

The architecture should stay workflow-first for filtering and validation. The synthesis layer can trust the LLM more: it may use multiple read/search tools, inspect broader conversation history, compare existing Skills and Memory Cards, and use web search when useful. The write boundary remains human review.

## Product Concept

Use one product concept everywhere: **Memory Card**.

Do not introduce a second user-facing concept such as "Skill Supplement" or "Workflow Patch". Those can exist internally as routing or rendering modes, but the user should always feel they are reviewing, approving, merging, and assigning Memory Cards.

Memory Cards can have different functions:

| Function | Purpose | User-facing wording |
|---|---|---|
| Library Memory Card | A standalone project or global rule for future work. | "Memory Card" |
| Skill-targeted Memory Card | A card whose value is to improve a specific project-level Skill. It can be mounted to that Skill or fused into that Skill's usable context. | "Memory Card for this Skill" |
| Workflow Memory Card | A card that improves a recurring project workflow even when no Skill currently owns it. | "Workflow Memory Card" |
| Merge Memory Card | A card proposal that should update an existing card instead of becoming a new item. | "Merge into Memory Card" |

The implementation may store `function` or `target.type`, but UI copy should keep Memory Card as the noun.

## Design Principles

### No Card Is A Success

The system optimizes for future usefulness, not extraction volume. Returning `ignore`, `already_covered`, `merge_card`, or `needs_human` is a successful result when a new Memory Card would add clutter.

The product should make this visible in review summaries:

- "Already covered by an existing Skill."
- "Merged into an existing Memory Card."
- "Ignored because the evidence was one-off."
- "Needs human judgment because the target workflow is unclear."

### Value Before Form

A Memory Card proposal must first prove its value, then render cleanly. Style polish cannot compensate for a weak or duplicate card.

The value question is:

> What will this Memory Card make the agent do better next time?

### Local Evidence First, Web Evidence Second

Conversation history, project Skills, existing Memory Cards, and user corrections outrank web sources. Web search can support framing, current facts, or established practice, but it must not override project-specific preferences without review.

The agent must not send private conversation excerpts as raw web search queries. Web queries should be abstracted, for example "agent memory card writing best practices" rather than a user quote.

## Non-Goals

- Do not make `C:\obsidian` a runtime dependency.
- Do not build a general autonomous research agent.
- Do not let the LLM write directly into `.agent-kernel/memory-cards` without review.
- Do not expose chain-of-thought. Show concise trace summaries, evidence, and tool calls instead.
- Do not turn the Drafts page back into a diagnostics cockpit.
- Do not create a Memory Card that merely restates an existing Skill, Memory Card, or obvious project convention.

## Design Inputs

- The local note `C:\obsidian\AI与Agent\Agent基础\Workflow vs Agent.md` frames the core boundary: fixed, evaluable paths should be workflows; open-ended, feedback-driven steps may use agents.
- Anthropic's agent guidance distinguishes predefined workflow paths from agents that dynamically choose process and tools. It also recommends starting simple and increasing complexity only when it improves outcomes.
- `earendil-works/pi` is useful as a reference for a lightweight agent base: stateful agent sessions, tool execution, event streaming, context transforms, preflight hooks, post-tool hooks, and stop conditions.
- Current Agent Memory Kernel already has observations, candidates, Memory Cards, Skills, provider calls, merge paths, and review UI. The next step is not a rewrite; it is a sharper synthesis layer above the existing pipeline.

## Mature Agent Reuse Constraint

Do not build the Memory Card synthesis agent as an ad hoc ReAct loop. If the product needs a true runtime, first evaluate whether the `earendil-works/pi` packages can be embedded or adapted. If direct reuse is not practical because this project keeps its core in Rust/Tauri, port the architecture shape rather than inventing a looser one.

The minimum compatible shape should mirror pi's mature boundaries:

| Pi pattern | Memory Card synthesis implication |
|---|---|
| Stateful agent context | Keep `SynthesisSession` state instead of rebuilding prompts from scattered strings. |
| `AgentMessage -> transformContext -> convertToLlm` | Separate product trace/context records from provider-ready prompt messages. |
| Typed tools with validated arguments | Expose `search_observations`, `read_skill_summary`, `find_memory_duplicates`, and web tools as schema-checked read-only tools. |
| `beforeToolCall` / `afterToolCall` hooks | Enforce read-only access, evidence caps, source citation, duplicate checks, and trace normalization around tool use. |
| Event stream | Emit compact reviewable events for the UI: reads, comparisons, duplicate decisions, stop reason. |
| Explicit stop conditions | Stop on enough evidence, already-covered proof, merge target found, insufficient evidence, or context pressure. Do not use a fixed tool-count cutoff as the main control. |
| Session tree / resumable history | Store synthesis attempts as append-only trace entries so review can explain what changed across retries. |
| Progressive skill loading | Scan Skills broadly, but load full `SKILL.md` details only for likely project-level targets. |

This constraint exists to keep the agent boring in the good way: observable, typed, bounded, resumable, and replaceable. The user should experience better Memory Cards, not a custom agent framework leaking into the product.

## Proposed Pipeline

```mermaid
flowchart LR
  A["Conversation history / observations"] --> B["Filtering Workflow"]
  B --> C["Candidate Cluster"]
  C --> D["Context Pack Builder"]
  D --> E["Gap & Value Analysis"]
  E --> F["Memory Card Synthesis Agent"]
  F --> G["Structured Proposal"]
  G --> H["Validation Workflow"]
  H --> I["Review Inbox"]
  I --> J["Approve / Merge / Attach to Skill"]
```

## Stage 1: Filtering Workflow

This stage should remain deterministic or bounded-LLM workflow logic. It answers: "Is this worth remembering?"

Responsibilities:

- Normalize imported conversation records into observations.
- Remove one-off commands, temporary status updates, current task details, and purely emotional chatter.
- Score durability, recurrence, user correction strength, evidence quality, and actionability.
- Cluster semantically similar observations before generating user-facing cards.
- Route the cluster to a likely output target:
  - `memory_card.project`
  - `memory_card.global`
  - `memory_card.skill_targeted`
  - `workflow_gap`
  - `ignore`
  - `needs_more_context`

Output shape:

```json
{
  "cluster_id": "cluster-...",
  "decision": "needs_more_context",
  "candidate_kind": "procedure",
  "scope": "project",
  "activation": "skill",
  "evidence": [
    {
      "observation_id": "obs-...",
      "quote": "User-visible supporting quote"
    }
  ],
  "signals": {
    "durability": 0.82,
    "recurrence": 2,
    "actionability": 0.9,
    "one_off_risk": 0.12
  }
}
```

## Stage 2: Context Pack Builder

This is not an LLM agent. It is a controlled retrieval step that builds the initial context the writer agent is allowed to see.

Default context:

- The selected candidate cluster and its evidence quotes.
- Nearby observations from the same session or time window.
- Existing Memory Cards with high lexical or embedding similarity.
- Project-level Skills and their mounted Memory Cards.
- Global Skills only as reference, not as default attachment targets.
- The project-local Memory Card writing guide described below.

Optional context:

- Additional conversation snippets found by project-local grep/search.
- Relevant generated artifacts such as `AGENTS.md`, `CLAUDE.md`, or skill indexes.
- Web information when the agent needs external grounding, current public facts, or examples of established practice to produce a better card.

Runtime should not scan `C:\obsidian`. If the Obsidian notes contain useful principles, distill them into the in-repo writing guide and ship that as stable product guidance.

## Stage 3: Gap & Value Analysis

This is the key product shift. The system should not ask "what card can I generate from this conversation?" It should ask "what existing workflow or Skill failed, and what targeted memory would prevent that failure next time?"

The analysis should compare the candidate cluster against:

- Existing project-level Skills and their `Use when / Instructions / Boundaries`.
- Existing project and global Memory Cards.
- Recent user corrections and failed/awkward UI or workflow moments.
- Repeated manual decisions that could become a reusable rule.
- External references when the right practice is unclear or current.

Possible outcomes:

| Outcome | Meaning |
|---|---|
| `ignore` | The cluster is one-off, already handled, or too vague. |
| `already_covered` | Existing Skill or Memory Card already expresses the behavior well enough. |
| `merge_card` | The value exists but belongs inside an existing Memory Card. |
| `skill_targeted_card` | A project-level Skill is missing a concrete instruction, boundary, or trigger, so the proposal should become a Memory Card for that Skill. |
| `workflow_card` | A recurring project workflow needs a Memory Card even if no Skill currently owns it. |
| `new_card` | No existing Skill or card covers the durable behavior. |
| `needs_human` | The agent found a possible gap but lacks enough evidence to write confidently. |

Value criteria:

- The proposal prevents a repeated failure, confusion, or low-quality output.
- The proposal improves a real Skill or workflow the user is likely to invoke again.
- The proposal is more specific than the existing Skill text, not a paraphrase.
- The proposal has evidence from history or credible external grounding.
- The proposal has a clear trigger and boundary.
- The proposal explains the delta between current behavior and improved future behavior.

Anti-value criteria:

- It repeats an existing Skill instruction with different wording.
- It summarizes what happened rather than changing future behavior.
- It captures a temporary implementation detail or current branch state.
- It only exists because the extractor was able to produce something.
- It adds broad advice without a target Skill, workflow, or failure mode.

## Stage 4: Memory Card Synthesis Agent

This is the agentic layer in the default proposal. It answers: "Given the evidence, existing Skills, existing Memory Cards, and optional external context, what targeted improvement should the user review?"

The agent should be trusted with exploration, but constrained by write boundaries and reviewability:

- Read-only tools by default.
- No filesystem writes.
- No shell commands except safe search/read commands exposed through dedicated tool wrappers.
- No fixed tool-call limit. The agent may continue searching while each step is justified by the value question.
- Stop when it can produce a grounded proposal, prove the issue is already covered, or explain why it needs human judgment.
- If it cannot identify a concrete improvement over existing Skills or cards, it returns `ignore`.

Minimal tool set inspired by pi-style agent foundations:

| Tool | Purpose | Boundary |
|---|---|---|
| `search_observations` | Find related conversation snippets by text, tag, session, or time range. | Read-only, returns snippets with IDs and quotes. |
| `read_observations` | Load specific observation records selected by ID. | Read-only, capped result size. |
| `search_memory_cards` | Find duplicate or related Memory Cards. | Read-only, includes similarity and merge hints. |
| `read_memory_card` | Load a candidate existing card before recommending merge. | Read-only. |
| `search_skills` | Find project and global Skills. | Read-only, marks source kind clearly. |
| `read_skill_summary` | Load SKILL.md metadata and short body summary. | Read-only, no full arbitrary filesystem access. |
| `read_writing_guide` | Load built-in card-writing rules. | Static in-repo document. |
| `web_search` | Find external references or current facts when local context is insufficient. | Read-only, cited sources required. |
| `web_read` | Read a selected external source. | Read-only, short summaries with URLs and retrieval dates. |
| `compare_with_skill` | Compare a proposal or cluster against a project Skill and identify missing trigger, instruction, boundary, or acceptance criteria. | Read-only, returns structured overlap and gap analysis. |
| `find_memory_duplicates` | Compare a proposal against existing Memory Cards and classify as duplicate, merge candidate, or distinct delta. | Read-only. |
| `summarize_workflow_failures` | Summarize repeated failure modes from selected observations. | Read-only, evidence IDs required. |
| `build_skill_target_context` | Build a concise target context for a Skill: purpose, existing Memory Cards, gaps, and recent failures. | Read-only. |

The value-oriented tools should be preferred over forcing the agent to assemble all comparisons from low-level reads. Low-level grep/read remains available for inspection, but product-level tools encode the actual job.

### Stop Conditions

There is no fixed tool-call limit. The agent should stop when one of these conditions is met:

- It proves the behavior is already covered by an existing Skill or Memory Card.
- It identifies a concrete gap, value delta, target, evidence, and non-duplicate proposal.
- It finds the right merge target and can explain the update.
- It cannot find enough evidence after reasonable search and returns `needs_human`.
- The context becomes too large, so it summarizes the trace and produces the best grounded decision available.

The trace should show why the agent stopped without exposing chain-of-thought.

The writer agent output must be JSON:

```json
{
  "action": "new_card | merge_card | mount_to_skill | skill_targeted_card | workflow_card | already_covered | ignore | needs_human",
  "function": "library | skill_targeted | workflow | merge",
  "value_claim": "What failure, gap, or improvement this proposal addresses",
  "value_delta": {
    "existing_behavior": "What the current Skill, workflow, or Memory Card already covers",
    "missing_part": "The missing trigger, instruction, boundary, acceptance criterion, or decision rule",
    "new_behavior": "What future agent behavior should improve after this Memory Card exists",
    "why_not_duplicate": "Why this is not just a paraphrase of existing instructions"
  },
  "target": {
    "type": "project_skill | workflow | memory_card | global_reference",
    "id": "optional-target-id",
    "why_this_target": "Why this is the right place"
  },
  "title": "Short imperative title",
  "kind": "constraint | procedure | preference",
  "scope": "project | global",
  "activation": "always | skill | situational",
  "target_skills": ["agent-kernel-ux-quality-pass"],
  "body": {
    "use_when": "...",
    "instructions": ["..."],
    "boundaries": ["..."]
  },
  "evidence": [
    {
      "observation_id": "obs-...",
      "quote": "..."
    }
  ],
  "merge": {
    "target_card_id": "optional",
    "reason": "optional"
  },
  "duplicate_check": {
    "nearest_existing_items": ["skill-or-card-id"],
    "difference": "What is new here compared with existing instructions"
  },
  "review_notes": ["Why this is durable and how it differs from nearby cards"]
}
```

## Stage 5: Validation Workflow

After the agent writes a draft, deterministic validators should run before the Review Inbox shows it.

Required checks:

- Evidence quotes must resolve to real observations.
- Body must not contain chatty phrases such as "the user said" or "this candidate suggests".
- Card must include trigger, action, and boundary.
- Similarity against existing cards and Skills must be below the duplicate threshold unless `action=merge_card` or the proposal explains the exact delta.
- `value_claim` must name the workflow failure, Skill gap, or future improvement.
- `value_delta` must describe existing behavior, missing part, new behavior, and why the proposal is not duplicate.
- `target_skills` must be project-level for direct mounting or fusion.
- Global Skills may be referenced but not modified by default.
- Web-cited claims must carry a source URL and retrieval date.
- Web evidence must be supporting context, not the sole basis for a project preference.

If validation fails, the system can run a repair pass or return `needs_human`. Repair should preserve the same evidence and target rather than inventing a new rationale.

## Built-In Writing Guide

Create an in-repo guide such as `docs/analysis/memory-card-writing-guide.md` or a runtime resource under `prompts/`.

The guide should distill the stable principles from notes and prior work:

- A Memory Card is future instruction, not a summary of a past conversation.
- Good cards are concise, operational, scoped, and evidence-grounded.
- Use a SKILL-like structure:
  - `Use When`
  - `Instructions`
  - `Boundaries`
  - `Evidence`
- Prefer merging over creating near-duplicates.
- Preserve human approval for durable memory changes.
- Skill-targeted Memory Cards must make the Skill better at a repeated workflow, not merely attach project trivia.
- Every card needs a value claim: what future mistake, ambiguity, or friction it prevents.
- A card that duplicates existing Skill behavior should be rejected or merged, not added as another layer.
- No Card Is A Success: if the best decision is `already_covered`, `ignore`, or `needs_human`, say that clearly instead of fabricating a card.
- Value delta must be explicit: existing behavior, missing part, new behavior, and why it is not duplicate.

This guide gives the agent a stable standard without requiring access to private notes outside the project.

## User Experience

Review Inbox should show mature artifacts, not raw extraction internals:

- Left: one mature draft card per candidate cluster.
- Right: value claim, value delta, target Skill/workflow, evidence quotes, duplicate/merge target, and project Skill fit.
- Actions:
  - Ignore
  - Approve as Memory Card
  - Merge into Existing
  - Mount Memory Card to Skill
  - Fuse Memory Card into Skill Context

For Skill-targeted Memory Cards, the review panel should show a before/after preview:

- Current Skill summary.
- Discovered gap.
- Memory Card content.
- Fused Skill context preview.
- Expected future behavior change.

The user should see a compact trace:

- "Read 3 related observations"
- "Compared 4 existing Memory Cards"
- "Checked 2 project Skills"
- "Searched web for current convention"
- "Recommended merge because overlap is high"
- "Recommended Skill-targeted Memory Card because the current Skill lacks an acceptance boundary"
- "Stopped because the behavior is already covered"

The user should not see raw chain-of-thought.

## Conversation Search Design

To support global vision without overengineering:

- Index observations with session ID, timestamp, source file, speaker, tags, and normalized text.
- Add a simple grep-like search over observations first.
- Add embedding search only if lexical search misses obvious historical patterns.
- Return short snippets, never entire long sessions by default.
- Let the writer agent ask for specific neighboring context only when needed.
- Let the agent search broadly when the value question requires it; broad search is acceptable when the trace explains why.

The first version can use:

- SQLite FTS or existing file-backed observation search.
- A small Rust search service exposed to Tauri and provider flows.
- Stable observation IDs for evidence validation.

## Implementation Sketch

Suggested modules:

- `src/extract/filter_workflow.rs`
  - cluster scoring, routing, one-off filtering.
- `src/extract/context_pack.rs`
  - gathers related observations, Memory Cards, Skills, writing guide.
- `src/extract/value_analysis.rs`
  - compares candidate clusters against existing Skills and Memory Cards, then identifies target gaps.
- `src/extract/writer_agent.rs`
  - provider-driven synthesis loop with read-only tools, web search support, and structured trace.
- `src/extract/writer_tools.rs`
  - read-only tool wrappers for observations, Memory Cards, Skills, writing guide, web, and value-oriented comparisons.
- `src/extract/card_validation.rs`
  - deterministic post-agent validation.
- `prompts/memory-card-writing-guide.md`
  - built-in writing guide used at runtime.

The existing approval and fusion paths should remain the write boundary. The writer agent proposes; workflows validate; the user approves.

## Production Runtime Slice: 2026-05-20

The first production runtime foundation is implemented in `src/synthesis_agent.rs`.

This slice ports the pi-style shape into Rust instead of embedding the TypeScript package directly. That keeps Memory Card persistence, approval, and project config ownership in the Rust core while preserving the mature runtime boundaries:

- `SynthesisReview` is the session result: candidate ID, session ID, context pack, proposal, events, and stop reason.
- `SynthesisEvent` is the reviewable event stream: observation search, Memory Card comparison, Skill search, writing guide load, duplicate check, and Skill comparison.
- Tools are read-only and typed. They can read observations, project/global Memory Cards, Skill metadata, and the built-in writing guide; they do not mutate files or project config.
- Project Skills are valid attach/fusion targets. Global/referenced Skills are used only as comparison context.
- The runtime emits `ValueDelta`, `TargetContext`, `card_function`, and compact trace entries that the existing Review Inbox can display.
- Candidate approval now consumes the runtime proposal. If the runtime finds a merge target, approval updates the existing Memory Card instead of adding clutter.
- Provider refinement receives the `synthesis_context`; deterministic fallback uses the same review when no provider is available.

The slice deliberately does not add a full provider tool-call loop yet. The next replacement point is clear: swap the deterministic tool planner for a pi-style provider loop while keeping the same `SynthesisReview` output contract and read-only tool boundary.

## Global Vision Runtime Slice: 2026-05-21

The second runtime slice improves local global vision without adding write access or an autonomous provider loop.

The context pack now separates:

- direct evidence snippets: source observations or high-confidence local matches;
- broader history snippets: same-source context and weaker related feedback from the project history;
- workflow failure insights: compact signals derived from selected observations, such as review-iterate loops, quality corrections, scope boundaries, and pipeline breaks.

This keeps the review trace explainable:

- `search_observations` means the runtime read direct evidence.
- `search_global_history` means it looked beyond the candidate's immediate evidence window.
- `summarize_workflow_failures` means it found repeated failure patterns worth considering before writing or merging a Memory Card.

The wider history layer is intentionally read-only and capped. It helps the agent answer "what existing workflow failed?" before generating a card, but it does not let weak history fabricate confidence. If neither direct evidence nor broader local context exists, the runtime returns `needs_human`.

## Testing Strategy

- Unit tests for filtering decisions:
  - one-off command rejected
  - repeated correction promoted
  - skill-improving feedback routed to `memory_card.skill_targeted`
- Value-analysis tests:
  - existing Skill already covers the advice, so proposal is ignored or merged
  - current Skill lacks a boundary, so proposal targets that Skill
  - workflow gap without a Skill owner becomes a project Memory Card
  - no-card success is accepted when the best answer is `already_covered`
- Golden tests for mature card style:
  - no chatty body
  - has `Use When / Instructions / Boundaries`
  - evidence resolves
  - includes a concrete `value_claim`
  - includes a concrete `value_delta`
- Duplicate tests:
  - near-duplicate recommends merge
  - distinct related rule recommends new card
- Tool-loop tests:
  - agent cannot write files
  - stops after it has enough evidence or proves there is no value
  - returns `needs_human` when evidence is insufficient
  - web sources are cited when web tools are used
- Counterfactual improvement tests:
  - run a historical workflow prompt with the target Skill before and after the Memory Card context
  - verify the after version avoids the original failure or follows the new boundary
  - reject proposals whose after behavior is not meaningfully different
- Product metrics:
  - duplicate suppression rate
  - merge rate versus new-card rate
  - human approval rate
  - approved-card later usefulness rate
  - Skill-targeted Memory Card counterfactual pass rate
- UI trial:
  - user can understand proposed card, evidence, and merge/skill action in under one screen.

## Open Questions

1. Should Skill fusion produce an editable preview before writing the mounted Memory Card context?
2. Should the first implementation use only lexical observation search, or include embedding search immediately?
3. Should mature Memory Cards be stored as structured JSON internally and rendered as Markdown, or should Markdown remain the canonical body?
4. How should the UI rank proposals: by confidence, target Skill importance, repeated pain, or expected future value?

## Current Recommendation

Build the first version as:

1. Filtering workflow plus deterministic validation.
2. Built-in Memory Card writing guide.
3. Gap and value analysis against existing Skills, Memory Cards, and workflows.
4. Writer agent with read-only project tools, value-oriented comparison tools, and web search when useful.
5. Before/after preview for Skill-targeted Memory Cards.
6. Human review as the only write gate.

This keeps the product understandable while giving the LLM enough freedom to find real gaps. The success metric is not how many Memory Cards are generated; it is how many approved cards measurably improve future Skill use or workflow quality.
