# Agent-Kernel

Agent-Kernel is a local-first Tauri Inbox workspace for turning Claude Code and Codex usage history into reviewable, high-value Skilllets.

The product flow is:

1. Register or scan local projects.
2. Open the Tauri Inbox workspace.
3. Run an Evolve job that imports Observations, removes noise, deduplicates, scores, and writes Candidates.
4. Review Candidates, promote selected items to Drafts, then approve Drafts into Skilllets.
5. Assign Skilllets to Claude Code / Codex and compile artifacts such as `AGENTS.md`, `CLAUDE.md`, and skill files.

The app is intentionally not a chat-log summarizer. It filters out one-off task chatter and keeps durable agent knowledge: preferences, constraints, reusable workflows, repeated corrections, architecture decisions, and Skill supplement material.

Build output is split by activation:

- Always-on preferences and constraints compile into `AGENTS.md` and `CLAUDE.md`.
- Reusable procedures, templates, and workflows compile into Agent Skills under `.agents/skills/<name>/SKILL.md` and `.claude/skills/<name>/SKILL.md`, with full rule text in `references/`.
- Hook-oriented Skilllets with `activation: hook` compile into Claude Code project hooks in `.claude/settings.local.json`. Use tags such as `hook:event:stop`, `hook:event:precompact`, `hook:event:session-end`, and optional `hook:matcher:<text>`.
- Generated Agent Skill folders use the open `SKILL.md` shape: frontmatter with `name` and `description`, plus only `references/`, `scripts/`, and `assets/` support folders.

## Architecture

The v1 domain model is:

```text
Observation -> Candidate -> Draft -> Skilllet -> Assignment -> Artifact
```

- `Observation` stores evidence only.
- `Candidate` stores scored, deduplicated, hideable, rejectable system suggestions.
- `Draft` stores user-promoted editable review items.
- `Skilllet` stores approved source facts.
- `Assignment` maps Skilllets to target agents.
- `Artifact` is generated output for Claude Code, Codex, and compatible adapters.

Candidate storage is file-native YAML under:

```text
.agent-kernel/candidates/project/*.yml
```

The Tauri command layer is kept thin. Business use cases live in application services, and frontend pages load page-level read models instead of full project snapshots. `get_project_snapshot` remains only as a legacy/debug fallback.

## Development

Use Bun for the frontend:

```bash
bun install
bun run --cwd app test
bun run --cwd app build
```

Run the Tauri workspace during development:

```bash
bun run app:dev
```

If the Vite frontend is opened directly in a normal browser, it enters preview mode with static demo data. Real scanning, mutation, evolution, and artifact sync require the Tauri runtime.

## CLI

The core CLI remains available for automation and scripting:

```bash
cargo run -- project scan --root . --max-depth 4
cargo run -- import --project .
cargo run -- observe evolve --project . --target codex --target claude-code
cargo run -- draft list --project .
cargo run -- skilllet list --project .
cargo run -- catalog list --project .
cargo run -- build --preview --project .
cargo run -- sync --project .
cargo run -- mcp --project .
```

The `mcp` command serves a minimal stdio MCP endpoint with `list_drafts`, `approve_draft`, and `build_artifacts` tools, so Claude Code or other MCP clients can review and advance the local Draft Inbox without shelling out ad hoc commands.

The old native app and legacy web UI have been removed. New visual work belongs in `app/` and `src-tauri/`.

## Verification

Recommended local verification:

```bash
cargo test --quiet
cargo clippy --quiet -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --quiet
cargo clippy --manifest-path src-tauri/Cargo.toml --quiet -- -D warnings
bun run --cwd app test
bun run --cwd app build
bun test bun/agent-kernel-lib.test.js
```
