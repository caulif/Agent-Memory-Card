# Agent-Kernel Optimization Design

## Goal

Optimize the current local Agent-Kernel codebase as the baseline: keep the Tauri + React desktop direction, improve extraction quality, reduce candidate duplication, split oversized modules, add data/index migration paths, and prepare safe automation hooks.

## Baseline

The local worktree is the source of truth. The removed native UI files are not restored. The new `app/`, `src-tauri/`, `src/candidate.rs`, `src/kernel/`, and `tests/` areas are treated as active product code.

Local runtime state must stay out of source control. Root `.agent-kernel/project.yml`, providers, skilllets, and rule tests remain source data. Generated observations, drafts, candidates, audit logs, indexes, nested `.agent-kernel/` folders, Vite logs, build output, and dependency folders are local artifacts.

## Architecture

Keep the file-native Rust core and Tauri application service. Add focused utility and quality modules before larger refactors:

- `textutil`: shared slugging, list normalization, tokenization, and similarity helpers.
- `extract` submodules: extraction engine, signal detection, preference templates, dedupe, and quality tests.
- `observation` submodules: import, incremental state, conversation parsing, synthesis.
- Tauri command modules under `src-tauri/src/commands/`, with `app_service` remaining the orchestration layer.
- Frontend page/component modules under `app/src/components/` and page-specific style files.

## Extraction Quality

The default path remains local and deterministic. The first improvement is near-duplicate detection using normalized tokens and Jaccard similarity within compatible candidate kind/scope groups. This reduces repeated suggestions such as equivalent Axios or Bun preferences without merging unrelated rules.

Quality becomes measurable through a small checked-in fixture suite that labels durable rules, one-off task chatter, unresolved requests, repeated corrections, and bilingual preferences. Precision and recall should be reported by tests before adding heavier extraction behavior.

LLM assistance is optional and later. It receives only locally prefiltered borderline material, bounded by character limits, redacted through the existing provider layer, and writes only Candidates/Drafts for review.

## Data And Performance

Add schema/version fields where missing and a migration entry point for `.agent-kernel/` YAML data. Add a lightweight index/read-model cache so large projects do not repeatedly traverse every draft, candidate, observation, and skilllet file for common views.

Indexes are accelerators, not source of truth. They can be rebuilt from YAML records.

## Automation

Hook integration should install, validate, and uninstall Claude Code hooks for Stop, SessionEnd, or PreCompact workflows. Hooks trigger import/evolve jobs and create reviewable candidates only. They do not approve Drafts, write Skilllets, or compile artifacts without Kernel Policy authorization.

## Frontend

Split the oversized `app/src/main.tsx` into project selection, dashboard, review inbox, skilllet library, assignment view, quality view, and job center components. Keep the current interaction model stable while moving code into focused modules. Styling should be split by surface without changing the visual system in the first pass.

## Acceptance Criteria

- Local runtime artifacts no longer pollute `git status`.
- Candidate dedupe merges obvious near duplicates while preserving distinct rules.
- Extraction quality tests expose precision/recall for a labeled fixture suite.
- `extract.rs`, `observation.rs`, `src-tauri/src/lib.rs`, and `app/src/main.tsx` are reduced through behavior-preserving module splits.
- Data migrations and indexes are rebuildable and tested.
- Hook automation is opt-in and review-first.
- Existing Rust, Tauri, Bun, and frontend tests pass after each phase.
