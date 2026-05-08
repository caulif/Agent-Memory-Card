# Agent Memory Kernel Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Repair extraction quality, maintainability, data lifecycle, automation, and frontend structure across the current local Agent Memory Kernel baseline.

**Architecture:** Start with repository hygiene and shared text utilities, then make extraction quality measurable, split large Rust/Tauri/frontend modules along existing boundaries, and add indexes, migrations, hooks, and optional LLM refinement behind review-first policy gates.

**Tech Stack:** Rust 2024, Tauri 2, Bun, Vite, React, YAML/JSONL file-native storage, optional provider-backed LLM refinement.

---

## Phase 0: Repository Hygiene

- [x] Update `.gitignore` for root and nested Agent Memory Kernel runtime state.
- [x] Verify `git status --short --untracked-files=all` shows source files and docs, not generated observations, drafts, indexes, audit logs, or Vite logs.
- [x] Run a lightweight format/check command after ignore changes.

## Phase 1: Shared Text Utilities And Semantic Dedupe

- [x] Add `src/textutil.rs` tests for `slug`, `normalize_string_list`, tokenization, and Jaccard similarity.
- [x] Implement `src/textutil.rs` and export it from `src/lib.rs`.
- [x] Replace duplicated `slug` and `normalize_string_list` helpers in `candidate.rs`, `draft.rs`, `extract.rs`, and `observation.rs`.
- [x] Add failing extraction tests for near-duplicate candidates and false-positive non-merges.
- [x] Implement candidate similarity ranking and canonical candidate selection in extraction dedupe.
- [x] Run focused Rust tests for text utilities, candidate domain, draft, observation, and extract behavior.

## Phase 2: Extraction Quality Regression Suite

- [x] Create labeled bilingual fixtures under `tests/fixtures/extract_quality.yml`.
- [x] Add a Rust integration test that reports precision, recall, false positives, and false negatives.
- [x] Cover durable preferences, constraints, procedures, one-off task requests, unresolved questions, correction signals, and noisy tool output.
- [x] Tune existing local rules only where the fixture exposes clear regressions.

## Phase 3: Observation Module Split

- [ ] Create `src/observation/mod.rs` and move public types without changing API names.
- [x] Move import/file parsing helpers out of the main observation surface where low-risk (`conversation.rs`, `incremental.rs`).
- [x] Move `ObservationIndex` and incremental source state to `src/observation/incremental.rs`.
- [x] Move conversation discovery and JSONL parsing to `src/observation/conversation.rs`.
- [x] Move observation unit tests to `src/observation/tests.rs` so the public surface stays focused.
- [x] Run observation and evolve tests after each move.

## Phase 4: Extract Module Split

- [ ] Create `src/extract/mod.rs` and preserve current public functions and structs.
- [x] Move signal predicates to `src/extract/signals.rs`.
- [ ] Move preference registry and template matching to `src/extract/preferences.rs`.
- [x] Move dedupe and scoring helpers to `src/extract/dedupe.rs`.
- [ ] Move high-value prompt and synthesis templates to `src/extract/templates.rs`.
- [x] Move extract unit tests to `src/extract/tests.rs` so the public surface stays focused.
- [x] Run extract tests after each move.

## Phase 5: Data Indexes And Migrations

- [x] Add schema/version fields to Candidate, Draft, and Memory Card records with serde defaults.
- [x] Add a migration module that upgrades older records in memory before save.
- [x] Add `.agent-kernel/index.yml` as a rebuildable read-model cache for counts and record metadata.
- [x] Add Rust and CLI functions to rebuild indexes from source YAML.
- [x] Test index rebuild and stale-index fallback.

## Phase 6: Optional LLM-Assisted Refinement

- [ ] Add provider config for bounded candidate refinement without making it mandatory.
- [ ] Add tests for prompt construction, redaction, size limits, and failure fallback.
- [ ] Route only borderline locally prefiltered candidates to the provider.
- [ ] Write refined output as Candidate/Draft only, never directly as Memory Card.

## Phase 7: Hook Integration

- [x] Add CLI commands to install and uninstall Claude Code hooks.
- [x] Generate hooks that trigger import/evolve jobs for Stop, SessionEnd, or PreCompact events.
- [x] Add dry-run and policy checks before writing hook files.
- [x] Test hook generation paths and idempotent uninstall.

## Phase 8: Tauri Command Split

- [ ] Create `src-tauri/src/commands/` with project, candidate, draft, memory_card, assignment, jobs, quality, and settings modules.
- [x] Move Job Manager state/history logic to `src-tauri/src/jobs.rs`.
- [x] Move Job Center IPC commands to `src-tauri/src/commands/jobs.rs`.
- [x] Move command functions out of `src-tauri/src/lib.rs` while preserving Tauri command names.
- [ ] Keep `app_service.rs` as the business orchestration layer.
- [x] Run Tauri unit tests after command moves.

## Phase 9: Frontend Component Split

- [x] Create component modules for project pages, shared controls, and job center.
- [x] Move logic from `app/src/main.tsx` into component modules without changing behavior.
- [x] Split `styles.css` by surface after components are stable.
- [x] Run Bun tests and build after page extraction.

## Phase 10: Git-Based Memory Card Packs

- [ ] Extend catalog metadata for Git remote, version, and lockfile fields.
- [ ] Add sync/update commands that operate on one project or all registered projects.
- [ ] Test lockfile stability and offline behavior.

## Verification Gates

- [x] `cargo test --quiet`
- [x] `cargo clippy --quiet -- -D warnings`
- [x] `cargo test --manifest-path src-tauri/Cargo.toml --quiet`
- [x] `cargo clippy --manifest-path src-tauri/Cargo.toml --quiet -- -D warnings`
- [x] `bun run --cwd app test`
- [x] `bun run --cwd app build`
- [x] `bun test bun/agent-kernel-lib.test.js`
