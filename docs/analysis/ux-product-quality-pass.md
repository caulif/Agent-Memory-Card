# UX Product Quality Pass

Date: 2026-05-19
Mode: GITHUB_FULL
Branch: codex/ui-product-quality-pass

## Intent

Improve Agent Memory Kernel as a practical desktop tool by reducing first-run friction, making the current workflow state obvious, and increasing the clarity of review and Memory Card management panels.

## External UX Baseline

NN/g's 10 usability heuristics are the baseline for this pass:

- Visibility of system status: users need fast, visible feedback and a clear next step.
- Match with the real world: the UI should use product terms such as Suggestion, Memory Card, Loadout, and Artifact instead of internal implementation terms.
- Recognition rather than recall: key workflow state and choices should stay visible in context.
- Aesthetic and minimalist design: panels should prioritize information needed for the next user decision.
- Help and documentation: help should appear in context, with concrete next actions.

Source: https://www.nngroup.com/articles/ten-usability-heuristics/

## Current Friction

- The first screen explains little about the required path from project selection to artifact sync.
- The top-level dashboard has numbers but not enough workflow orientation.
- Suggestion Review can leave the detail pane blank until the user selects an item.
- Memory Card management exposes governance counts but does not frame what needs attention first.
- Some empty states explain the data model more than the user's next action.

## Design Drivers

- Keep a workflow rail visible near the top of the work area.
- Prefer one recommended action over many equal-weight choices.
- Show risk, evidence, and action impact before approval.
- Turn empty states into usable onboarding states.
- Preserve the local-first safety model: review, assign, preview, sync.

