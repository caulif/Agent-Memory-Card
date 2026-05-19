# Archive Templates

Templates for Phase 7 (Archive). Output to `docs/archives/`.

---

## docs/archives/README.md (Archive Index)

```markdown
# Project Archives

This directory contains archived artifacts from completed Spec-Driven Develop workflows. Each subdirectory represents one development project, preserving its full analysis, plan, progress history, and task-specific skill for future reference.

| Project | Description | Period | Mode | Progress |
|:--------|:------------|:-------|:-----|:---------|
| [<project-name>](./<project-name>/progress/MASTER.md) | One-line description | YYYY-MM-DD — YYYY-MM-DD | GITHUB_FULL | Complete |
```

When updating this file, append new rows to the table. Do not remove existing entries. In GitHub modes, include the GitHub Project URL in the Description field for easy reference.

---

## Archive Directory Structure

```
docs/archives/<project-name>/
├── analysis/
│   ├── project-overview.md
│   ├── module-inventory.md
│   └── risk-assessment.md
├── plan/
│   ├── task-breakdown.md
│   ├── dependency-graph.md
│   └── milestones.md
├── progress/
│   ├── MASTER.md
│   ├── phase-1-<short-name>.md
│   └── ...
└── skill/
    └── SKILL.md
```
