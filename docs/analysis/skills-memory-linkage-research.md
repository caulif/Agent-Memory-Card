# Skills And Memory Linkage Research

Task: Manage project Skills as first-class assets and connect high-value Memory Cards to Skills as scoped supplemental guidance.

Mode: GITHUB_FULL
Repository: caulif/Agent-Memory-Card

## User Intent

The user wants Agent Memory Kernel to manage Skills, not only Memory Cards. A useful Memory Card should be attachable to a Skill so the Skill gains durable project-specific context without hand-editing `SKILL.md`.

## External References

- Agent Skills specification: Skills are file-system packages with `SKILL.md`, frontmatter, and optional resources.
- Anthropic Agent Skills: Skills are demand-loaded capability packages that let agents perform real workflows.
- `kepano/obsidian-skills`: Obsidian-specific skills package with Markdown, Bases, Canvas, CLI, and extraction skills.
- MemSkill and Memento-Skills: Research and implementation patterns for memory-enhanced agent skills.
- SkillSmith: Treats agent skills as compiled, scoped interfaces instead of unstructured prompts.

## Obsidian Design Inputs

From `C:\obsidian`:

- `AGENTS.md` already treats `.github/obsidian-skills/` as a repository skill contract.
- `知识库编译索引.md` frames knowledge work as `ingest`, `query`, `lint`, and `compile pass`.
- `Context Engineering.md` defines the product goal: decide what the agent should know now, in what order, how much, and when to forget.
- `Agentic Coding Workflow.md` closes the loop with "沉淀到规范、模板、hooks 或 skills".

These imply a product model where Memory Cards are small durable context units and Skills are executable context packages. Linkage should be explicit, previewable, and reversible.

## Current Code Baseline

- `src/scanner.rs` scans `SKILL.md` files into `.agent-kernel/skill-index.yml`.
- `src/config.rs` already stores `skills.mirrors` and `skills.supplements`.
- `config::add_skill_supplement` links a Memory Card to a Skill.
- `src/build.rs` writes `AGENT_KERNEL_MEMORY_CARDS.md` into mirrored Skill folders.
- CLI has `memory-card attach-skill`.
- Tauri already exposes `attach_memory_card_to_skill`, but no product-facing Skill library view exists.

## Gap

The feature exists as hidden infrastructure but lacks a clear read model, UI, recommendation surface, and safe preview loop. Users cannot see which Skills exist, which Memory Cards supplement them, or which Memory Cards are good candidates for Skill linkage.
