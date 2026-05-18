# Spec-Driven Progress: Agent Memory Kernel Production Tooling

> Initialized 2026-05-18 by spec-driven-develop.

## Task

Make Agent Memory Kernel feel like a practical production tool for its owner and early Windows users by hardening one daily workflow:

```text
Select project -> run evolution -> review evidence-backed suggestions
-> approve/edit/reject -> assign -> preview artifact diff -> sync -> verify/recover
```

## Tracking

- Mode: `GITHUB_FULL`
- Repository: `caulif/Agent-Memory-Card`
- GitHub Project URL: https://github.com/users/caulif/projects/1
- Project: `Spec: Agent Memory Kernel Production Tooling`
- Current active issue: none

## Analysis Artifacts

- `docs/archives/agent-kernel-production-tooling/analysis/project-overview.md`
- `docs/archives/agent-kernel-production-tooling/analysis/module-inventory.md`
- `docs/archives/agent-kernel-production-tooling/analysis/risk-assessment.md`

## Plan Artifacts

- `docs/archives/agent-kernel-production-tooling/plan/production-improvement-roadmap.md`

## Execution Skill

- `.codex/skills/agent-kernel-production-dev/SKILL.md`

## Milestones

- [x] Phase 1: Daily Workflow Reset (4/4 tasks + adaptive rescope) - https://github.com/caulif/Agent-Memory-Card/milestone/1
- [x] Phase 2: Trust, Evidence, And Evaluation (3/3 tasks) - https://github.com/caulif/Agent-Memory-Card/milestone/2
- [x] Phase 3: Artifact Sync And Recovery Hardening (3/3 tasks) - https://github.com/caulif/Agent-Memory-Card/milestone/3
- [x] Phase 4: Onboarding And Distribution (3/3 tasks) - https://github.com/caulif/Agent-Memory-Card/milestone/4
- [x] Phase 5: Workflow-Shaped Architecture Cleanup (3/3 tasks) - https://github.com/caulif/Agent-Memory-Card/milestone/5

## Issue Mapping

| Task ID | Issue | Status |
| --- | --- | --- |
| T1.1 | #1 Define the canonical v1 workflow contract | Closed |
| T1.2 | #2 Make Review Inbox evidence-first | Closed |
| T1.3 | #3 Collapse Candidate/Draft language in UI | Closed |
| T1.4 | #4 Make Dashboard a next-action cockpit | Closed |
| Adaptive | #17 Scope Re-evaluation Required for Phase 1 | Closed |
| T2.1 | #5 Promote Eval Run to a first-class read model | Closed |
| T2.2 | #6 Capture real provider evidence validity | Closed |
| T2.3 | #7 Build a rejection-learning loop | Closed |
| T3.1 | #8 Add per-file drift operations to the UI | Closed |
| T3.2 | #9 Add sync checkpoints and rollback guidance | Closed |
| T3.3 | #10 Verify generated artifacts after sync | Closed |
| T4.1 | #11 Create first-run setup checklist | Closed |
| T4.2 | #12 Release smoke test workflow | Closed |
| T4.3 | #13 Public quickstart rewrite around one workflow | Closed |
| T5.1 | #14 Split Tauri command families | Closed |
| T5.2 | #15 Split extraction pipeline contracts | Closed |
| T5.3 | #16 Split frontend features and styles | Closed |

## Current Status

- [x] Phase 0: intent capture
- [x] Phase 1: deep project analysis
- [x] Phase 2: user confirmation of refined scope
- [x] Phase 3: task decomposition and GitHub sync
- [x] Phase 4: progress tracking
- [x] Phase 5: task-specific project skill
- [x] Implementation complete: all planned GITHUB_FULL issues closed

## Execution Telemetry Reference

- Issue comments store per-task telemetry.
- Milestone descriptions store adaptive control state.
- Phase 1 thresholds: annotate=1, replan=2, rescope=3.
- Phase 1 drift_score: 4. Issue #17 records adaptive rescope.
- Rescope decision: continue Phase 1, but constrain #4 to cockpit integration for the already-expanded workflow/product-language surface.

## Quick Status Commands

```powershell
$env:HTTP_PROXY="http://127.0.0.1:7897"
$env:HTTPS_PROXY="http://127.0.0.1:7897"
$env:ALL_PROXY="http://127.0.0.1:7897"
gh issue list --repo caulif/Agent-Memory-Card --label spec-driven --state all
gh project view 1 --owner caulif --format json
```
