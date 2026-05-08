# Agent Memory Kernel

Agent Memory Kernel is a local-first Tauri Inbox workspace for turning Claude Code and Codex usage history into reviewable, high-value Memory Cards.

The product flow is:

1. Register or scan local projects.
2. Open the Tauri Inbox workspace.
3. Run an Evolve job that imports Observations, removes noise, deduplicates, scores, and writes Candidates.
4. Review Candidates, promote selected items to Drafts, then approve Drafts into Memory Cards.
5. Assign Memory Cards to Claude Code / Codex and compile artifacts such as `AGENTS.md`, `CLAUDE.md`, and skill files.

The app is intentionally not a chat-log summarizer. It filters out one-off task chatter and keeps durable agent knowledge: preferences, constraints, reusable workflows, repeated corrections, architecture decisions, and Skill supplement material.

Build output is split by activation:

- Always-on preferences and constraints compile into `AGENTS.md` and `CLAUDE.md`.
- Reusable procedures, templates, and workflows compile into Agent Skills under `.agents/skills/<name>/SKILL.md` and `.claude/skills/<name>/SKILL.md`, with full rule text in `references/`.
- Hook-oriented Memory Cards with `activation: hook` compile into Claude Code project hooks in `.claude/settings.local.json`. Use tags such as `hook:event:stop`, `hook:event:precompact`, `hook:event:session-end`, and optional `hook:matcher:<text>`.
- Generated Agent Skill folders use the open `SKILL.md` shape: frontmatter with `name` and `description`, plus only `references/`, `scripts/`, and `assets/` support folders.

## Architecture

The v1 domain model is:

```text
Observation -> Candidate -> Draft -> Memory Card -> Assignment -> Artifact
```

- `Observation` stores evidence only.
- `Candidate` stores scored, deduplicated, hideable, rejectable system suggestions.
- `Draft` stores user-promoted editable review items.
- `Memory Card` stores approved source facts.
- `Assignment` maps Memory Cards to target agents.
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
cargo run -- memory_card list --project .
cargo run -- catalog list --project .
cargo run -- build --preview --project .
cargo run -- sync --project .
cargo run -- mcp --project .
```

The `mcp` command serves a minimal stdio MCP endpoint with `list_drafts`, `approve_draft`, and `build_artifacts` tools, so Claude Code or other MCP clients can review and advance the local Draft Inbox without shelling out ad hoc commands.

The old native app and legacy web UI have been removed. New visual work belongs in `app/` and `src-tauri/`.

## Installation

Agent Memory Kernel is currently pre-1.0. The recommended ways to try it are:

- Build the CLI locally with `cargo build --release`.
- Run the Bun wrapper in development with `bun run start -- <args>`.
- Install a release artifact from GitHub Releases once tagged builds are published.

Windows desktop releases are built from the Tauri workspace. A local Windows build can be produced with:

```bash
bun run tauri:build
```

The generated installer artifacts are written under:

```text
src-tauri/target/release/bundle/
```

Release builds also package the CLI binary for Windows, Linux, and macOS.

## Privacy and Local Data

Agent Memory Kernel is local-first, but it can inspect local agent history and project configuration when you ask it to import or evolve observations. Generated Drafts and Memory Cards are review-first: they are not meant to be silently enabled without user approval.

Do not commit runtime data from `.agent-kernel/`, generated agent skill folders, local provider configuration, build outputs, or private conversation logs. The repository `.gitignore` excludes these local artifacts for public development.

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
