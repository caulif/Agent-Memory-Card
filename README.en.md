# Agent Memory Kernel

[简体中文](README.md) | [English](README.en.md)

Agent Memory Kernel is a local-first Memory Card workspace for heavy Claude Code and Codex users. It turns useful knowledge from real conversations, project rules, and Skills into reviewable Memory Cards, then syncs approved knowledge back into each agent's native memory surfaces.

It is not a chat-log summarizer. It is a review-first memory compiler for agent workflows.

## Who It Is For

- You use Claude Code or Codex heavily on real projects.
- Your `AGENTS.md`, `CLAUDE.md`, or Skills are becoming long, duplicated, or inconsistent.
- You often tell the agent "do this next time", but those lessons are hard to preserve.
- You want memory to stay local, inspectable, editable, and versionable.
- You want short always-on rules and larger on-demand Skills to be managed separately.

## Core Workflow

```text
Observation -> Candidate -> Draft -> Memory Card -> Assignment -> Artifact
```

- `Observation`: evidence imported from local conversations, rules, or project files.
- `Candidate`: a scored and deduplicated memory suggestion.
- `Draft`: an editable item in the human review inbox.
- `Memory Card`: an approved long-term memory source.
- `Assignment`: a mapping from Memory Cards to target agents.
- `Artifact`: generated native files for Claude Code, Codex, and compatible adapters.

Approved knowledge can compile into:

- `AGENTS.md` and `CLAUDE.md` for always-on preferences and constraints.
- `.agents/skills/<name>/SKILL.md` and `.claude/skills/<name>/SKILL.md` for reusable procedures, templates, and workflows.
- Claude Code project hooks for hook-oriented Memory Cards.

## Features

The main product surface is the Tauri desktop workspace:

- Project list for scanning, registering, and opening local Claude Code / Codex projects.
- Review Dashboard for Drafts, Memory Cards, artifact drift, and sync status.
- Draft Inbox for reviewing extracted candidates with confidence, rationale, classification, and target agent metadata.
- Memory Library for approved long-term agent knowledge.
- Assignment Matrix for deciding which Memory Cards apply to Codex, Claude Code, or both.
- Catalog for reusable Memory Card packages.
- Observation Evolve for turning local Claude Code / Codex history into reviewable candidates.
- Build / Sync for generating `AGENTS.md`, `CLAUDE.md`, and Skill folders.

The CLI is currently a source-development and internal verification surface. It is not published as a public release artifact yet.

## Install And Run

Agent Memory Kernel is currently pre-1.0. Windows users should prefer the desktop installer from GitHub Releases; developers can run it from source.

Requirements:

- Rust 1.85+
- Bun 1.3+
- Claude Code CLI, optional but recommended because the default provider is Claude Code

Install dependencies:

```bash
bun install
bun install --cwd app
bun run --cwd app build
```

Run the desktop workspace:

```bash
bun run app:dev
```

After launch, open a project from the project list. The desktop workspace lets you review Drafts, approve Memory Cards, assign them to agents, evolve local history, and run Build / Sync from the UI.

Check the CLI:

```bash
cargo run -- --version
```

Build the Windows desktop bundle:

```bash
bun run tauri:build
```

## Quick Start

Use the desktop UI for the full workflow:

1. Start the workspace.
2. Scan or add a local project.
3. Open the project dashboard.
4. Run Observation Evolve to collect reviewable candidates from local agent history.
5. Review Draft Inbox items and approve useful ones as Memory Cards.
6. Adjust agent assignments.
7. Preview and sync generated artifacts.

```bash
bun run app:dev
```

Default artifact targets:

- `codex` -> `AGENTS.md` and `.agents/skills`
- `claude-code` -> `CLAUDE.md` and `.claude/skills`

## Developer CLI

```bash
cargo run -- import --scan-home --project .
cargo run -- project add --path .
```

The default provider is `claude-cli`. For deterministic local diagnostics, pass `--provider local` explicitly:

```bash
cargo run -- extract --text "Use Axios for frontend HTTP requests. Do not use raw fetch." --target codex --provider local --dry-run --project .
```

Useful automation commands:

```bash
cargo run -- observe evolve --project . --target codex --target claude-code --dry-run
cargo run -- build --preview --project .
cargo run -- sync --project .
cargo run -- mcp --project .
```

The `mcp` command serves a small stdio MCP endpoint with tools such as `list_drafts`, `approve_draft`, and `build_artifacts`.

## Privacy Model

Agent Memory Kernel is local-first. It reads local projects and local agent history only when you ask it to import, scan, or evolve observations.

The trust model is intentionally conservative:

- Automation finds candidates.
- Humans approve long-term memory.
- Memory Cards remain readable and editable.
- Generated agent files are build artifacts that can be inspected and regenerated.

## Development

```bash
cargo fmt --check
cargo clippy --quiet -- -D warnings
bun run --cwd app build
```

For Tauri backend changes:

```bash
cargo clippy --manifest-path src-tauri/Cargo.toml --quiet -- -D warnings
cargo build --manifest-path src-tauri/Cargo.toml --release
```

## Contributing

Issues, documentation improvements, bug fixes, adapters, and real-world workflow reports are welcome. For pull requests, please:

- Explain the user-visible problem being solved.
- Keep the scope focused.
- Add verification notes for behavior changes. Test files stay in local development environments and are not uploaded to the public repository.
- Include screenshots or recordings for UI changes.
- Describe privacy and failure-mode implications for provider, file-writing, or artifact-sync changes.

See [CONTRIBUTING.md](CONTRIBUTING.md) for more details.

## License

MIT

