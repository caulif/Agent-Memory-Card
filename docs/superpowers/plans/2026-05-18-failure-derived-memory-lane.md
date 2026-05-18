# Failure-Derived Memory Lane Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a deterministic extraction lane that turns extraction failures, user corrections, and quality misses into conservative future-facing Memory Card candidates.

**Architecture:** Add a focused `failure_flow` observation module that produces compact synthetic extraction material from `ObservationRecord[]`. Add `failure_flow_candidates` in the candidate factory, route it through existing extraction and quality gates, and keep real histories as local-only validation inputs.

**Tech Stack:** Rust core, existing `extract` and `observation` modules, existing unit tests plus local replay dry-runs.

---

### Task 1: Failure Flow Summary

**Files:**
- Create: `src/observation/failure_flow.rs`
- Modify: `src/observation.rs`

- [ ] **Step 1: Write failing tests**

Add tests in `src/observation/failure_flow.rs` for:

```rust
#[test]
fn failure_flow_detects_pipeline_break_and_quality_correction() {
    let observations = vec![
        observation("a", "2026-01-01T00:00:00Z", "LLM induction 阶段返回的 JSON 被截断/格式不完整，解析失败了，所以没有进入最终 crystallize 卡片阶段。"),
        observation("b", "2026-01-01T01:00:00Z", "不能只看指标，要自己看最终卡片质量。"),
    ];

    let summary = summarize_failure_flow(&observations);
    let material = summary.to_extraction_material();

    assert!(material.contains("FailureFlowSummary:"), "{material}");
    assert!(material.contains("signal:pipeline_break"), "{material}");
    assert!(material.contains("signal:final_quality_correction"), "{material}");
}
```

- [ ] **Step 2: Run RED**

Run: `cargo test --lib observation::failure_flow::tests::failure_flow_detects_pipeline_break_and_quality_correction`

Expected: fail because the module/function does not exist.

- [ ] **Step 3: Implement summary types**

Create `FailureSignalKind`, `FailureSignal`, `FailureFlowSummary`, `summarize_failure_flow`, and `to_extraction_material`. Detect only compact, user-facing signals:

- pipeline break: truncated JSON, induction parse failure, no crystallize
- final quality correction: cannot only look at metrics, must inspect final cards
- false-positive noise: code analysis or extraction taxonomy was captured
- privacy boundary: real history local-only, not Git, not Golden Set
- local/global boundary: distinguish project and global memory

- [ ] **Step 4: Run GREEN**

Run: `cargo test --lib observation::failure_flow`

Expected: pass.

### Task 2: Failure-Derived Candidate Factory

**Files:**
- Modify: `src/extract/candidate_factory.rs`
- Modify: `src/extract.rs`

- [ ] **Step 1: Write failing tests**

Add tests in `src/extract.rs`:

```rust
#[test]
fn high_value_extraction_surfaces_failure_flow_pipeline_break_rule() {
    let temp = tempfile::tempdir().expect("tempdir");
    let input = "FailureFlowSummary:\n- signal:pipeline_break obs:a text:LLM induction JSON 被截断，解析失败，所以没有进入最终 crystallize 卡片阶段。\n- signal:final_quality_correction obs:b text:不能只看指标，要自己看最终卡片质量。\n";

    let report = extract_high_value_text_to_drafts(temp.path(), input, vec!["codex".to_string()], "failure flow prefilter", Some("local".to_string()), true, 8).expect("extract");

    assert!(report.candidates.iter().any(|candidate| candidate.matched_template.as_deref() == Some("failure-flow:pipeline-break")), "{report:#?}");
    assert!(report.candidates.iter().any(|candidate| candidate.body.contains("先定位链路断点")), "{report:#?}");
}
```

- [ ] **Step 2: Run RED**

Run: `cargo test --lib extract::tests::high_value_extraction_surfaces_failure_flow_pipeline_break_rule`

Expected: fail because no failure-flow candidate exists.

- [ ] **Step 3: Implement candidate templates**

Add `failure_flow_candidates(input: &str) -> Vec<Candidate>` and route it in both extraction candidate paths. Use templates:

- `failure-flow:pipeline-break`
- `failure-flow:final-quality-review`
- `failure-flow:false-positive-abstraction`
- `failure-flow:local-only-regression`
- `failure-flow:scope-boundary`

Candidates must be future-facing, scoped conservatively, and use confidence `0.82..0.88`.

- [ ] **Step 4: Run GREEN**

Run: `cargo test --lib extract::tests::high_value_extraction_surfaces_failure_flow_pipeline_break_rule`

Expected: pass.

### Task 3: Quality Gate and Chunk Integration

**Files:**
- Modify: `src/extract/quality_gate.rs`
- Modify: `src/observation/chunked.rs`

- [ ] **Step 1: Write failing tests**

Add tests that:

- reject unsupported `failure-flow:*` material with no direct correction/failure support
- surface failure-derived candidates from `extract_local_chunks_to_report`

- [ ] **Step 2: Run RED**

Run focused tests:

```powershell
cargo test --lib quality_gate::tests::rejects_unsupported_failure_flow_candidate
cargo test --lib observation::chunked::tests::local_chunk_extraction_surfaces_failure_flow_candidates
```

Expected: fail until gate and chunk integration exist.

- [ ] **Step 3: Implement gate and merge**

Add `has_failure_flow_support` in `quality_gate.rs`. In `chunked.rs`, add `merge_failure_flow_candidates` after `merge_flow_candidates` and before methodology snippet scanning.

- [ ] **Step 4: Run GREEN**

Run:

```powershell
cargo test --lib quality_gate::tests::rejects_unsupported_failure_flow_candidate
cargo test --lib observation::chunked::tests::local_chunk_extraction_surfaces_failure_flow_candidates
```

Expected: pass.

### Task 4: Regression and Real-History Validation

**Files:**
- Modify: `docs/global-flow-extraction-validation.md`

- [ ] **Step 1: Run focused tests**

Run:

```powershell
cargo fmt --check
cargo test --lib observation::failure_flow extract::tests::high_value_extraction_surfaces_failure_flow_pipeline_break_rule
cargo test --lib
cargo test --test extract_quality_v2
```

- [ ] **Step 2: Run eval and local-only replay**

Run:

```powershell
cargo run --quiet -- eval --golden-set --project $PWD
cargo run --quiet -- observe replay --project $PWD --home $env:USERPROFILE --target codex --target claude-code --engine local --dry-run
cargo run --quiet -- observe replay --project "C:\QianCi" --home $env:USERPROFILE --target codex --target claude-code --engine local --dry-run
```

- [ ] **Step 3: Manually inspect output**

Confirm failure-derived candidates are specific, future-facing, not duplicated, and not derived from private text committed to the repo.
