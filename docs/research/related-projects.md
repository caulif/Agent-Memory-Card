# Related Projects And Borrowed Ideas

Date: 2026-05-01

Agent Memory Kernel should avoid rebuilding mature pieces from nearby ecosystems. The v0.1 implementation borrows these ideas:

## Agent Skills Standard

- Source: https://agentskills.io/
- Borrowed idea: a Skill is a directory with `SKILL.md` and optional supporting files.
- Product impact: Mirror mode copies the entire directory structure, not only `SKILL.md`.

## Claude Code Skills

- Source: https://code.claude.com/docs/en/skills
- Borrowed idea: progressive disclosure and on-demand skills keep always-loaded context small.
- Product impact: Agent Memory Kernel treats full Skills as attachable capabilities, while rule files stay short build artifacts.
- 2026-05-01 update: Claude Code documents personal/project/plugin skill locations, live skill change detection, nested `.claude/skills/` discovery, and optional supporting files (`references/`, `examples/`, `scripts/`) loaded only when needed. This reinforces Mirror mode and argues for keeping Memory Cards small while preserving full Skill folders.
- 2026-05-01 update: Claude Code local transcript JSONL files are treated as Observation inputs rather than direct Memory Card outputs. This preserves the review boundary before any learned preference is compiled back into `CLAUDE.md` or Skills.

## OpenAI Codex Skills And AGENTS.md

- Source: https://developers.openai.com/codex/skills
- Source: https://developers.openai.com/codex/guides/agents-md
- Borrowed idea: project instructions and skills are separate surfaces.
- Product impact: Codex export writes `AGENTS.md` and mirrors Skills into `.agents/skills`.
- 2026-05-01 update: Codex skills use progressive disclosure with an initial skill list capped around 2% of the context window or 8,000 characters when unknown. Codex also treats plugins as the distribution unit for reusable skills. This directly supports Agent Memory Kernel's separation between tiny compiled instructions, referenced Skills, and installable Catalog packages.
- 2026-05-01 update: Codex `AGENTS.md` discovery is layered: global guidance, then project files from root to current directory, with closer files overriding earlier guidance. Codex stops once the combined size reaches `project_doc_max_bytes` (32 KiB default), which supports adding future budget checks to Agent Memory Kernel build previews.
- 2026-05-01 update: local Codex session JSONL files are a useful Observation source for Memory Card evolution. Agent Memory Kernel now treats Codex conversations as raw observations before any Memory Card synthesis.

## Letta Code / MemFS

- Source: https://docs.letta.com/letta-code/memory/
- Source: https://www.letta.com/blog/context-repositories
- Borrowed idea: memory as a git-backed filesystem of markdown files, curated over time by reflection subagents.
- Product impact: Agent Memory Kernel should keep source-of-truth memory files reviewable, versioned, and editable as plain text. Letta's compaction-triggered reflection suggests a future "post-compaction Draft Inbox" trigger.
- 2026-05-01 update: Letta's context repository framing reinforces Agent Memory Kernel's choice to keep Observation, Draft, and Memory Card state as local reviewable files instead of hiding learned preferences inside a runtime service.

## Mem0

- Source: https://docs.mem0.ai/platform/overview
- Source: https://docs.mem0.ai/platform/cli
- Borrowed idea: user, agent, and session memory separation; production memory layers provide graph/rerank infrastructure when an app truly needs retrieval.
- Product impact: Agent Memory Kernel should remain the lightweight file-governance layer and integrate with heavy memory systems later through import/export rather than reimplementing vector/graph memory in the MVP.
- 2026-05-01 update: Mem0's CLI memory import flow validates a terminal-first ingestion path, while Agent Memory Kernel keeps the output as reviewable Draft Memory Cards rather than immediate runtime memory.

## OpenCode And Goose Skills

- Source: https://opencode.ai/docs/skills/
- Source: https://goose-docs.ai/docs/guides/context-engineering/using-skills
- Borrowed idea: more coding agents are converging on `SKILL.md` directories plus supporting files, which makes portable Memory Card and Skill management more valuable than yet another proprietary prompt file.
- Product impact: OpenCode and Goose stay outside the MVP target set, but their native Skill support argues for an adapter registry later: one Memory Card source, multiple Agent-native renderers.

## 2026 Skill Optimization Research

- Source: https://arxiv.org/abs/2602.12430
- Source: https://arxiv.org/abs/2603.29919
- Source: https://arxiv.org/abs/2604.04853
- Borrowed idea: current research is moving toward portable skills, token-efficient skill reduction, and ground-truth-preserving memory systems.
- Product impact: Agent Memory Kernel should treat local transcript Observations as ground truth, synthesize only reviewable Drafts, and add future Rule CI before aggressive Memory Card compression.

## Cursor Rules

- Source: https://docs.cursor.com/en/context
- Borrowed idea: `.cursor/rules/*.mdc` supports metadata-driven activation (`alwaysApply`, `globs`, and agent-requested rules).
- Product impact: future Cursor exporter should emit scoped `.mdc` files instead of only a generic markdown artifact, especially for directory-specific Memory Cards.

## Cline Memory Bank And .clinerules

- Source: https://docs.cline.bot/customization/memory-bank
- Source: https://cline.bot/blog/clinerules-version-controlled-shareable-and-ai-editable-instructions
- Borrowed idea: Cline's Memory Bank uses structured markdown files for project continuity, while `.clinerules` treats instructions as version-controlled code.
- Product impact: Agent Memory Kernel's Draft Inbox and build-artifact model can support `.clinerules/` and memory-bank exports later, with user review before mutable memories become always-loaded instructions.
- 2026-05-01 update: Cline-style rules fit best as a rules directory where Agent Memory Kernel owns one generated file (`.clinerules/agent-kernel.md`) instead of overwriting the user's whole rule surface. This mirrors Cursor's generated `.mdc` artifact and keeps manual Cline rules available beside compiled Memory Cards.

## Roo Code And Windsurf Rules

- Source: https://docs.roocode.com/features/custom-instructions
- Source: https://docs.windsurf.com/windsurf/cascade/memories
- Borrowed idea: adjacent agent tools also split persistent guidance into rules, memories, and scoped project files.
- Product impact: the exporter layer should remain table-driven. Adding Roo/Windsurf later should mostly mean declaring target paths and renderers, not changing Memory Card storage.

## Continue Rules

- Source: https://docs.continue.dev/customize/rules
- Borrowed idea: Continue rules are portable Markdown instructions that can be scoped and shared with a team.
- Product impact: future exporters should support one Memory Card source compiled into tool-native rule files while keeping Agent Memory Kernel's source-of-truth YAML/MD declarative.

## Agent Skills Spec And Security Research

- Source: https://agentskills.io/specification
- Source: https://arxiv.org/abs/2602.12430
- Source: https://arxiv.org/abs/2604.06550
- Borrowed idea: Skill packages need both structural portability and lifecycle governance. Recent research highlights vulnerability rates and the difficulty of scanning natural-language instructions.
- Product impact: Catalog packages now carry provenance (`version`, `source_url`, `tags`). Next useful step is a local `catalog validate` trust gate for duplicate IDs, missing provenance, and suspicious package metadata before installation.

## GitHub Copilot Coding Agent Skills

- Source: https://docs.github.com/en/copilot/how-tos/use-copilot-agents/coding-agent/create-skills
- Borrowed idea: GitHub also uses `SKILL.md`-based skill directories.
- Product impact: scanning now includes project `.github/skills` and home `.copilot/skills` candidates.

## Superpowers

- Source: https://github.com/obra/superpowers
- Borrowed idea: skills are practical workflow bundles and can be shared through a linked local directory.
- Product impact: the local install is indexed as referenced skills and mirrored without modifying the source.

## Skilldex-Style Validation

- Related search term: "Skilldex AI skills manager SKILL.md"
- Borrowed idea: skill libraries benefit from format checks and dispatch-quality warnings.
- Product impact: Scan records warnings for missing `name`, missing `description`, empty names, and very short descriptions.

## Dotfiles And Build Artifact Tools

- Reference patterns: chezmoi, GNU Stow, Vite/Webpack build outputs.
- Borrowed idea: user-owned source config should produce generated target files.
- Product impact: `.agent-kernel/project.yml` is declarative source; `AGENTS.md`, `CLAUDE.md`, and mirrored Skill folders are build artifacts.
- 2026-05-01 update: build-artifact workflows need drift detection before automatic overwrite. Agent Memory Kernel now records generated artifact hashes in `project.lock.yml` and surfaces manual edits as artifact drift, which sets up a future Reverse Parse flow that can ingest manual changes into Draft Inbox instead of silently discarding them.
- 2026-05-01 update: the first Reverse Parse implementation intentionally borrows from build-tool source-map thinking rather than RAG. Agent Memory Kernel re-renders the expected artifact, compares it with the edited artifact, and imports added lines as reviewable Drafts tied to the owning Agent target.
