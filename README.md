# Agent-Kernel

Agent-Kernel v0.1 is a Rust-built, project-centered workspace for managing Claude Code and Codex rules and Skills.

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
2. The Bun entrypoint first looks for packaged binaries under `bin/<platform>-<arch>/`.
3. Local development still falls back to `cargo build` when no packaged binary exists.
4. Tag builds assemble Windows, Linux, and macOS artifacts plus a Bun-compatible package tarball.

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

v0.19 adds catalog provenance:

1. Catalog packages now include `version`, `source_url`, and `tags`.
2. `catalog list` prints package versions and source URLs.
3. The Canvas App Store shows package provenance before installation.

v0.20 adds a local Catalog trust gate:

1. `catalog validate` checks duplicate IDs, missing versions, missing sources, empty package bodies, and missing tags.
2. `/api/catalog/validate` exposes the same validation report to the Canvas.
3. The App Store shows Catalog Health before package installation.

v0.21 adds instruction budget warnings:

1. Build preview warns when generated Agent instruction artifacts exceed 32 KiB.
2. The budget follows Codex-style project document constraints and helps prevent prompt bloat early.

v0.22 explored the generic rules exporter:

1. Agents with `rules_dir` can receive generated rule artifacts.
2. The core keeps a generic extension point for future adapters.
3. Cursor is no longer part of the default MVP target set.

v0.23 adds Agent target controls:

1. `agent list` shows configured Agent targets and enabled state.
2. `agent enable --agent <name>` and `agent disable --agent <name>` update `project.yml`.
3. The Canvas Agent nodes expose Enable/Disable controls.

v0.24 adds Skilllet target assignment:

1. `skilllet targets --id <skilllet> --target <agent>` updates a Skilllet's Agent targets.
2. `/api/skilllet/targets` exposes the same assignment operation to the Canvas.
3. Skilllet cards show Agent target buttons for quick multi-agent assignment.

v0.25 adds a Skilllet target matrix:

1. `skilllet matrix` prints a Skilllet by Agent assignment table.
2. `/api/skilllet/matrix` exposes the matrix to the Canvas.
3. The inspector shows a compact target summary for each Skilllet.

v0.26 explored a Cline-style rules exporter:

1. The generic rules exporter can write Markdown rules into custom agent directories.
2. Cline is no longer part of the default MVP target set.
3. Future non-core agents should be added through adapters instead of default project config.

v0.27 improves the Canvas target matrix:

1. The inspector now renders a real Skilllet by Agent table instead of a compact text list.
2. Matrix cells show assigned or unassigned state at a glance.
3. Clicking a cell updates that Skilllet's Agent targets through the existing assignment API.

v0.28 adds generated artifact drift detection:

1. `status` now checks generated instruction/rules artifacts recorded in `project.lock.yml`.
2. Artifacts are reported as `synced`, `missing`, or `artifact drifted`.
3. The Canvas shows an Artifact Status panel and Review summary counts artifact drifts.

v0.29 starts Reverse Parse:

1. `import --artifacts` turns manual edits in generated artifacts into Draft Inbox items.
2. The importer re-renders the expected artifact, extracts added lines from the edited file, and targets the draft to the owning Agent.
3. The Canvas footer exposes the same flow through Import Artifacts.

v0.30 resets the MVP scope:

1. Default targets are Claude Code and Codex only.
2. Cursor and Cline are removed from the default project config and Canvas target set.
3. The generic `rules_dir` exporter remains as an extension interface for future adapters.

v0.31 adds richer Skilllet operations:

1. `skilllet merge` combines multiple Skilllets into a new reviewable Skilllet asset.
2. `skilllet attach-skill` attaches a Skilllet as a generated supplement to an existing mirrored Skill.
3. Mirrored Skills receive `AGENT_KERNEL_SKILLLETS.md` supplements without modifying the original Skill source.

v0.32 starts the Observation Layer:

1. Raw observations live under `.agent-kernel/observations`.
2. `observe import --file <path>` imports a local transcript or note file with secret redaction.
3. `observe local` scans common Claude Code and Codex local JSONL conversation folders and stores observations for later Skilllet synthesis.

v0.33 closes the first local evolution loop:

1. `observe synthesize` turns imported Observations into Draft Inbox candidates.
2. Synthesis uses the same local heuristic extractor and never auto-enables a Skilllet.
3. The Canvas Observations panel can synthesize reviewable drafts for all enabled Agents.

v0.34 adds the first one-command evolution path:

1. `observe evolve` scans local Claude Code / Codex sessions and synthesizes Draft Inbox items.
2. The command still stops at Draft Inbox, so the user keeps final approval control.
3. `--dry-run` imports Observations and previews candidate counts without writing Drafts.

v0.35 makes repeated evolution safer:

1. Observation imports are ID-deduplicated.
2. Re-importing the same transcript reports `skipped` instead of pretending a new Observation was created.
3. Repeated `observe evolve` runs become easier to trust in daily use.

v0.36 switches the JavaScript package wrapper to Bun:

1. The package entrypoint now lives under `bun/`.
2. Local scripts use `bun` and `bun test`.
3. Release packaging uses `bun pm pack`, while the Rust binary remains the core implementation.
4. This repository's own package-management Skilllet now prefers Bun.

v0.37 improves Skilllet synthesis for package-manager preferences:

1. Local extraction now recognizes high-confidence Bun package-management corrections.
2. Phrases like "move from npm to Bun" normalize to `project:prefer-bun`.
3. The generated Draft uses the stable title `Prefer Bun` and a concise machine-oriented body.

v0.38 improves Skilllet synthesis for HTTP client preferences:

1. Local extraction now recognizes common Fetch -> Axios corrections.
2. Frontend request preferences normalize to `project:use-axios`.
3. The generated Draft uses the stable title `Use Axios` and the concise body already used by project Skilllets.

v0.39 starts the Known Preference Registry:

1. High-confidence local extraction rules are now table-driven instead of one-off conditionals.
2. Bun, Axios, and Vitest preferences share the same stable title/body matching path.
3. Common frontend unit-test corrections now normalize to `project:use-vitest`.

v0.40 improves evolution report explainability:

1. `observe synthesize` now reports the exact Draft IDs it creates.
2. `observe evolve` carries those Draft IDs through the one-command local evolution report.
3. CLI and JSON consumers can show "what changed" instead of only counts.

v0.41 improves dry-run evolution previews:

1. Observation synthesis dry-runs now report candidate Draft IDs before writing files.
2. One-command local evolution dry-runs carry the same candidate Draft IDs.
3. This makes preview/review flows suitable for UI confirmation before Draft Inbox writes.

v0.42 makes Known Preference Registry project-configurable:

1. Projects can add `.agent-kernel/preference-registry.yml` to define custom high-confidence extraction templates.
2. Project templates use `title`, `body`, `required`, and `context` fields.
3. Project templates are matched before built-ins, so advanced users can override default Bun/Axios/Vitest wording.

## Commands

```bash
cargo run -- import --scan-home --project .
cargo run -- import --artifacts --project .
cargo run -- ui --project .
cargo run -- mirror --skill superpowers:brainstorming --agent codex --project .
cargo run -- build --preview --project .
cargo run -- build --project .
cargo run -- status --project .
cargo run -- sync --project .
cargo run -- skilllet add --id project:use-axios --title "Use Axios" --body "Use Axios for frontend HTTP requests." --target codex --project .
cargo run -- skilllet list --project .
cargo run -- skilllet targets --id project:use-axios --target codex --target claude-code --project .
cargo run -- skilllet merge --id project:frontend-defaults --title "Frontend Defaults" --source project:use-axios --source project:prefer-bun --target codex --project .
cargo run -- skilllet attach-skill --id project:use-axios --skill superpowers:brainstorming --project .
cargo run -- skilllet matrix --project .
cargo run -- draft add --id project:prefer-bun --title "Prefer Bun" --body "Use Bun for JavaScript package management and scripts." --target codex --project .
cargo run -- draft approve --id project:prefer-bun --project .
cargo run -- draft reject --id project:prefer-bun --project .
cargo run -- extract --text "Always use Bun for JavaScript package management and scripts." --target codex --project .
cargo run -- extract --text "以后把 npm 改为 Bun，所有 JS 脚本都用 bun run。" --target codex --dry-run --project .
cargo run -- extract --text "以后前端请求统一使用 Axios，不要再用 Fetch。" --target codex --dry-run --project .
cargo run -- extract --text "以后前端单元测试默认使用 Vitest，不要再写 Jest 配置。" --target codex --dry-run --project .
cargo run -- extract --file chat.md --target codex --project .
cargo run -- extract --text "Always run cargo test before pushing." --target codex --provider local --dry-run --project .
cargo run -- observe import --file chat.jsonl --agent claude-code --source-kind claude-code-session --project .
cargo run -- observe local --project .
cargo run -- observe list --project .
cargo run -- observe synthesize --target codex --target claude-code --project .
cargo run -- observe synthesize --dry-run --project .
cargo run -- observe evolve --target codex --target claude-code --project .
cargo run -- observe evolve --dry-run --project .
cargo run -- provider init --project .
cargo run -- provider show --project .
cargo run -- extract --text "Always use token=supersecret123456789 before pushing." --target codex --provider local --dry-run --project .
cargo run -- test-rules --project .
cargo run -- review --project .
cargo run -- review --json --project .
cargo run -- review --approve-draft project:prefer-bun --project .
cargo run -- review --reject-draft project:prefer-bun --project .
cargo run -- catalog list --project .
cargo run -- catalog init --project .
cargo run -- catalog validate --project .
cargo run -- catalog install --id core:rust-quality-gate --target codex --project .
cargo run -- agent list --project .
cargo run -- agent enable --agent claude-code --project .
cargo run -- agent disable --agent claude-code --project .
```

Example project preference registry:

```yaml
preferences:
  - title: Use Playwright
    body: Use Playwright for browser automation tests.
    required:
      - playwright
    context:
      - cypress
      - browser automation
      - 浏览器自动化
```

The Bun wrapper works locally after the Rust binary has been built:

```bash
bun bun/agent-kernel.js scan --scan-home --project .
bun bun/agent-kernel.js --version
```

For a packaged `bunx` flow, the wrapper resolves binaries in this order:

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
cargo run -- draft add --id project:prefer-bun --title "Prefer Bun" --body "Use Bun for JavaScript package management and scripts." --target codex --project .
cargo run -- draft approve --id project:prefer-bun --project .

# 10. Extract local heuristic drafts from text or files.
cargo run -- extract --text "Always use Bun for JavaScript package management and scripts." --target codex --project .
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
cargo run -- review --approve-draft project:prefer-bun --project .
```

The Canvas UI exposes:

- `/api/state` for project config, imported rules, and skill index
- `/api/mirror` for declaring a Skill mirror
- `/api/agent/enabled` for enabling or disabling Agent targets
- `/api/skilllet/targets` for assigning Skilllets to Agent targets
- `/api/skilllet/matrix` for the Skilllet by Agent target matrix
- `/api/extract` creates Draft Inbox items from pasted text
- `/api/observations/synthesize` creates Draft Inbox items from imported Observations
- `/api/build/preview` for build previews
- `/api/sync` for syncing declared mirrors and generated artifacts
- `/api/status` for synced / missing / drifted mirror state
- `/api/rule-tests` for structured Rule CI results
- `/api/review` for the unified review protocol used by the Canvas
- `/api/catalog` and `/api/catalog/install` for local App Store packages
- `/api/catalog/validate` for local Catalog trust checks

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
- Custom future adapters can use `rules_dir` to generate Markdown rule artifacts.
