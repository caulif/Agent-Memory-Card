# Agent-Kernel

Agent-Kernel v0.1 is a Rust-built, project-centered workspace for managing Agent rules and Skills.

This prototype implements the first loop:

1. Import existing rule files and Skills.
2. Show a Project-centered Canvas UI.
3. Mirror referenced Skills into target Agent skill directories.
4. Preview or build generated Agent artifacts.

## Commands

```bash
cargo run -- import --scan-home --project .
cargo run -- ui --project .
cargo run -- mirror --skill superpowers:brainstorming --agent codex --project .
cargo run -- build --preview --project .
cargo run -- build --project .
cargo run -- status --project .
```

The npm wrapper works locally after the Rust binary has been built:

```bash
node npm/agent-kernel.js scan --scan-home --project .
```

## Generated State

- `.agent-kernel/project.yml` is the declarative project config.
- `.agent-kernel/skill-index.yml` is the imported Skill index.
- `.agent-kernel/project.lock.yml` records generated mirror and artifact hashes.
- `AGENTS.md`, `CLAUDE.md`, and mirrored skill folders are build artifacts.
