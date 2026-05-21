# Skills And Memory Linkage Roadmap

## Target Workflow

```text
Select project -> scan/import Skills -> open Skills Library
-> inspect Skill -> attach relevant Memory Cards
-> preview generated supplement -> sync agent artifacts
```

## Phase 1: Skill Library Read Model

Acceptance criteria:

- Desktop has a `get_project_skill_library` read model.
- Read model includes indexed Skills, mirror targets, supplement links, recommended Memory Cards, and warnings.
- Rust tests cover linked and recommended Memory Cards.

## Phase 2: Product UI

Acceptance criteria:

- Main navigation includes a Skills page.
- Skills page shows source, mirror status, warnings, linked Memory Cards, and recommendations.
- User can attach a Memory Card to a Skill from the page through existing guarded command flow.
- UI can route users to sync preview after making linkage changes.

## Phase 3: Sync Preview Clarity

Acceptance criteria:

- Build preview surfaces generated Skill supplement artifacts clearly.
- UI copy explains that Memory Cards supplement mirrored Skills through generated `AGENT_KERNEL_MEMORY_CARDS.md`.
- No direct `SKILL.md` mutation is performed.

## Design Constraints

- Rust core owns the read model.
- Tauri command remains a thin IPC boundary.
- React renders typed read models and emits named mutations.
- All linkage is explicit and reversible through project config.
- Do not embed Memory Card text directly into source `SKILL.md`.
