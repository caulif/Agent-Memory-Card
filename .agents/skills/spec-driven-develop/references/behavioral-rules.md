# Behavioral Rules

These rules apply to every agent and every phase in the Spec-Driven Develop workflow. They are non-negotiable.

---

1. **Never skip phases**. Even if you think a phase is unnecessary, at minimum create a lightweight version of its outputs.

2. **Always confirm with the user** before proceeding to the next phase. Each phase boundary is a checkpoint.

3. **Document everything**. If you make a decision, record it in the relevant progress file's "Notes" section.

4. **Progress updates are mandatory**. After completing any task, update progress immediately. In GitHub modes: the PR with `closes #N` handles Issue closure; update MASTER.md's "Current Status" and "Issue Mapping" sections. In LOCAL_ONLY mode: update the checkbox in the phase file AND the completion count in MASTER.md.

5. **New conversation = read MASTER.md first**. This is non-negotiable. The master file is your memory across conversations. In GitHub modes, also query GitHub for the latest Issue states — PRs may have been merged since the last session.

6. **Respect the user's time**. Keep summaries concise. Use bullet points and tables, not walls of text.

7. **Archiving is not optional**. When all tasks are done, always enter Phase 7. Archive all artifacts to `docs/archives/` for traceability — don't leave them scattered in working directories or delete them.

8. **Dual-write progress updates**. When completing a task, update progress in two places for redundancy. The specific targets depend on the tracking mode:
   - **GitHub modes**: GitHub Issue (via PR with `closes #N`) + MASTER.md local index. The native platform task tool is an optional third layer.
   - **LOCAL_ONLY mode**: Platform's native task tool (mark as completed) + Markdown progress files (check the box, update counts).
   In all modes, the principle is the same: no single point of failure for progress state.

9. **Use AskUserQuestionTool for all user interactions**. Whenever you need to ask the user a question, request clarification, or get confirmation (including phase boundary checkpoints), you MUST use the platform's built-in `AskUserQuestionTool`. Do not rely on plain text output to ask questions — the tool ensures the user sees and responds to your question directly.

10. **Post-task telemetry is mandatory**. After completing every task, record actual effort, S.U.P.E.R score, and unplanned dependency count BEFORE marking the task as done. This is as non-negotiable as progress updates (rule 4). See `references/adaptive-control.md` § 1 for what to collect and § 4 for where to store it.

11. **Drift threshold triggers are automatic**. When `drift_score` exceeds a threshold, the agent MUST halt and execute the corresponding response action (annotate / replan / rescope) without waiting for user instruction. The thresholds are computed per-phase as percentages of total task count (20% / 40% / 60%). See `references/adaptive-control.md` § 3 for the response protocol.

12. **Adaptive state is persistent**. Always read and write `drift_score` via the defined storage: Milestone description YAML block in GitHub modes, or the "Adaptive Control State" section in MASTER.md for LOCAL_ONLY. Never store adaptive state only in conversation memory — it must survive across sessions.
