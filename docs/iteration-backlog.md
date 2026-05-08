# Agent Memory Kernel Iteration Backlog

This backlog is maintained by the Leader role. It captures self-driven requirements for autonomous iteration and keeps PM, Rust, UI, and QA work aligned.

## Current Product North Star

Agent Memory Kernel is a local-first Memory Card evolution kernel for Claude Code and Codex. It turns local conversations, rules, successful workflows, and project conventions into reviewable, editable, assignable, and compilable Memory Cards.

## v1 Usable Product Reset

The v1 product must optimize for one reliable daily workflow:

1. Select a local Claude Code / Codex project.
2. See a lightweight dashboard immediately.
3. Run local evolution as a visible background job.
4. Review a small, high-value Draft Inbox.
5. Edit, merge, approve, or reject Drafts.
6. Assign approved Memory Cards to Claude Code and/or Codex.
7. Compile and verify generated artifacts.

The storage principle is **file-native local store / no mandatory external DB**. Agent Memory Kernel should not require SQLite, Postgres, a vector database, or a background service, but it may and should maintain structured indexes, read-model caches, job files, and append-only audit logs under `.agent-kernel/` and `~/.agent-kernel/`.

## Active Principles

- Source of truth lives in `.agent-kernel/` and `~/.agent-kernel/`.
- `CLAUDE.md`, `AGENTS.md`, `.claude/skills`, and `.agents/skills` are compiler artifacts.
- UI reads lightweight read models before loading full project snapshots.
- Long-running operations are Jobs, not blocking button clicks.
- AI automation must enter through `KernelCommand` and `KernelPolicy`.
- UI is a control plane; file writes go through Rust IPC.
- Claude Code handles UI polish first; Codex reviews, integrates, and verifies.
- Default automation mode should protect trust before convenience.

## P0 - v1 Architecture Repair

- [x] Add an Application Service layer between Tauri IPC and Rust core modules.
- [x] Add lightweight project dashboard/read-model endpoints for first paint.
- [x] Split heavy snapshot data into dedicated read endpoints: Drafts, Memory Cards, Assignments, Quality, Build Preview.
- [x] Move Drafts, Memory Cards, Catalog, Assignment, and Quality UI surfaces to page-level read models with `ProjectSnapshot` as fallback.
- [x] Promote `DesktopTaskStore` into a Job Manager-compatible status model with job id, stage, logs, timestamps, and result summary.
- [x] Add Job Manager command envelope with job id return values for scan, history evolution, and sync.
- [x] Add retry controls for failed/cancelled scan, evolution, and sync jobs.
- [x] Add cancel request and in-memory job history.
- [x] Persist recent job history to local JSONL under `~/.agent-kernel/jobs/`.
- [x] Persist replay payloads for retryable jobs so Job Center can re-run the exact Tauri command and args.
- [x] Make scan, local conversation evolution, and sync start jobs and return immediately.
- [x] Make Memory Card fusion start a job and return immediately.
- [x] Add Job Center filters by status/type and mark jobs that support precise retry.
- [ ] Make remaining external AI synthesis/refinement actions start jobs and return immediately.
- [ ] Keep Catalog/App Store as secondary until Review Inbox and assignment flow are reliable.
- [ ] Keep Canvas as a later visualization layer; v1 default UI should be Inbox + Library + Assignment Matrix.

## P1 - Trust And Safety

- [x] Enforce Kernel Policy in Tauri mutating commands, not only in UI planning.
- [x] Block or require explicit override when `sync_project` detects generated artifact drift.
- [x] Bind confirmed project mutations to backend-issued decision tokens.
- [x] Consume decision tokens once to prevent replaying high-risk writes.
- [x] Prevent approving a Draft over an existing Memory Card without conflict review.
- [x] Validate Draft and Memory Card editable fields: non-empty title/body, known kind/scope, known agent targets.
- [ ] Keep AI-generated synthesis and fusion in Draft Inbox by default.
- [x] Add audit records for Tauri project mutations, including authorized and blocked decisions.
- [ ] Surface audit records in the desktop UI and add rollback/checkpoint actions.

## P2 - Editing And Governance

- [x] Draft editor can update title, brief, body, kind, scope, tags, and targets.
- [x] Draft targets can be explicitly cleared to represent dormant / inactive.
- [x] Memory Card editor can update title, brief, body, kind, scope, tags, and language.
- [x] UI displays Kernel Policy risk, disposition, and reason before save.
- [ ] Add a global automation mode selector: Manual, Assisted, Guarded Auto, Agent Managed.
- [ ] Persist per-project automation policy in `.agent-kernel/project.yml`.
- [ ] Add source/evidence panel inside editor for Drafts and fusion candidates.
- [ ] Add conflict notes to Draft / Memory Card fusion outputs.

## P3 - AI Fusion And Recommendation

- [ ] Replace simple Memory Card concatenation with engine-backed semantic fusion.
- [ ] Use Claude Code as default fusion engine, Codex as selectable engine, local heuristic as fallback.
- [ ] Generate structured fusion output: title, brief, body, tags, kind, scope, confidence, reason, conflict notes, source IDs.
- [ ] Add project recommendation: recommend global Memory Cards and Skills after project scan/import.
- [ ] Add dormant/promotion candidates based on assignment and repeated usage.

## P4 - Distribution And Ecosystem

- [ ] Prepare MCP server exposing Kernel tools.
- [ ] Add package registry metadata for Memory Card packs.
- [ ] Add import/export for global Memory Card library bundles.
- [ ] Add platform packaging checks for Windows, macOS, and Linux release artifacts.

## Role Cadence

- PM defines next user-visible value and acceptance criteria.
- Rust implements core capability and Tauri IPC.
- UI implements desktop control plane after backend contracts stabilize.
- QA reviews trust, safety, path handling, drift, policy, and cross-platform behavior.
- Leader integrates, verifies, and updates this backlog after each iteration.
