# High Quality Extraction Classification And Hardness Design

## Purpose

This document extends the existing high-quality extraction design with a sharper definition of what should become a reviewable Agent-Kernel record.

The previous extraction funnel already establishes the product direction:

```text
Observation -> EvidenceChunk -> Noise Gate -> Signal Gate -> Value Score -> Candidate/Draft
```

This increment adds a second judgment layer:

```text
Signal -> Knowledge Classification -> Hardness/Fragility -> Artifact Target -> Tags
```

The goal is not to extract more. The goal is to extract fewer records with better downstream shape: short always-on rules for AGENTS.md/CLAUDE.md, task-triggered workflows for Skills, and review-only notes for risky or ambiguous knowledge.

## Product Principle

Agent-Kernel serves one advanced local developer and their coding agents. A high-quality Candidate is useful only if it prevents a future agent mistake, shortens a future workflow, or preserves a project decision that would otherwise be lost.

Good extraction should answer five questions before creating a Candidate:

1. Would a future Codex or Claude Code instance likely act worse without this knowledge?
2. Is this grounded in local evidence: user correction, repeated trace, project artifact, accepted AI synthesis, or a completed fix?
3. Is it actionable enough for an agent to follow?
4. Is it reusable beyond the current one-off task?
5. What is the correct delivery form: always-on rule, path-scoped rule, workflow Skill, Skill supplement, memory note, or reject?

When the engine is uncertain, it should stay silent or mark the item as borderline for review/debug output. Daily governance depends on a small Inbox.

## External Model Alignment

The design borrows from current agent instruction systems:

- Codex `AGENTS.md` is best for concise, project-scoped instructions that should be loaded into the agent context by default.
- Claude Code `CLAUDE.md` memory works best when rules are specific, short, and easy to follow.
- Claude/Agent Skills are better for on-demand workflows, scripts, references, and long procedural knowledge that should not always consume context.
- Cursor/Windsurf-style rules distinguish always-on rules, path-scoped rules, agent-requested rules, manual rules, workflows, skills, and memories.

Agent-Kernel should use these distinctions during extraction rather than treating every useful sentence as the same kind of Skilllet.

## Knowledge Classification

Add a deterministic classification layer that describes the shape of the extracted knowledge.

### Signal

`signal` describes why the text looks valuable:

- `preference`: stable default, tool choice, library choice, style, or process preference.
- `constraint`: prohibited behavior, risky action, or "do not" rule.
- `procedure`: reusable multi-step workflow or checklist.
- `correction`: user correction intended to prevent repeated failure.
- `decision`: product, architecture, storage, protocol, or scope decision.
- `gotcha`: recurring local trap, platform quirk, or surprising failure mode.
- `template`: reusable prompt, handoff, review checklist, or instruction pattern.
- `validation`: required test, build, preview, audit, or verification command.
- `supplement`: content that should enrich an existing Skill rather than become a standalone always-on rule.
- `ai_project_improvement`: accepted AI-origin improvement that materially improves this project.

Existing storage kinds can remain `preference`, `constraint`, and `procedure` for compatibility. The richer signal should live in extraction metadata and tags.

### Artifact Kind

`artifact_kind` describes where the knowledge belongs after review:

- `always_on_rule`: short, durable, broad rule suitable for AGENTS.md or CLAUDE.md.
- `path_rule`: rule triggered by paths, technologies, or file globs.
- `workflow_skill`: multi-step workflow that should become or update a Skill directory.
- `skill_supplement`: reference, checklist, template, or script material for an existing Skill.
- `memory_note`: useful but low-pressure remembered context.
- `review_only`: high-risk or ambiguous content that should stay in Inbox until edited.
- `reject`: not durable agent knowledge.

The first implementation should only classify and tag. It does not need to auto-create Skills or path-scoped artifacts.

### Activation

`activation` describes when an agent should see the knowledge:

- `always_on`: every relevant project session.
- `path_glob`: only when working under matching paths or file types.
- `model_decision`: the agent may load it when the task matches the description.
- `manual`: the user or review flow must opt in.
- `skill`: available through a Skill trigger.

This is important because high-quality long workflows are harmful if injected into every prompt.

### Control Strength

`control` describes how prescriptive the rule is:

- `principle`: guidance with judgment.
- `default`: preferred behavior unless there is a reason to deviate.
- `checklist`: ordered or unordered steps that should be verified.
- `exact_sequence`: brittle workflow where order matters.
- `prohibition`: action that should not happen without explicit approval.

Control strength influences both scoring and target assignment.

## Hardness And Fragility

`hardness` is not task size. It is how costly or likely agent failure is if the instruction is vague.

### Low

Low-hardness knowledge is easy to follow and cheap to correct.

Examples:

- Use Bun for JavaScript scripts.
- Use Axios for frontend HTTP requests.
- Prefer Vitest for frontend unit tests.

Expected artifact: `always_on_rule` or `path_rule`.

Quality gate:

- Must name a concrete tool, library, style, or default.
- Should be one sentence after normalization.
- Does not need a long evidence bundle unless it conflicts with existing rules.

### Medium

Medium-hardness knowledge needs workflow awareness but allows judgment.

Examples:

- For UI polish, let Claude Code propose component/interaction changes, then let Codex integrate and test.
- For extraction changes, add quality fixtures before changing scoring.

Expected artifact: `always_on_rule` if short, otherwise `workflow_skill` or `skill_supplement`.

Quality gate:

- Must include trigger context.
- Must include at least one action and one expected outcome.
- Should be reviewable as a checklist or compact procedure.

### High

High-hardness knowledge prevents common serious mistakes or project drift.

Examples:

- Do not overwrite AGENTS.md directly; import drift as Draft and require review.
- Preview and audit compiled artifacts before writing.
- For Rust extraction changes, run fixture tests, cargo test, and clippy before claiming success.

Expected artifact: `always_on_rule`, `review_only`, or validation metadata.

Quality gate:

- Must preserve the forbidden action or required verification.
- Should include the safer replacement workflow.
- Should be tagged with `hardness:high` and `control:prohibition` or `control:checklist`.

### Critical

Critical knowledge involves secrets, destructive operations, publishing, migrations, credentials, policy bypass, or irreversible file changes.

Expected artifact: `review_only` by default. It may become a strong constraint only after human editing.

Quality gate:

- Never auto-approve.
- Never silently compile from AI-origin text.
- Must keep provenance and risk markers visible in the Inbox.

## Quality Scoring Changes

The current deterministic scorer should move from a single broad score to a score with interpretable dimensions:

```text
score =
  future_agent_value
  + grounding
  + actionability
  + trigger_clarity
  + specificity
  + recurrence_or_correction
  + validation_value
  + fragility_value
  + novelty
  - one_off_penalty
  - unresolved_penalty
  - generic_advice_penalty
  - transcript_noise_penalty
  - prompt_leak_penalty
  - duplicate_penalty
  - sensitive_content_penalty
```

Key behavior:

- A low-hardness preference can pass with strong specificity and clear trigger.
- A medium-hardness workflow needs trigger clarity and actionability.
- A high-hardness constraint needs safer alternative or validation value.
- Generic advice should fail even if it contains words like "quality", "tests", or "documentation".
- AI-origin content needs acceptance or reinforcement markers before it can become a Candidate.

## Tag Taxonomy

Tags should become structured and predictable. A Candidate may have several tags, but each tag should come from a known axis.

### Shape Tags

- `shape:preference`
- `shape:constraint`
- `shape:procedure`
- `shape:correction`
- `shape:decision`
- `shape:gotcha`
- `shape:template`
- `shape:validation`
- `shape:supplement`

### Domain Tags

- `domain:rust`
- `domain:tauri`
- `domain:react`
- `domain:typescript`
- `domain:frontend`
- `domain:backend`
- `domain:testing`
- `domain:build`
- `domain:git`
- `domain:security`
- `domain:extraction`
- `domain:governance`
- `domain:ui`
- `domain:agent-behavior`

### Target Tags

- `target:codex`
- `target:claude-code`
- `target:agents-md`
- `target:claude-md`
- `target:skill-dir`

### Activation Tags

- `activation:always-on`
- `activation:path-scoped`
- `activation:model-decision`
- `activation:manual`
- `activation:skill`

### Hardness Tags

- `hardness:low`
- `hardness:medium`
- `hardness:high`
- `hardness:critical`

### Evidence Tags

- `evidence:user-correction`
- `evidence:repeated-trace`
- `evidence:accepted-ai`
- `evidence:artifact`
- `evidence:completed-fix`
- `evidence:review`

Structured tags can coexist with older free-form tags during migration. New extraction should prefer structured tags.

## Candidate Examples

### Always-On Rule

Evidence:

```text
以后这个项目的前端 HTTP 请求统一用 Axios，不要再写裸 fetch。
```

Classification:

- `signal`: `preference`
- `artifact_kind`: `always_on_rule`
- `activation`: `always_on`
- `hardness`: `low`
- `control`: `default`
- tags: `shape:preference`, `domain:frontend`, `target:agents-md`, `hardness:low`

### High-Risk Governance Constraint

Evidence:

```text
不要直接覆盖 AGENTS.md；发现漂移时先导入成 Draft，让我确认后再同步。
```

Classification:

- `signal`: `constraint`
- `artifact_kind`: `always_on_rule`
- `activation`: `always_on`
- `hardness`: `high`
- `control`: `prohibition`
- tags: `shape:constraint`, `domain:governance`, `target:agents-md`, `hardness:high`, `evidence:user-correction`

### Workflow Skill Or Skill Supplement

Evidence:

```text
处理大型 UI 重构时，可以先让 Claude Code 输出组件边界、状态流和可交互按钮清单，再由 Codex 做集成测试和 Rust 后端检查，这个提示能显著减少返工。
```

Classification:

- `signal`: `template`
- `artifact_kind`: `workflow_skill`
- `activation`: `skill`
- `hardness`: `medium`
- `control`: `checklist`
- tags: `shape:template`, `shape:procedure`, `domain:ui`, `domain:agent-behavior`, `target:skill-dir`, `hardness:medium`

### Accepted AI Project Improvement

Evidence:

```text
基于用户确认，项目应该把 AI 生成的高质量项目改善也送入 Candidate/Draft，而不是直接写 Skilllet；这能保留审阅边界。
```

Classification:

- `signal`: `ai_project_improvement`
- `artifact_kind`: `review_only` at extraction time. A reviewer may edit it into an `always_on_rule`.
- `activation`: `manual` until approved
- `hardness`: `high`
- `control`: `prohibition`
- tags: `shape:constraint`, `domain:governance`, `evidence:accepted-ai`, `hardness:high`

## Noise Rules To Strengthen

Reject or strongly penalize:

- Generic best-practice advice without local trigger or evidence.
- One-off task requests, even if they contain quality words.
- Unaccepted AI suggestions.
- Long lists where no single reusable rule is clear.
- Shell output and compiler errors unless paired with a user correction or completed fix.
- Internal agent narration such as "I will inspect files" without durable instruction.
- Raw system/developer prompts unless imported as explicit artifact evidence.
- Secrets, tokens, credentials, and long absolute path dumps.

## Metadata Additions

Extend `ExtractionMetadata` with optional fields:

```rust
pub classification: Option<KnowledgeClassification>,
pub tags: Vec<String>,
```

Where `KnowledgeClassification` includes:

```rust
pub signal: String,
pub artifact_kind: String,
pub activation: String,
pub hardness: String,
pub control: String,
pub rationale: String,
```

The first implementation can store classification in metadata and continue writing existing top-level `kind`, `scope`, `tags`, `reason`, and `matched_template` fields for compatibility.

## UI Impact

The Review Inbox should show classification as compact chips:

- signal
- hardness
- artifact kind
- evidence origin

The editor should explain:

- why this is valuable
- why it is not noise
- where it should compile after approval
- what risk/hardness level it carries

The UI does not need new pages for this phase.

## Acceptance Criteria

- Quality fixtures cover classification, hardness, tags, and artifact target.
- The extractor distinguishes always-on rules from workflow Skill material.
- High-hardness constraints preserve the forbidden action and safer replacement.
- Generic AI advice and one-off requests remain rejected.
- Accepted AI-origin improvements can become reviewable Candidates but never direct Skilllets.
- New structured tags are generated deterministically for new Candidates.
- Existing tests and build commands remain green.

## Non-Goals

- Do not add a mandatory vector database.
- Do not auto-create Skill directories in this phase.
- Do not add MCP/A2A integration in this phase.
- Do not make LLM extraction mandatory.
- Do not broaden the product beyond one local advanced developer.

## References

- Codex AGENTS.md guide: https://developers.openai.com/codex/guides/agents-md
- Claude Skills overview: https://claude.com/docs/skills/overview
- Agent Skills specification: https://agentskills.io/specification
- Agent Skills best practices: https://agentskills.io/skill-creation/best-practices
- Claude Code memory: https://code.claude.com/docs/en/memory
- Windsurf memories and rules: https://docs.windsurf.com/windsurf/cascade/memories
