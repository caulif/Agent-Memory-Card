# Agent-Kernel v1 Usable Architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Re-center Agent-Kernel around a fast, trustworthy project workflow: select project, evolve local evidence into a small review inbox, edit/approve Skilllets, assign them to Claude Code/Codex, then compile and verify artifacts.

**Architecture:** Keep the Rust core and file-native storage, but add an Application Service layer between Tauri IPC and core modules. UI reads lightweight dashboard/read-model endpoints first, starts long work as jobs, and loads heavy checks such as Rule CI and build preview only after the first paint.

**Tech Stack:** Rust core, Tauri 2 IPC, Bun + Vite + React UI, YAML/JSONL file-native storage, no mandatory external DB.

---

## File Structure

- `agent-kernel-memory-gc-idea.md`: product and architecture source-of-truth, updated to v1 language.
- `docs/iteration-backlog.md`: execution backlog, updated to prioritize Application Service, Job Manager, read models, and review inbox quality.
- `src-tauri/src/app_service.rs`: new Tauri application service layer for read models and use-case orchestration.
- `src-tauri/src/lib.rs`: wires `app_service` into IPC and tests the new policy/read model behavior.
- `app/src/ui-helpers.ts`: adds `ProjectDashboard` frontend type.
- `app/src/main.tsx`: loads project dashboard immediately on selection and keeps full snapshot as background detail state.
- `app/tests/ui-helpers.test.ts`: guards the UI against regressing to full snapshot as the project-switching path.

## Task 1: Planning And Product Language

- [x] Replace absolute “zero database” wording with “file-native local store / no mandatory external DB”.
- [x] Add the v1 main workflow: Project List -> Project Dashboard -> Evolution Job -> Review Inbox -> Assignment Matrix -> Compile.
- [x] Demote Catalog/App Store and Canvas to post-core workflow surfaces until the inbox and assignment loop is reliable.
- [x] Add acceptance criteria: project switch first paint stays lightweight; history evolution runs as a visible background job; review inbox defaults to high-value top candidates.

## Task 2: Lightweight Dashboard Read Model

- [x] Write a failing Tauri/Rust test proving `load_project_dashboard` returns counts and metadata without requiring build preview or Rule CI.
- [x] Create `src-tauri/src/app_service.rs` with:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct ProjectDashboard {
    pub project_path: String,
    pub draft_count: usize,
    pub skilllet_count: usize,
    pub observation_count: usize,
    pub global_skilllet_count: usize,
    pub enabled_agents: Vec<String>,
    pub warning_count: usize,
}

pub fn load_project_dashboard(project_root: &Path, home: &Path) -> anyhow::Result<ProjectDashboard>;
```

- [x] Add `get_project_dashboard(project_path, home)` Tauri command.
- [x] Keep `get_project_snapshot` available for detail pages, but remove it from the critical path for first project selection.

## Task 3: UI First Paint

- [x] Add `ProjectDashboard` to `app/src/ui-helpers.ts`.
- [x] In `app/src/main.tsx`, maintain `dashboard` state separate from `snapshot`.
- [x] On refresh, scan, and project switch, load dashboard first, then load full snapshot in the background.
- [x] `ProjectOverviewStrip` should show dashboard counts immediately and upgrade to snapshot counts when available.
- [x] Guard stale dashboard and snapshot responses with the existing request-id/current-project pattern.

## Task 4: Job Manager Follow-Up

- [x] Promote `DesktopTaskStore` into a Job Manager-compatible status model with job id, stage, progress, logs, start/end timestamps, and result summary.
- [x] Make `evolve_project`, `scan_projects`, and `sync_project` return a `job_id` command envelope for long-running work.
- [x] UI should render a Job Center drawer rather than a single global progress pill.
- [x] Add cancel request controls after the job-id command envelope is introduced.
- [x] Make `scan_projects` and `sync_project` truly non-blocking by moving filesystem work to background jobs.
- [x] Persist recent job history to local JSONL under `~/.agent-kernel/jobs/`.
- [x] Add retry controls for failed/cancelled scan, evolution, and sync jobs.
- [x] Store precise job replay payloads in history so retries invoke the original command with the original project path, targets, engine, and policy.
- [x] Move Skilllet fusion into a non-blocking job with replay metadata and automatic read-model refresh on completion.
- [x] Add Job Center status/type filters and visible precise-retry badges.

## Task 4.5: Page-Level Read Models

- [x] Add `ProjectReviewInbox` for Draft Inbox-only loading.
- [x] Add `ProjectSkillletLibrary` for project/global Skilllets and catalog status.
- [x] Add `ProjectAssignmentView` for enabled agents and Skilllet target matrix.
- [x] Add `ProjectQualityView` for Rule CI, build preview, and status warnings.
- [x] Move Drafts, Skilllets, Catalog, Assignment, and Quality surfaces from full `ProjectSnapshot` to page-level read models with fallback.

## Task 5: Review Inbox Quality Follow-Up

- [ ] Split evolution into Observation -> Candidate -> Draft files.
- [ ] Persist candidate scores and rejection reasons.
- [ ] Default Draft Inbox to top 20 high-value candidates, with filters for all/hidden/noise.
- [ ] Let Claude Code/Codex refine only locally prefiltered candidates, never raw full-history material.

## Self-Review

- Spec coverage: Tasks 1-5 cover storage language, Application Service, read models, background jobs, and Draft quality.
- Placeholder scan: no TBD/TODO placeholders remain; follow-up tasks are explicit and scoped.
- Type consistency: `ProjectDashboard` uses snake_case-compatible serde fields matching frontend `ProjectDashboard`.
