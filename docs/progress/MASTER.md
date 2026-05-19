# Skills And Memory Linkage Progress

Task: Manage Skills as first-class assets and link high-value Memory Cards into Skills as generated supplemental guidance.

Mode: GITHUB_FULL
Repository: caulif/Agent-Memory-Card
Branch: codex/skills-memory-linkage

## Artifacts

- Analysis: `docs/analysis/skills-memory-linkage-research.md`
- Plan: `docs/plan/skills-memory-linkage-roadmap.md`
- Skill: `.codex/skills/agent-kernel-production-dev/SKILL.md`

## Milestone

- Skills And Memory Linkage: Issues #28, #26, and #27

## Issue Mapping

| Task | Issue | Status |
| ---- | ----- | ------ |
| SML1 Skill Library Read Model | #28 | done |
| SML2 Skills Page And Linkage Actions | #26 | done |
| SML3 Skill Supplement Preview Clarity | #27 | done |

## Current Status

- Active task: Complete
- Completed: Skill Library read model, Skills page, Memory Card linkage actions, supplement clarity copy, responsive smoke checks, and Windows packaging.
- Build artifact: `src-tauri/target/release/agent-kernel-desktop.exe`
- Installer artifacts:
  - `src-tauri/target/release/bundle/nsis/Agent Memory Kernel_0.1.0_x64-setup.exe`
  - `src-tauri/target/release/bundle/msi/Agent Memory Kernel_0.1.0_x64_en-US.msi`

## Verification

- `cargo test -p agent-kernel-desktop` — 44 passed
- `bun test ./src/utils/review-workbench.test.ts ./src/utils/kernel-plan.test.ts` — 28 passed
- `bun run build` in `app` — passed
- `bun run tauri:build` — passed
- Playwright smoke screenshots:
  - `output/playwright/skills-page.png`
  - `output/playwright/skills-page-mobile.png`

## Quick Status Commands

```powershell
$env:HTTP_PROXY="http://127.0.0.1:7897"
$env:HTTPS_PROXY="http://127.0.0.1:7897"
$env:ALL_PROXY="http://127.0.0.1:7897"
gh issue list -R caulif/Agent-Memory-Card --label "spec-driven" --state all
```
