# Agent Skills Kernel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the first reusable Kernel layer so UI, CLI, future MCP tools, and AI agents can evaluate Agent-Kernel actions through one policy and rule model.

**Architecture:** Keep existing business modules intact and add `src/kernel/` as a thin command/planning layer. The first version evaluates command risk and automation policy; later versions can route actual execution through the same layer.

**Tech Stack:** Rust library modules, serde-serializable command/decision structs, Tauri command exposure, existing Bun/Tauri frontend compatibility.

---

### Task 1: Kernel Policy And Rules

**Files:**
- Create: `src/kernel/mod.rs`
- Create: `src/kernel/policy.rs`
- Create: `src/kernel/rules.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write failing tests**

Add tests in `src/kernel/policy.rs` and `src/kernel/rules.rs` for:

```rust
#[test]
fn manual_policy_routes_mutating_commands_to_review() {
    let policy = KernelPolicy::manual();
    let command = KernelCommand::ApproveDraft { id: "project:prefer-bun".to_string() };
    let decision = plan_command(&command, &policy);
    assert_eq!(decision.disposition, KernelDisposition::ReviewRequired);
    assert!(decision.requires_human_review);
}

#[test]
fn rules_filter_low_value_task_chatter() {
    let assessment = assess_text("继续优化一下，然后再修一下这个按钮");
    assert_eq!(assessment.value, KernelValue::Noise);
    assert!(assessment.tags.contains(&"noise".to_string()));
}
```

- [ ] **Step 2: Verify tests fail**

Run: `cargo test kernel`

Expected: compile failure because `kernel` module and types do not exist.

- [ ] **Step 3: Implement minimal types**

Create `AutomationMode`, `KernelPolicy`, `KernelCommand`, `KernelRisk`, `KernelDisposition`, `KernelDecision`, `RuleAssessment`, `KernelValue`, and deterministic helpers.

- [ ] **Step 4: Verify tests pass**

Run: `cargo test kernel`

Expected: all kernel tests pass.

### Task 2: Tauri Planning Command

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing test**

Add a unit test that calls the Tauri-side helper for planning an `approve-draft` command under `Manual` policy and expects `ReviewRequired`.

- [ ] **Step 2: Verify failure**

Run: `cargo test --manifest-path src-tauri\Cargo.toml kernel`

Expected: compile failure because the helper does not exist.

- [ ] **Step 3: Add Tauri command**

Expose `plan_kernel_command(input)` and parse input into the shared `agent_kernel::kernel` structs.

- [ ] **Step 4: Verify**

Run: `cargo test --manifest-path src-tauri\Cargo.toml kernel`

Expected: test passes.

### Task 3: Regression Verification

**Files:**
- Existing test suites only.

- [ ] **Step 1: Format**

Run: `cargo fmt --check`

- [ ] **Step 2: Rust tests**

Run: `cargo test`

- [ ] **Step 3: Tauri tests**

Run: `cargo test --manifest-path src-tauri\Cargo.toml`

- [ ] **Step 4: Lints and frontend**

Run: `cargo clippy -- -D warnings`, `cargo clippy --manifest-path src-tauri\Cargo.toml -- -D warnings`, and `bun run test`.

