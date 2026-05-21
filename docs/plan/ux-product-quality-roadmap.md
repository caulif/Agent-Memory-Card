# UX Product Quality Roadmap

## Task UQ1: Workflow Cockpit And First-Run Guidance

Priority: P0
Lane: ui
SUPER drivers: Single Purpose, Ports over Implementation, Replaceable Parts

Acceptance criteria:

- A project workflow rail shows the current stage and progress.
- First-run and no-project states explain what to do next without opening documentation.
- The topbar uses user-facing page names and keeps status feedback visible.
- Logic lives in typed utility functions instead of JSX-only calculations.

## Task UQ2: Suggestion Review Clarity

Priority: P0
Lane: ui
SUPER drivers: Single Purpose, Unidirectional Flow, Ports over Implementation

Acceptance criteria:

- The first available suggestion is selected automatically.
- The queue shows evidence/risk/action impact at scan speed.
- The detail pane groups review into decision blocks with visible risk and consequences.
- Empty states point to the next useful action.

## Task UQ3: Memory Card Management Clarity

Priority: P1
Lane: ui
SUPER drivers: Single Purpose, Environment-Agnostic, Replaceable Parts

Acceptance criteria:

- Memory Card library surfaces the most important governance issue first.
- Filter states show count context and a clear reset path.
- Empty and zero-match states reduce user confusion.
- Styling remains responsive at the existing desktop minimum width.

## Verification

- `bun test ./src/utils/review-workbench.test.ts ./src/utils/kernel-plan.test.ts`
- `bun run build`
- Browser smoke check against Vite preview or Tauri dev UI where practical.

