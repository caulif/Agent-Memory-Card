# Agent-Kernel

Agent-Kernel v0.1 is a Rust-built, project-centered workspace for managing Agent rules and Skills.

This prototype implements the first loop:

1. Import existing rule files and Skills.
2. Show a Project-centered Canvas UI.
3. Mirror referenced Skills into target Agent skill directories.
4. Preview or build generated Agent artifacts.

v0.2 adds the trust loop:

1. Mirror metadata records the source hash at sync time.
2. `status` distinguishes `synced`, `missing`, `source updated`, `target drifted`, and combined drift.
3. `sync` reconciles declared mirrors and generated artifacts.
4. The Canvas API exposes status and sync endpoints for UI controls.

## Commands

```bash
cargo run -- import --scan-home --project .
cargo run -- ui --project .
cargo run -- mirror --skill superpowers:brainstorming --agent codex --project .
cargo run -- build --preview --project .
cargo run -- build --project .
cargo run -- status --project .
cargo run -- sync --project .
```

The npm wrapper works locally after the Rust binary has been built:

```bash
node npm/agent-kernel.js scan --scan-home --project .
```

## v0.1 Workflow

```bash
# 1. Import existing local rules and Skills.
cargo run -- import --scan-home --project .

# 2. Open the Project-centered Canvas.
cargo run -- ui --project .

# 3. Mirror a referenced Skill to an Agent target.
cargo run -- mirror --skill superpowers:brainstorming --agent codex --project .

# 4. Preview generated artifacts.
cargo run -- build --preview --project .

# 5. Write generated artifacts and mirrored skill directories.
cargo run -- build --project .

# 6. Check mirror health.
cargo run -- status --project .

# 7. Re-sync declared mirrors and generated artifacts.
cargo run -- sync --project .
```

The Canvas UI exposes:

- `/api/state` for project config, imported rules, and skill index
- `/api/mirror` for declaring a Skill mirror
- `/api/build/preview` for build previews
- `/api/sync` for syncing declared mirrors and generated artifacts
- `/api/status` for synced / missing / drifted mirror state

## Generated State

- `.agent-kernel/project.yml` is the declarative project config.
- `.agent-kernel/skill-index.yml` is the imported Skill index.
- `.agent-kernel/project.lock.yml` records generated mirror and artifact hashes.
- `AGENTS.md`, `CLAUDE.md`, and mirrored skill folders are build artifacts.
