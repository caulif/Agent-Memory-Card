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

v0.7 exposes extraction in the Canvas UI:

1. The right-side panel has an Extract Drafts text box.
2. `/api/extract` writes local heuristic candidates into Draft Inbox.
3. Extracted drafts can be approved or rejected from the UI.

v0.8 starts the Hybrid provider foundation:

1. `provider init` writes `.agent-kernel/providers.yml`.
2. `provider show` displays local-first provider config.
3. The config includes `local` and `openai-compatible` slots plus privacy defaults.
4. Non-local providers are config scaffolding for now; extraction remains local-only.

v0.9 adds extraction preview controls:

1. `extract --provider local` selects the local heuristic provider.
2. `extract --dry-run` shows candidate drafts without writing Draft Inbox files.
3. UI extraction supports selecting enabled Agent targets instead of hardcoding Codex.

v0.10 adds a local privacy safety layer:

1. Provider privacy config defaults to `redact_secrets: true`.
2. Extract preview and Draft evidence redact common token/key shapes.
3. Reports show when secrets were redacted.

v0.11 starts Rule CI:

1. `.agent-kernel/tests/*.yml` defines local assertions for generated Agent artifacts.
2. `test-rules` checks include/exclude expectations without calling an LLM.
3. This is the local foundation for future model-judged Rule CI.

v0.12 brings Rule CI into the UI:

1. `/api/rule-tests` returns structured Rule CI results.
2. The Canvas inspector shows pass/fail status.
3. The footer has a Rule CI action beside build preview and sync.

v0.13 starts review mode:

1. `review` aggregates Draft Inbox, Mirror Status, Rule CI, and Build Preview.
2. This is the non-interactive foundation for the future `git add -p` style reviewer.

v0.14 starts the release/distribution foundation:

1. `agent-kernel --version` reports the Cargo package version.
2. The npm entrypoint first looks for packaged binaries under `bin/<platform>-<arch>/`.
3. Local development still falls back to `cargo build` when no packaged binary exists.
4. Tag builds assemble Windows, Linux, and macOS artifacts plus an npm tarball.

v0.15 starts the interactive review protocol foundation:

1. `review --json` returns machine-readable Draft Inbox, Mirror Status, Rule CI, and Build Preview state.
2. `review --approve-draft <id>` applies a Draft Inbox approval before rendering the report.
3. `review --reject-draft <id>` removes an unwanted draft before rendering the report.
4. These flags are the scriptable base for a later `git add -p` style CLI.

v0.16 brings review into the Canvas:

1. `/api/review` exposes the same structured review report to the UI.
2. The inspector shows pending Drafts, Rule CI failures, build action count, and warnings.
3. The footer Review button refreshes the review report without leaving the Canvas.

v0.17 starts the Skilllet Catalog / App Store foundation:

1. `catalog list` shows built-in local Skilllet packages.
2. `catalog init` writes `.agent-kernel/catalog.yml` so users can customize the local catalog.
3. `catalog install --id <package>` installs a catalog package as an owned Skilllet.
4. The Canvas App Store now separates installable Catalog Packages from mirrored Indexed Skills.

v0.18 improves App Store trust feedback:

1. Catalog APIs now report whether each package is already installed.
2. `catalog list` labels packages as `available` or `installed`.
3. The Canvas disables already-installed package buttons.

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
cargo run -- extract --text "Always run cargo test before pushing." --target codex --provider local --dry-run --project .
cargo run -- provider init --project .
cargo run -- provider show --project .
cargo run -- extract --text "Always use token=supersecret123456789 before pushing." --target codex --provider local --dry-run --project .
cargo run -- test-rules --project .
cargo run -- review --project .
cargo run -- review --json --project .
cargo run -- review --approve-draft project:prefer-pnpm --project .
cargo run -- review --reject-draft project:prefer-pnpm --project .
cargo run -- catalog list --project .
cargo run -- catalog init --project .
cargo run -- catalog install --id core:rust-quality-gate --target codex --project .
```

The npm wrapper works locally after the Rust binary has been built:

```bash
node npm/agent-kernel.js scan --scan-home --project .
node npm/agent-kernel.js --version
```

For a packaged `npx` flow, the wrapper resolves binaries in this order:

1. `bin/<platform>-<arch>/agent-kernel[.exe]`
2. `target/release/agent-kernel[.exe]`
3. `target/debug/agent-kernel[.exe]`

If none exist, it runs `cargo build` as a development fallback.

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

# 11. Initialize Hybrid provider config and preview extraction payloads locally.
cargo run -- provider init --project .
cargo run -- extract --text "Always run cargo test before pushing." --target codex --provider local --dry-run --project .

# 12. Redaction is enabled by default for extraction previews and draft evidence.
cargo run -- extract --text "Always use token=supersecret123456789 before pushing." --target codex --provider local --dry-run --project .

# 13. Run local Rule CI assertions against generated artifacts.
cargo run -- test-rules --project .

# 14. Review pending changes before sync/build.
cargo run -- review --project .

# 15. Use review as a scriptable protocol for future interactive CLI/UI decisions.
cargo run -- review --json --project .
cargo run -- review --approve-draft project:prefer-pnpm --project .
```

The Canvas UI exposes:

- `/api/state` for project config, imported rules, and skill index
- `/api/mirror` for declaring a Skill mirror
- `/api/extract` creates Draft Inbox items from pasted text
- `/api/build/preview` for build previews
- `/api/sync` for syncing declared mirrors and generated artifacts
- `/api/status` for synced / missing / drifted mirror state
- `/api/rule-tests` for structured Rule CI results
- `/api/review` for the unified review protocol used by the Canvas
- `/api/catalog` and `/api/catalog/install` for local App Store packages

## Generated State

- `.agent-kernel/project.yml` is the declarative project config.
- `.agent-kernel/providers.yml` stores local-first provider and privacy settings.
- `.agent-kernel/skilllets/` contains owned lightweight Skilllet source files.
- `.agent-kernel/drafts/` contains local Draft Inbox candidates before approval.
- `.agent-kernel/tests/` contains local Rule CI assertion files.
- `.agent-kernel/catalog.yml` optionally overrides the built-in local Skilllet catalog.
- `.agent-kernel/skill-index.yml` is the imported Skill index.
- `.agent-kernel/project.lock.yml` records generated mirror and artifact hashes.
- `AGENTS.md`, `CLAUDE.md`, and mirrored skill folders are build artifacts.
