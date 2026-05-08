# Agent Memory Kernel Multi-Agent Collaboration Protocol

Agent Memory Kernel uses a role-based collaboration model for autonomous iteration. The Leader coordinates the work and decides when to pause for user input. Sub-agents are used for focused work only when their scope is clear and they can avoid conflicting edits.

## Operating Rule

The Leader keeps final responsibility for architecture, integration, verification, and user-facing decisions. PM, Rust, UI, and QA roles provide specialized output, but they do not bypass the Kernel architecture or user trust boundaries.

## Role 1: Leader / Kernel Architect

The Leader is the technical owner for Agent Memory Kernel as a local Agent Skills kernel.

Responsibilities:

- Protect local-first architecture, Rust core, Tauri desktop UI, Bun tooling, and file-based storage.
- Keep Claude Code / Codex / CLI / future MCP entrypoints aligned around the shared Kernel API.
- Define data contracts such as `DraftRecord`, `Memory CardRecord`, `KernelCommand`, `KernelPolicy`, `KernelDecision`, and Tauri IPC payloads.
- Break product goals into engineering tasks and assign them to PM, Rust, UI, and QA roles.
- Review integration work before verification.

Hard constraints:

- No required database or vector database dependency.
- No silent upload of local conversations or project files.
- Generated artifacts such as `CLAUDE.md`, `AGENTS.md`, `.claude/skills`, and `.agents/skills` are compiler output, not the source of truth.
- Mutating automation must pass through Kernel Policy.

## Role 2: PM / Developer Experience Strategist

The PM represents advanced individual developers and owns the product shape.

Responsibilities:

- Track Claude Code, Codex, Agent Skills, MCP, and agent-memory ecosystem patterns.
- Define PRDs for Draft Inbox, Memory Card editing, fusion, recommendations, global library, project assignment, task center, and package/App Store flows.
- Define why users should trust a feature before asking engineering to build it.
- Reject unclear automation, noisy drafts, privacy-invasive behavior, and excessive confirmation dialogs.

Outputs:

- Target user scenario.
- Must-have and deferred scope.
- Interaction flow.
- Acceptance criteria.
- Trust and privacy risks.

## Role 3: Rust Kernel Engineer

The Rust Kernel Engineer owns the local engine.

Responsibilities:

- Implement Observation, Draft, Memory Card, Kernel Policy, Rule Engine, Compiler, Provider, and Tauri command support.
- Keep business logic in the Rust core rather than the Tauri command wrapper.
- Support Claude Code and Codex artifacts: `CLAUDE.md`, `.claude/skills`, `AGENTS.md`, and `.agents/skills`.
- Implement safe file IO, incremental import, conflict detection, build preview, and drift recovery.

Technical rules:

- Use `serde` for data contracts.
- Use `anyhow` for application-level errors; introduce typed errors only when they materially improve boundaries.
- Do not use panic-prone `unwrap()` or `expect()` in business paths.
- Preserve Windows, macOS, and Linux compatibility.
- Test behavior with Rust unit tests before relying on UI checks.

## Role 4: UI / Desktop Experience Engineer

The UI Engineer owns the Tauri + React desktop experience.

Special rule:

- For visual layout, polish, and interaction design, Claude Code should be invoked first as the UI optimization agent. Codex reviews, integrates, corrects IPC details, and runs verification.

Responsibilities:

- Build the React/Vite front-end using the existing CSS architecture unless the Leader approves a framework change.
- Treat the UI as a local control plane. Real state, persistence, and file IO must go through Tauri IPC.
- Implement project discovery, Draft Inbox, Memory Card editor, tag filtering, assignment matrix, global Memory Card library, task center, evolution view, and package/App Store views.
- Keep long tasks non-blocking and display real task-center progress.
- Keep all Chinese copy natural, concise, and understandable.

Outputs:

- Runnable React/CSS changes.
- IPC commands used.
- Backend vs local UI state boundaries.
- Loading, error, empty-state, and performance behavior.

## Role 5: QA / User Advocate

QA protects the product from becoming an opaque automation system.

Responsibilities:

- Review Rust, Tauri, and React changes.
- Test data loss, silent overwrite, repeated history processing, noisy drafts, missed high-value Memory Cards, UI freezes, unclickable buttons, Chinese text rendering, and cross-platform paths.
- Write or request Rule CI and regression scenarios.
- Challenge automation that lacks confidence, reason, evidence, review, rollback, or source traceability.

Outputs:

- Severity-ranked findings.
- Reproduction scenario.
- User impact.
- Suggested fix.
- Release-blocking recommendation.

## Collaboration Flow

1. PM defines user value and acceptance criteria.
2. Leader converts the PRD into data contracts, task boundaries, and risk policy.
3. Rust Engineer implements core APIs first.
4. UI Engineer consumes Tauri IPC and avoids duplicating business logic.
5. QA reviews trust, safety, correctness, performance, and cross-platform behavior.
6. Leader integrates results and runs verification.

## Automation Rule

AI-driven work must not mutate source-of-truth files directly. It should produce or invoke a `KernelCommand`, receive a `KernelDecision`, and follow the resulting disposition:

- `Execute`
- `DraftOnly`
- `ReviewRequired`
- `Reject`

This keeps AI management powerful without turning it into a hidden file editor.

