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

v0.3 adds the Skilllet foundation:

1. Owned Skilllets live under `.agent-kernel/skilllets`.
2. `skilllet add` writes a Skilllet and updates declarative `project.yml`.
3. Build artifacts compile targeted Skilllets into the selected Agent instructions.
4. The Canvas state API exposes owned Skilllets for UI display.

v0.4 starts the Draft Inbox foundation:

1. Local drafts live under `.agent-kernel/drafts`.
2. `draft add` creates a reviewable candidate without enabling it.
3. `draft approve` converts a draft into an owned Skilllet and updates `project.yml`.
4. The Canvas state API exposes drafts for UI display.

v0.5 makes Draft Inbox actionable in the UI:

1. Draft cards expose Approve and Reject actions.
2. `/api/draft/approve` converts drafts into owned Skilllets.
3. `/api/draft/reject` removes unwanted drafts.
4. Build preview returns structured actions and warnings for richer UI rendering.

v0.6 adds local heuristic extraction:

1. `extract --text` creates Draft Inbox items from high-signal instruction text.
2. `extract --file` does the same for chat logs or session notes.
3. The extractor is local-only and does not call an LLM.
4. Extracted candidates remain drafts until approved.

## Commands

```bash
cargo run -- import --scan-home --project .
cargo run -- ui --project .
cargo run -- mirror --skill superpowers:brainstorming --agent codex --project .
cargo run -- build --preview --project .
cargo run -- build --project .
cargo run -- status --project .
cargo run -- sync --project .
cargo run -- skilllet add --id project:use-axios --title "Use Axios" --body "Use Axios for frontend HTTP requests." --target codex --project .
cargo run -- skilllet list --project .
cargo run -- draft add --id project:prefer-pnpm --title "Prefer pnpm" --body "Use pnpm for package management." --target codex --project .
cargo run -- draft approve --id project:prefer-pnpm --project .
cargo run -- draft reject --id project:prefer-pnpm --project .
cargo run -- extract --text "Always use pnpm for package management." --target codex --project .
cargo run -- extract --file chat.md --target codex --project .
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

# 8. Add an owned Skilllet and compile it into selected Agent instructions.
cargo run -- skilllet add --id project:use-axios --title "Use Axios" --body "Use Axios for frontend HTTP requests." --target codex --project .

# 9. Add and approve a Draft Inbox item.
cargo run -- draft add --id project:prefer-pnpm --title "Prefer pnpm" --body "Use pnpm for package management." --target codex --project .
cargo run -- draft approve --id project:prefer-pnpm --project .

# 10. Extract local heuristic drafts from text or files.
cargo run -- extract --text "Always use pnpm for package management." --target codex --project .
cargo run -- extract --file chat.md --target codex --project .
```

The Canvas UI exposes:

- `/api/state` for project config, imported rules, and skill index
- `/api/mirror` for declaring a Skill mirror
- `/api/build/preview` for build previews
- `/api/sync` for syncing declared mirrors and generated artifacts
- `/api/status` for synced / missing / drifted mirror state

## Generated State

- `.agent-kernel/project.yml` is the declarative project config.
- `.agent-kernel/skilllets/` contains owned lightweight Skilllet source files.
- `.agent-kernel/drafts/` contains local Draft Inbox candidates before approval.
- `.agent-kernel/skill-index.yml` is the imported Skill index.
- `.agent-kernel/project.lock.yml` records generated mirror and artifact hashes.
- `AGENTS.md`, `CLAUDE.md`, and mirrored skill folders are build artifacts.
