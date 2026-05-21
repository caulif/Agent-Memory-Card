# Sub-SKILL Template

When generating a task-specific sub-SKILL in Phase 5, delegate to the platform's native skill-creator. The sub-SKILL is always installed at **project level** to keep it co-located with the project it serves.

---

## Metadata

- **Name**: `<task-type>-dev` (e.g., `rust-migration-dev`, `microservice-overhaul-dev`)
- **Description**: Should reference the specific task and include trigger keywords
- **Installation**: Project level (e.g., `.cursor/skills/`, `.claude/commands/`, or project-local directory)

---

## Content Outline

The generated sub-SKILL must include these sections **in this order**:

### 1. Cross-Conversation Continuity Protocol

Read `docs/progress/MASTER.md` first, always. Identify the **tracking mode** (`GITHUB_FULL`, `GITHUB_STANDARD`, or `LOCAL_ONLY`) and the active phase/task, then resume from where the last session left off.

- **In GitHub modes**: After reading MASTER.md, query GitHub for the latest task status (Issues may have been closed via merged PRs since the last session). Use `gh issue list -R {repo} --label "spec-driven" --state all` to refresh progress. Update MASTER.md if GitHub state is ahead.
- **In LOCAL_ONLY mode**: MASTER.md and phase files are the sole source of truth.

### 2. S.U.P.E.R Architecture Principles (MUST be inlined)

**CRITICAL**: Do NOT merely reference `super-philosophy.md`. The sub-SKILL must **embed the full S.U.P.E.R principles inline** so that the executing agent has direct access without needing to read an external file. Include the following content verbatim in the generated sub-SKILL:

```markdown
## S.U.P.E.R Architecture — Mandatory Coding Standard

> Write code like building with LEGO — each brick has a single job, a standard interface, a clear direction, runs anywhere, and can be swapped at will.

All code produced in this project MUST conform to these five principles. Violations are treated as bugs.

### S — Single Purpose
- Each module, file, and function solves exactly one problem
- Prefer decomposition; power comes from composition
- **Litmus test**: Can you describe this module's responsibility in a single sentence? If not, split it.

### U — Unidirectional Flow
- Data flows in one direction: input → processing → output
- Dependencies point inward: outer layers depend on inner, inner layers know nothing about outer
- No circular imports, no reverse dependencies
- **Litmus test**: Can the core logic run unit tests with zero external services?

### P — Ports over Implementation
- Define interface contracts (JSON Schema, types, data structures) BEFORE writing implementation
- All cross-module I/O must be serializable
- Swapping a data source, render layer, or notification channel requires zero changes to core logic
- **Practice**: Every module boundary communicates via explicit, schema-defined contracts

### E — Environment-Agnostic
- Configuration via environment variables or config files, never hardcoded
- All dependencies explicitly declared (requirements.txt / package.json / Cargo.toml)
- Processes are stateless; persistence delegated to external storage
- Logs to stdout. Same codebase runs locally, in Docker, on cloud
- **Config precedence**: Environment variables > .env > config file > in-code defaults

### R — Replaceable Parts
- Any layer can be replaced without affecting others
- Replacement cost is THE core metric of architecture quality
- If replacing one component triggers cascading changes, the architecture is broken
- **Validation**: For each module, ask "Can I swap this with a different implementation by only touching this module's directory?"
```

### 3. S.U.P.E.R Code Review Checklist (mandatory after each task)

The sub-SKILL must include this checklist and instruct the agent to run it after completing every task:

```markdown
## S.U.P.E.R Code Review — Run After Every Task

Before marking any task as complete, verify ALL of the following:

| # | Check | Principle | Pass? |
|:--|:------|:----------|:------|
| 1 | Every new module/file has exactly one responsibility | S | |
| 2 | No function does more than one conceptual thing | S | |
| 3 | Data flows input → processing → output, no reverse deps | U | |
| 4 | No circular imports introduced | U | |
| 5 | Cross-module interfaces are schema-defined (types/contracts) | P | |
| 6 | Module I/O is serializable | P | |
| 7 | No hardcoded paths, URLs, keys, or config values | E | |
| 8 | All new dependencies explicitly declared in dependency file | E | |
| 9 | New modules can be replaced without changes to other modules | R | |
| 10 | All tests pass after the change | — | |

**Scoring**: All pass = ✅ proceed. 1-2 fail = fix before marking complete. 3+ fail = stop and refactor.
```

### 4. Target Technology Coding Standards

Conventions for the target language/framework. These should be concrete and actionable, not generic advice. Examples:
- Naming conventions, file organization patterns
- Error handling approach (aligned with S.U.P.E.R — errors flow unidirectionally)
- Testing strategy (unit tests for core logic with zero external deps, per U principle)
- Dependency injection patterns (per P principle — ports/interfaces first)

### 5. Project-Specific Architecture Context

Architecture context derived from `docs/analysis/` and the S.U.P.E.R health assessment from `risk-assessment.md`. Include:
- Which S.U.P.E.R violations from the current codebase must be fixed during this task
- The target architecture's layered model (per U principle)
- Key interface contracts that tasks must conform to (per P principle)

### 6. Progress Update Instructions

How to update progress depends on the tracking mode:

**In GitHub modes (`GITHUB_FULL` / `GITHUB_STANDARD`)**:
- Each task is a GitHub Issue. Execution follows: read Issue → worktree → implement → PR with `closes #N` → comment on Issue.
- Progress is tracked via Issue state (open/closed). When the PR is merged, the Issue is auto-closed.
- Update the "Current Status" and "Issue Mapping" sections in MASTER.md after each task.
- Run the S.U.P.E.R Code Review Checklist before creating the PR.

**In LOCAL_ONLY mode**:
- Update checkbox in phase file
- Update completion count in MASTER.md
- Update "Current Status" section
- Run the S.U.P.E.R Code Review Checklist before marking complete

### 7. Parallel Execution Protocol

Reference `references/parallel-protocol.md` for the full protocol. Summarize key points:
- Check `task-breakdown.md` for parallel lane assignments
- Launch one `task-executor` per lane simultaneously, each in an isolated worktree
- Each executor follows S.U.P.E.R principles independently
- **In GitHub modes**: Each executor creates its own branch and PR linked to its Issue
- Consolidate results: merge PRs sequentially, run full test suite after merge
- After consolidation, reconcile MASTER.md with GitHub Issue states

### 8. Archive Trigger

When all tasks are done (all Issues closed in GitHub modes, or all checkboxes checked in LOCAL_ONLY mode), initiate Phase 7 (Archive). Move local artifacts to `docs/archives/<project>/`. In GitHub modes, Milestones and Issues remain as a permanent record on GitHub.

### 9. Post-Task Telemetry (MANDATORY)

**CRITICAL**: This section implements the adaptive control feedback loop. The agent MUST execute these steps after completing every task and BEFORE marking the task as done (closing the Issue or checking the box). Skipping telemetry breaks the control loop. See `references/adaptive-control.md` for the complete protocol.

```markdown
## Post-Task Telemetry — Execute After Every Task

After completing a task, BEFORE marking it as done:

### Step 1: Record Actual Effort
Compare your actual experience against the estimated effort from task-breakdown.md:

| Level | Criteria |
|:------|:---------|
| S | < 30 minutes, no unexpected issues |
| M | 30 min – 2 hours, minor surprises |
| L | 2 – 4 hours, or significant unexpected complexity |
| XL | > 4 hours, or required re-thinking approach |

### Step 2: Run S.U.P.E.R Quick Check
Run the S.U.P.E.R Code Review Checklist (§ 3 above). Record the score (passes out of 10).

### Step 3: Count Unplanned Dependencies
Count files, tasks, or external resources you needed but that weren't listed in the task's "Affected Files" or "Dependencies" sections.

### Step 4: Write Telemetry

**In GitHub modes**: Post a structured comment on the Issue:

  ## 📊 Execution Telemetry

  | Metric | Estimated | Actual |
  |--------|-----------|--------|
  | Effort | {est} | {actual} |
  | SUPER Score | — | {score}/10 |
  | Unplanned Deps | 0 | {count} |

  **Deltas**: effort {+/-n}, SUPER {+/-n}, deps +{n}
  **Task drift contribution**: {task_drift}
  **Cumulative drift_score**: {new_total} (thresholds: annotate={a}, replan={r}, rescope={s})

**In LOCAL_ONLY mode**: Append a row to the "Task Telemetry Log" table in MASTER.md.

### Step 5: Update Drift Score

**In GitHub modes**: Update the adaptive YAML block in the active Milestone's description with the new drift_score and completed_tasks count.

**In LOCAL_ONLY mode**: Update the "Adaptive Control State" table in MASTER.md.

### Step 6: Check Thresholds

If drift_score >= threshold_rescope: HALT. Output:
  "🛑 Adaptive Control: drift_score={n} exceeded rescope threshold.
   Returning to Phase 2 for scope re-evaluation with user."

If drift_score >= threshold_replan: HALT. Output:
  "🔄 Adaptive Control: drift_score={n} exceeded replan threshold.
   Re-entering Phase 3 to re-decompose remaining tasks."

If drift_score >= threshold_annotate:
  Add warning to next pending task (label + comment on Issue, or note in phase file).
  Continue execution with caution.

Otherwise: proceed to next task normally.
```
