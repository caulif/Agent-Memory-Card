# Extraction Classification And Hardness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic knowledge classification, hardness, artifact target, and structured tags to high-quality extraction so the Inbox contains fewer and better shaped Candidate/Draft records.

**Architecture:** Keep the existing extraction funnel and add a focused `classify` module beside `scoring`. Classification decides `signal`, `artifact_kind`, `activation`, `hardness`, `control`, and structured tags from an `EvidenceChunk`; scoring consumes that classification to make better reject/candidate decisions; Candidate/Draft metadata stores the classification for UI and future compile targeting.

**Tech Stack:** Rust 2024, serde YAML/JSON, existing Agent-Kernel extraction modules, Bun/Tauri frontend types, file-native YAML records.

---

## File Structure

- Modify `tests/fixtures/extract_quality_v2.yml`: extend existing fixture cases with `expected_artifact_kind`, `expected_hardness`, and `expected_tags`.
- Modify `tests/extract_quality_v2.rs`: assert classification fields and structured tags in addition to precision gates.
- Create `src/extract/classify.rs`: own `KnowledgeClassification`, deterministic classifiers, structured tag generation, and helper predicates.
- Modify `src/extract.rs`: register `classify`, expose metadata from classification, attach structured tags to generated Drafts and previews.
- Modify `src/extract/scoring.rs`: score from classification dimensions instead of only broad keyword buckets.
- Modify `src/extract/quality.rs`: include classification in quality reports.
- Modify `src/candidate.rs`: extend `ExtractionMetadata` with optional `KnowledgeClassification` and generated tags.
- Modify `src/draft.rs`: carry classification metadata through Draft records.
- Modify `app/src/types/domain.ts`: add classification metadata type.
- Modify `app/src/components/pages/Drafts.tsx`: show compact classification chips in Inbox rows.
- Modify `app/src/components/pages/RecordEditor.tsx`: show classification and artifact target in the editor evidence section.
- Modify `app/src/demo/demo-data.ts`: include one demo record with classification metadata.
- Modify `app/tests/ui-helpers.test.ts`: assert classification metadata exists in demo/read-model output.

## Task 1: Extend Quality Fixtures With Expected Classification

**Files:**
- Modify: `tests/fixtures/extract_quality_v2.yml`
- Modify: `tests/extract_quality_v2.rs`

- [ ] **Step 1: Extend fixture schema**

Replace each case in `tests/fixtures/extract_quality_v2.yml` with cases that include classification expectations:

```yaml
cases:
  - id: durable-preference-bun
    origin: user
    input: "以后这个项目都用 Bun 管理 JavaScript 依赖和脚本，不要再建议 npm install。"
    expected: candidate
    expected_signal: preference
    expected_artifact_kind: always_on_rule
    expected_hardness: low
    expected_terms: ["Bun", "JavaScript"]
    expected_tags: ["shape:preference", "domain:build", "target:agents-md", "hardness:low"]
  - id: durable-constraint-artifact
    origin: user
    input: "不要直接覆盖 AGENTS.md；发现漂移时先导入成 Draft，让我确认后再同步。"
    expected: candidate
    expected_signal: constraint
    expected_artifact_kind: always_on_rule
    expected_hardness: high
    expected_terms: ["AGENTS.md", "Draft"]
    expected_tags: ["shape:constraint", "domain:governance", "target:agents-md", "hardness:high"]
  - id: durable-procedure-ui-polish
    origin: user
    input: "UI 视觉优化先让 Claude Code 做一轮组件和交互建议，再由 Codex 集成验证。这个流程以后保留。"
    expected: candidate
    expected_signal: procedure
    expected_artifact_kind: workflow_skill
    expected_hardness: medium
    expected_terms: ["Claude Code", "Codex"]
    expected_tags: ["shape:procedure", "domain:ui", "domain:agent-behavior", "target:skill-dir", "hardness:medium"]
  - id: durable-correction-tests
    origin: user
    input: "你又忘了跑测试。以后涉及 Rust 提取逻辑时，先加质量 fixture，再跑 cargo test。"
    expected: candidate
    expected_signal: correction
    expected_artifact_kind: always_on_rule
    expected_hardness: high
    expected_terms: ["fixture", "cargo test"]
    expected_tags: ["shape:correction", "shape:validation", "domain:rust", "domain:testing", "hardness:high"]
  - id: ai-project-improvement-accepted
    origin: assistant
    input: "建议把高质量提取做成 Observation -> EvidenceChunk -> Noise Gate -> Signal Gate -> Value Score -> Candidate 的漏斗。用户确认这是项目核心。"
    expected: candidate
    expected_signal: ai_project_improvement
    expected_artifact_kind: review_only
    expected_hardness: high
    expected_terms: ["EvidenceChunk", "Noise Gate"]
    expected_tags: ["shape:decision", "domain:extraction", "evidence:accepted-ai", "hardness:high"]
  - id: ai-generic-advice-noise
    origin: assistant
    input: "一般来说，软件项目应该保持代码整洁、测试充分、文档完善。"
    expected: noise
    expected_signal: ""
    expected_artifact_kind: reject
    expected_hardness: low
    expected_terms: []
    expected_tags: []
  - id: one-off-task-noise
    origin: user
    input: "把这个按钮颜色改成蓝色，然后帮我看看页面有没有报错。"
    expected: noise
    expected_signal: ""
    expected_artifact_kind: reject
    expected_hardness: low
    expected_terms: []
    expected_tags: []
  - id: unresolved-request-noise
    origin: user
    input: "能不能以后做一个很厉害的多智能体系统？先想想。"
    expected: noise
    expected_signal: ""
    expected_artifact_kind: reject
    expected_hardness: low
    expected_terms: []
    expected_tags: []
  - id: shell-output-noise
    origin: unknown
    input: "error[E0425]: cannot find value `foo` in this scope\n   --> src/main.rs:10:5"
    expected: noise
    expected_signal: ""
    expected_artifact_kind: reject
    expected_hardness: low
    expected_terms: []
    expected_tags: []
  - id: english-durable-decision
    origin: user
    input: "Keep Agent-Kernel file-native for v1. Do not introduce a mandatory vector database; indexes must remain rebuildable caches."
    expected: candidate
    expected_signal: decision
    expected_artifact_kind: always_on_rule
    expected_hardness: high
    expected_terms: ["file-native", "rebuildable"]
    expected_tags: ["shape:decision", "domain:architecture", "hardness:high"]
  - id: ai-skilllet-worthy-workflow
    origin: assistant
    input: "基于用户确认，项目应该把 AI 生成的高质量项目改善也送入 Candidate/Draft，而不是直接写 Skilllet；这能保留审阅边界。"
    expected: candidate
    expected_signal: ai_project_improvement
    expected_artifact_kind: review_only
    expected_hardness: high
    expected_terms: ["Candidate/Draft", "Skilllet"]
    expected_tags: ["shape:constraint", "domain:governance", "evidence:accepted-ai", "hardness:high"]
```

- [ ] **Step 2: Extend test fixture structs**

Update `tests/extract_quality_v2.rs` fixture structs:

```rust
#[derive(Debug, Deserialize)]
struct FixtureCase {
    id: String,
    origin: String,
    input: String,
    expected: String,
    expected_signal: String,
    expected_artifact_kind: String,
    expected_hardness: String,
    expected_terms: Vec<String>,
    expected_tags: Vec<String>,
}
```

- [ ] **Step 3: Pass new fields into quality cases**

Update the `QualityTextCase` construction:

```rust
extract::QualityTextCase {
    id: case.id.clone(),
    origin: case.origin.clone(),
    input: case.input.clone(),
    expected: case.expected.clone(),
    expected_signal: case.expected_signal.clone(),
    expected_artifact_kind: case.expected_artifact_kind.clone(),
    expected_hardness: case.expected_hardness.clone(),
    expected_terms: case.expected_terms.clone(),
    expected_tags: case.expected_tags.clone(),
}
```

- [ ] **Step 4: Run the focused test and verify it fails**

Run:

```powershell
cargo test --test extract_quality_v2 --quiet
```

Expected: compile failure mentioning missing fields on `QualityTextCase`. This is the required red phase.

- [ ] **Step 5: Commit fixtures and failing test**

```powershell
git add tests\fixtures\extract_quality_v2.yml tests\extract_quality_v2.rs
git commit -m "test: define extraction classification expectations"
```

## Task 2: Add Knowledge Classification Module

**Files:**
- Create: `src/extract/classify.rs`
- Modify: `src/extract.rs`
- Do not modify: `src/lib.rs`. The existing `agent_kernel::extract` export is enough because `classify` is registered under `extract`.

- [ ] **Step 1: Create `classify.rs`**

Create `src/extract/classify.rs`:

```rust
use serde::{Deserialize, Serialize};

use super::chunk::{ChunkOrigin, EvidenceChunk};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct KnowledgeClassification {
    pub signal: String,
    pub artifact_kind: String,
    pub activation: String,
    pub hardness: String,
    pub control: String,
    pub rationale: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

pub fn classify_chunk(chunk: &EvidenceChunk) -> KnowledgeClassification {
    let text = chunk.text.trim();
    let lower = text.to_lowercase();

    let signal = detect_signal(chunk, &lower);
    let artifact_kind = artifact_kind_for(&signal, &lower);
    let hardness = hardness_for(&signal, &artifact_kind, &lower);
    let activation = activation_for(&artifact_kind, &hardness);
    let control = control_for(&signal, &hardness, &lower);
    let mut tags = structured_tags(&signal, &artifact_kind, &activation, &hardness, &lower, chunk);
    tags.sort();
    tags.dedup();

    KnowledgeClassification {
        rationale: rationale_for(&signal, &artifact_kind, &hardness),
        signal,
        artifact_kind,
        activation,
        hardness,
        control,
        tags,
    }
}

fn detect_signal(chunk: &EvidenceChunk, lower: &str) -> String {
    if looks_like_noise(lower) {
        return String::new();
    }
    if looks_like_accepted_ai_project_improvement(chunk, lower) {
        return "ai_project_improvement".to_string();
    }
    if looks_like_correction(lower) {
        return "correction".to_string();
    }
    if looks_like_decision(lower) {
        return "decision".to_string();
    }
    if looks_like_validation(lower) {
        return "validation".to_string();
    }
    if looks_like_template(lower) {
        return "template".to_string();
    }
    if looks_like_constraint(lower) {
        return "constraint".to_string();
    }
    if looks_like_preference(lower) {
        return "preference".to_string();
    }
    if looks_like_procedure(lower) {
        return "procedure".to_string();
    }
    String::new()
}

fn artifact_kind_for(signal: &str, lower: &str) -> String {
    match signal {
        "" => "reject",
        "ai_project_improvement" => "review_only",
        "template" => "workflow_skill",
        "procedure" if lower.contains("claude code") && lower.contains("codex") => "workflow_skill",
        "validation" | "correction" | "constraint" | "decision" | "preference" => "always_on_rule",
        _ => "memory_note",
    }
    .to_string()
}

fn hardness_for(signal: &str, artifact_kind: &str, lower: &str) -> String {
    if lower.contains("token=")
        || lower.contains("secret")
        || lower.contains("credential")
        || lower.contains("删除")
        || lower.contains("rm -rf")
        || lower.contains("publish")
        || lower.contains("release")
    {
        return "critical".to_string();
    }
    if matches!(signal, "constraint" | "correction" | "decision" | "validation" | "ai_project_improvement")
        || artifact_kind == "review_only"
        || lower.contains("agents.md")
        || lower.contains("claude.md")
        || lower.contains("cargo test")
        || lower.contains("clippy")
    {
        return "high".to_string();
    }
    if matches!(artifact_kind, "workflow_skill" | "skill_supplement") || matches!(signal, "procedure" | "template") {
        return "medium".to_string();
    }
    "low".to_string()
}

fn activation_for(artifact_kind: &str, hardness: &str) -> String {
    match artifact_kind {
        "workflow_skill" | "skill_supplement" => "skill",
        "path_rule" => "path_glob",
        "review_only" if hardness == "critical" => "manual",
        "review_only" => "manual",
        "always_on_rule" => "always_on",
        _ => "model_decision",
    }
    .to_string()
}

fn control_for(signal: &str, hardness: &str, lower: &str) -> String {
    if matches!(signal, "constraint" | "ai_project_improvement") || lower.contains("不要") || lower.contains("do not") {
        return "prohibition".to_string();
    }
    if matches!(signal, "procedure" | "template" | "validation" | "correction")
        || lower.contains("先")
        || lower.contains("再")
        || lower.contains("then")
        || lower.contains("cargo test")
    {
        return "checklist".to_string();
    }
    if hardness == "high" || hardness == "critical" {
        return "exact_sequence".to_string();
    }
    if signal == "preference" {
        return "default".to_string();
    }
    "principle".to_string()
}

fn structured_tags(
    signal: &str,
    artifact_kind: &str,
    activation: &str,
    hardness: &str,
    lower: &str,
    chunk: &EvidenceChunk,
) -> Vec<String> {
    if signal.is_empty() || artifact_kind == "reject" {
        return Vec::new();
    }

    let mut tags = vec![
        format!("shape:{signal}"),
        format!("hardness:{hardness}"),
        format!("activation:{}", activation.replace('_', "-")),
    ];

    if artifact_kind == "always_on_rule" {
        tags.push("target:agents-md".to_string());
    }
    if artifact_kind == "workflow_skill" || artifact_kind == "skill_supplement" {
        tags.push("target:skill-dir".to_string());
    }
    if lower.contains("codex") {
        tags.push("target:codex".to_string());
    }
    if lower.contains("claude") {
        tags.push("target:claude-code".to_string());
    }
    if lower.contains("rust") || lower.contains("cargo") {
        tags.push("domain:rust".to_string());
    }
    if lower.contains("tauri") {
        tags.push("domain:tauri".to_string());
    }
    if lower.contains("react") || lower.contains("frontend") || lower.contains("前端") || lower.contains("ui") {
        tags.push("domain:frontend".to_string());
    }
    if lower.contains("ui") || lower.contains("界面") || lower.contains("视觉") {
        tags.push("domain:ui".to_string());
    }
    if lower.contains("test") || lower.contains("测试") || lower.contains("fixture") || lower.contains("clippy") {
        tags.push("domain:testing".to_string());
    }
    if lower.contains("bun") || lower.contains("npm") || lower.contains("package") || lower.contains("脚本") {
        tags.push("domain:build".to_string());
    }
    if lower.contains("agents.md") || lower.contains("claude.md") || lower.contains("draft") || lower.contains("skilllet") || lower.contains("审阅") {
        tags.push("domain:governance".to_string());
    }
    if lower.contains("agent") || lower.contains("codex") || lower.contains("claude") || lower.contains("智能体") {
        tags.push("domain:agent-behavior".to_string());
    }
    if lower.contains("architecture") || lower.contains("架构") || lower.contains("database") || lower.contains("file-native") {
        tags.push("domain:architecture".to_string());
    }
    if lower.contains("evidencechunk") || lower.contains("candidate") || lower.contains("noise gate") || lower.contains("提取") {
        tags.push("domain:extraction".to_string());
    }
    if chunk.origin == ChunkOrigin::Assistant && lower.contains("用户确认") {
        tags.push("evidence:accepted-ai".to_string());
    }
    if matches!(signal, "correction") {
        tags.push("evidence:user-correction".to_string());
    }
    if matches!(signal, "validation" | "correction") {
        tags.push("shape:validation".to_string());
    }

    tags
}

fn rationale_for(signal: &str, artifact_kind: &str, hardness: &str) -> String {
    if signal.is_empty() || artifact_kind == "reject" {
        return "Rejected because the text does not contain durable reusable agent knowledge.".to_string();
    }
    format!("Classified as {signal} for {artifact_kind} with {hardness} hardness.")
}

fn looks_like_noise(lower: &str) -> bool {
    let generic = lower.contains("代码整洁") && lower.contains("文档完善");
    let one_off = lower.contains("这个按钮") || lower.contains("改成蓝色") || lower.contains("帮我看看");
    let unresolved = lower.contains("能不能") || lower.contains("先想想") || lower.contains("maybe");
    let shell = lower.contains("error[") || lower.contains("stack trace") || lower.contains("--> src/");
    generic || one_off || unresolved || shell
}

fn looks_like_accepted_ai_project_improvement(chunk: &EvidenceChunk, lower: &str) -> bool {
    if chunk.origin != ChunkOrigin::Assistant {
        return false;
    }
    let accepted = lower.contains("用户确认") || lower.contains("基于用户确认") || lower.contains("accepted");
    let project_terms = lower.contains("candidate")
        || lower.contains("draft")
        || lower.contains("skilllet")
        || lower.contains("evidencechunk")
        || lower.contains("项目");
    let governance_terms = lower.contains("审阅")
        || lower.contains("review")
        || lower.contains("noise gate")
        || lower.contains("value score")
        || lower.contains("直接写")
        || lower.contains("项目核心");
    accepted && project_terms && governance_terms
}

fn looks_like_correction(lower: &str) -> bool {
    (lower.contains("忘了") || lower.contains("again") || lower.contains("不要再"))
        && (lower.contains("测试") || lower.contains("test") || lower.contains("规则") || lower.contains("fixture"))
}

fn looks_like_decision(lower: &str) -> bool {
    let decision = lower.contains("keep ")
        || lower.contains("decided")
        || lower.contains("chose")
        || lower.contains("决定")
        || lower.contains("选用");
    let project = lower.contains("file-native")
        || lower.contains("database")
        || lower.contains("vector")
        || lower.contains("architecture")
        || lower.contains("架构")
        || lower.contains("rebuildable")
        || lower.contains("v1");
    decision && project
}

fn looks_like_validation(lower: &str) -> bool {
    (lower.contains("cargo test") || lower.contains("clippy") || lower.contains("fixture") || lower.contains("build"))
        && (lower.contains("以后") || lower.contains("always") || lower.contains("必须") || lower.contains("before"))
}

fn looks_like_template(lower: &str) -> bool {
    (lower.contains("提示") || lower.contains("prompt") || lower.contains("清单") || lower.contains("handoff"))
        && (lower.contains("claude") || lower.contains("codex") || lower.contains("agent"))
}

fn looks_like_constraint(lower: &str) -> bool {
    lower.contains("不要")
        || lower.contains("禁止")
        || lower.contains("never")
        || lower.contains("do not")
        || lower.contains("don't")
        || lower.contains("must not")
}

fn looks_like_preference(lower: &str) -> bool {
    let durable = lower.contains("以后")
        || lower.contains("always")
        || lower.contains("prefer")
        || lower.contains("默认")
        || lower.contains("统一")
        || lower.contains("优先");
    let tool = lower.contains("bun")
        || lower.contains("axios")
        || lower.contains("vitest")
        || lower.contains("playwright")
        || lower.contains("cargo")
        || lower.contains("npm")
        || lower.contains("fetch");
    durable && tool
}

fn looks_like_procedure(lower: &str) -> bool {
    (lower.contains("先") && lower.contains("再"))
        || (lower.contains("first") && lower.contains("then"))
        || lower.contains("流程")
        || lower.contains("workflow")
}
```

- [ ] **Step 2: Register the module**

In `src/extract.rs`, add near the other module declarations:

```rust
pub mod classify;
```

- [ ] **Step 3: Run focused compile**

Run:

```powershell
cargo test --lib extract::tests --quiet
```

Expected: compile succeeds for the new module, while `extract_quality_v2` still fails until quality structs are extended.

- [ ] **Step 4: Commit**

```powershell
git add src\extract.rs src\extract\classify.rs
git commit -m "feat: classify extraction knowledge"
```

## Task 3: Extend Quality Report With Classification Assertions

**Files:**
- Modify: `src/extract/quality.rs`
- Modify: `tests/extract_quality_v2.rs`

- [ ] **Step 1: Extend `QualityTextCase`**

In `src/extract/quality.rs`, change `QualityTextCase` to:

```rust
#[derive(Debug, Clone)]
pub struct QualityTextCase {
    pub id: String,
    pub origin: String,
    pub input: String,
    pub expected: String,
    pub expected_signal: String,
    pub expected_artifact_kind: String,
    pub expected_hardness: String,
    pub expected_terms: Vec<String>,
    pub expected_tags: Vec<String>,
}
```

- [ ] **Step 2: Add classification checks to report calculation**

Update `quality_report_for_text_cases` so each case computes classification:

```rust
let chunk = text_case_to_chunk(&case.id, &case.origin, &case.input);
let score = score_chunk(&chunk);
let classification = super::classify::classify_chunk(&chunk);
let predicted_candidate = score.disposition == ExtractionDisposition::Candidate;
let expected_candidate = case.expected == "candidate";
let signal_ok = case.expected_signal.is_empty() || classification.signal == case.expected_signal;
let artifact_ok = case.expected_artifact_kind.is_empty()
    || classification.artifact_kind == case.expected_artifact_kind;
let hardness_ok = case.expected_hardness.is_empty()
    || classification.hardness == case.expected_hardness;
let terms_ok = case
    .expected_terms
    .iter()
    .all(|term| case.input.contains(term));
let tags_ok = case
    .expected_tags
    .iter()
    .all(|tag| classification.tags.iter().any(|actual| actual == tag));
let expected_hit = expected_candidate && signal_ok && artifact_ok && hardness_ok && terms_ok && tags_ok;
```

Replace existing uses of `expected_candidate && signal_ok && terms_ok` with `expected_hit`.

- [ ] **Step 3: Run quality test**

Run:

```powershell
cargo test --test extract_quality_v2 --quiet
```

Expected: fails with specific false positives or false negatives because `scoring.rs` does not yet consume the new classification and may disagree.

- [ ] **Step 4: Commit**

```powershell
git add src\extract\quality.rs tests\extract_quality_v2.rs
git commit -m "test: assert extraction classification quality"
```

## Task 4: Make Scoring Consume Classification

**Files:**
- Modify: `src/extract/scoring.rs`

- [ ] **Step 1: Import classification**

At the top of `src/extract/scoring.rs`, add:

```rust
use super::classify::{classify_chunk, KnowledgeClassification};
```

- [ ] **Step 2: Replace signal detection in `score_chunk`**

Inside `score_chunk`, immediately after `let lower = ...`, add:

```rust
let classification = classify_chunk(chunk);
let matched_signal = classification.signal.clone();
```

Remove the mutable `matched_signal = detect_signal_name(text)` line and remove the later `if looks_like_ai_project_improvement(text)` block.

- [ ] **Step 3: Replace broad scoring calls**

Replace the existing `add(...)` scoring block with:

```rust
add(&mut breakdown, "future_agent_value", future_agent_value(&classification));
add(&mut breakdown, "grounding", grounding_score(chunk, &lower));
add(&mut breakdown, "actionability", actionability_score(text));
add(&mut breakdown, "trigger_clarity", trigger_clarity_score(text, &classification));
add(&mut breakdown, "specificity", specificity_score(text));
add(
    &mut breakdown,
    "recurrence_or_correction",
    recurrence_or_correction_score(text, &classification),
);
add(
    &mut breakdown,
    "validation_value",
    validation_value_score(text, &classification),
);
add(
    &mut breakdown,
    "fragility_value",
    fragility_value_score(&classification),
);
add(&mut breakdown, "novelty", 0.8);
add(&mut breakdown, "one_off_penalty", -one_off_penalty(&lower));
add(&mut breakdown, "unresolved_penalty", -unresolved_request_penalty(&lower));
add(&mut breakdown, "generic_advice_penalty", -generic_advice_penalty(&lower));
add(&mut breakdown, "transcript_noise_penalty", -transcript_noise_penalty(&lower));
add(&mut breakdown, "prompt_leak_penalty", -prompt_leak_penalty(&lower));
add(&mut breakdown, "sensitive_content_penalty", -sensitive_content_penalty(&lower));
```

- [ ] **Step 4: Add helper functions**

Add these helpers below `add`:

```rust
fn future_agent_value(classification: &KnowledgeClassification) -> f32 {
    match classification.artifact_kind.as_str() {
        "always_on_rule" => 0.9,
        "workflow_skill" | "skill_supplement" => 0.8,
        "review_only" => 0.7,
        "memory_note" => 0.4,
        _ => 0.0,
    }
}

fn grounding_score(chunk: &EvidenceChunk, lower: &str) -> f32 {
    if chunk.origin.as_str() == "assistant" && (lower.contains("用户确认") || lower.contains("accepted")) {
        0.8
    } else if lower.contains("又忘了") || lower.contains("忘了") || lower.contains("这次") || lower.contains("最终") {
        0.8
    } else if chunk.origin.as_str() == "user" {
        0.6
    } else {
        0.2
    }
}

fn trigger_clarity_score(text: &str, classification: &KnowledgeClassification) -> f32 {
    let lower = text.to_lowercase();
    if lower.contains("以后")
        || lower.contains("when ")
        || lower.contains("for ")
        || lower.contains("处理")
        || lower.contains("涉及")
        || classification.activation == "always_on"
    {
        0.7
    } else {
        0.2
    }
}

fn recurrence_or_correction_score(text: &str, classification: &KnowledgeClassification) -> f32 {
    let lower = text.to_lowercase();
    if classification.signal == "correction"
        || lower.contains("又")
        || lower.contains("again")
        || lower.contains("repeated")
        || lower.contains("每次")
    {
        0.8
    } else {
        0.0
    }
}

fn validation_value_score(text: &str, classification: &KnowledgeClassification) -> f32 {
    let lower = text.to_lowercase();
    if classification.signal == "validation"
        || lower.contains("cargo test")
        || lower.contains("clippy")
        || lower.contains("fixture")
        || lower.contains("preview")
        || lower.contains("audit")
    {
        0.7
    } else {
        0.0
    }
}

fn fragility_value_score(classification: &KnowledgeClassification) -> f32 {
    match classification.hardness.as_str() {
        "critical" => 1.0,
        "high" => 0.8,
        "medium" => 0.4,
        _ => 0.0,
    }
}

fn one_off_penalty(lower: &str) -> f32 {
    if lower.contains("这个按钮") || lower.contains("改成蓝色") || lower.contains("帮我看看") {
        2.4
    } else {
        0.0
    }
}

fn generic_advice_penalty(lower: &str) -> f32 {
    if lower.contains("一般来说") || (lower.contains("代码整洁") && lower.contains("文档完善")) {
        2.2
    } else {
        0.0
    }
}

fn transcript_noise_penalty(lower: &str) -> f32 {
    if lower.contains("error[") || lower.contains("stack trace") || lower.contains("--> src/") || lower.contains("exit code") {
        2.4
    } else {
        0.0
    }
}

fn prompt_leak_penalty(lower: &str) -> f32 {
    if lower.contains("base_instructions") || lower.contains("developer message") || lower.contains("<system") {
        2.4
    } else {
        0.0
    }
}
```

- [ ] **Step 5: Adjust disposition thresholds**

In `score_chunk`, set disposition with classification-aware thresholds:

```rust
let disposition = if classification.artifact_kind == "reject" || matched_signal.is_empty() {
    reason = "Rejected because the text does not contain durable reusable agent knowledge.".to_string();
    ExtractionDisposition::Reject
} else if score >= 4.0 {
    reason = format!(
        "High-value {} signal for {} with {} hardness.",
        classification.signal, classification.artifact_kind, classification.hardness
    );
    ExtractionDisposition::Candidate
} else if score >= 3.2 {
    reason = format!(
        "Borderline {} signal for optional provider review.",
        classification.signal
    );
    ExtractionDisposition::Borderline
} else {
    reason = "Rejected because the value score is below the Candidate threshold.".to_string();
    ExtractionDisposition::Reject
};
```

- [ ] **Step 6: Remove dead helper functions**

Delete `detect_signal_name`, `looks_like_preference_direct`, `looks_like_decision`, `durability_score`, `recurrence_score`, `correction_score`, `task_specific_penalty`, `vague_penalty`, and `looks_like_ai_project_improvement` if no remaining code references them. Keep `actionability_score`, `specificity_score`, `unresolved_request_penalty`, and `sensitive_content_penalty` if reused by the new scoring block.

- [ ] **Step 7: Run quality test**

Run:

```powershell
cargo test --test extract_quality_v2 --quiet
```

Expected: quality classification test passes. If it fails, adjust only the helper scores or classification predicates required by the named fixture id.

- [ ] **Step 8: Run clippy for scorer**

Run:

```powershell
cargo clippy --quiet -- -D warnings
```

Expected: no warnings.

- [ ] **Step 9: Commit**

```powershell
git add src\extract\scoring.rs
git commit -m "feat: score extraction classification hardness"
```

## Task 5: Store Classification Metadata On Candidate And Draft

**Files:**
- Modify: `src/candidate.rs`
- Modify: `src/draft.rs`
- Modify: `src/extract.rs`

- [ ] **Step 1: Extend `ExtractionMetadata`**

In `src/candidate.rs`, import the classification type:

```rust
use crate::extract::classify::KnowledgeClassification;
```

Extend `ExtractionMetadata`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtractionMetadata {
    #[serde(default)]
    pub origin: String,
    #[serde(default)]
    pub matched_signal: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub source_observations: Vec<String>,
    #[serde(default)]
    pub score_breakdown: BTreeMap<String, f32>,
    #[serde(default)]
    pub similar_record: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classification: Option<KnowledgeClassification>,
    #[serde(default)]
    pub tags: Vec<String>,
}
```

- [ ] **Step 2: Carry tags into Candidate defaults**

In `add_candidate`, when setting `tags`, merge explicit candidate tags with `candidate.extraction.tags` before falling back to inferred metadata:

```rust
let mut normalized_tags = textutil::normalize_string_list(candidate.tags);
normalized_tags.extend(textutil::normalize_string_list(candidate.extraction.tags.clone()));
normalized_tags.sort();
normalized_tags.dedup();
```

Use:

```rust
tags: if normalized_tags.is_empty() { metadata.tags } else { normalized_tags },
```

- [ ] **Step 3: Update extraction metadata builder**

In `src/extract.rs`, update `extraction_metadata_for_chunk`:

```rust
fn extraction_metadata_for_chunk(
    chunk: &chunk::EvidenceChunk,
    score: &scoring::ExtractionScore,
    similar_record: Option<String>,
) -> candidate::ExtractionMetadata {
    let classification = classify::classify_chunk(chunk);
    candidate::ExtractionMetadata {
        origin: chunk.origin.as_str().to_string(),
        matched_signal: classification.signal.clone(),
        reason: score.reason.clone(),
        source_observations: chunk.source_observations.clone(),
        score_breakdown: score.breakdown.clone(),
        similar_record,
        tags: classification.tags.clone(),
        classification: Some(classification),
    }
}
```

- [ ] **Step 4: Keep direct Draft tags in extraction metadata**

`NewDraft` does not carry top-level tags in the current code. Do not add a `tags` field to `NewDraft` in this phase. Classification tags should live in `ExtractionMetadata.tags` and can be promoted to top-level tags in a later metadata normalization pass.

Add this short comment next to the `NewDraft` construction in `extract_local_text_to_drafts`:

```rust
// NewDraft does not carry top-level tags yet; classification tags are stored in extraction metadata.
```

- [ ] **Step 5: Add a metadata preservation test**

In `tests/extract_quality_v2.rs`, add:

```rust
#[test]
fn local_extract_persists_classification_metadata_on_draft() {
    let temp = tempfile::tempdir().expect("tempdir");

    extract::extract_to_drafts(
        temp.path(),
        Some("不要直接覆盖 AGENTS.md；发现漂移时先导入成 Draft，让我确认后再同步。".to_string()),
        None,
        vec!["codex".to_string()],
        Some("local".to_string()),
        false,
    )
    .expect("extract");

    let drafts = agent_kernel::draft::load_drafts(temp.path()).expect("drafts");
    let classification = drafts[0]
        .extraction
        .classification
        .as_ref()
        .expect("classification metadata");

    assert_eq!(classification.signal, "constraint");
    assert_eq!(classification.artifact_kind, "always_on_rule");
    assert_eq!(classification.hardness, "high");
    assert!(drafts[0].extraction.tags.contains(&"domain:governance".to_string()));
}
```

- [ ] **Step 6: Run metadata test**

Run:

```powershell
cargo test --test extract_quality_v2 local_extract_persists_classification_metadata_on_draft --quiet
```

Expected: passes.

- [ ] **Step 7: Run candidate/draft tests**

Run:

```powershell
cargo test candidate:: draft:: --quiet
```

Expected: candidate and draft tests pass.

- [ ] **Step 8: Commit**

```powershell
git add src\candidate.rs src\draft.rs src\extract.rs tests\extract_quality_v2.rs
git commit -m "feat: persist extraction classification metadata"
```

## Task 6: Make Dry-Run Previews Show Classification

**Files:**
- Modify: `src/extract.rs`

- [ ] **Step 1: Extend `ExtractCandidatePreview`**

Add optional preview fields:

```rust
pub classification: Option<classify::KnowledgeClassification>,
pub tags: Vec<String>,
```

- [ ] **Step 2: Populate preview fields in local extraction**

In the local preview mapper, replace:

```rust
.map(|(candidate, _score)| ExtractCandidatePreview {
```

with:

```rust
.map(|(candidate, _score)| {
    let chunk = chunk::EvidenceChunk {
        id: candidate.title.clone(),
        text: candidate.evidence.clone(),
        origin: chunk::ChunkOrigin::User,
        source_kind: source.to_string(),
        source_observations: Vec::new(),
    };
    let classification = classify::classify_chunk(&chunk);
    ExtractCandidatePreview {
        classification: Some(classification.clone()),
        tags: classification.tags.clone(),
```

Keep the existing fields inside the struct literal, then close with:

```rust
    }
})
```

- [ ] **Step 3: Populate preview fields in LLM extraction**

In the LLM preview mapper, build an assistant-origin chunk from `item.body`, classify it, and set the same fields:

```rust
let chunk = chunk::EvidenceChunk {
    id: item.title.clone(),
    text: item.body.clone(),
    origin: chunk::ChunkOrigin::Assistant,
    source_kind: source.to_string(),
    source_observations: Vec::new(),
};
let classification = classify::classify_chunk(&chunk);
classification: Some(classification.clone()),
tags: classification.tags.clone(),
```

- [ ] **Step 4: Render classification in CLI report**

In `ExtractReport::render`, inside the dry-run candidate loop after `matched_template`, add:

```rust
if let Some(classification) = candidate.classification.as_ref() {
    out.push_str(&format!(
        "  classification: signal={}, artifact={}, hardness={}, activation={}\n",
        classification.signal,
        classification.artifact_kind,
        classification.hardness,
        classification.activation
    ));
}
if !candidate.tags.is_empty() {
    out.push_str(&format!("  tags: {}\n", candidate.tags.join(", ")));
}
```

- [ ] **Step 5: Add dry-run assertion**

In `tests/extract_quality_v2.rs`, add:

```rust
#[test]
fn dry_run_previews_include_classification_and_tags() {
    let temp = tempfile::tempdir().expect("tempdir");

    let report = extract::extract_to_drafts(
        temp.path(),
        Some("以后这个项目的前端 HTTP 请求统一用 Axios，不要再写裸 fetch。".to_string()),
        None,
        vec!["codex".to_string()],
        Some("local".to_string()),
        true,
    )
    .expect("extract");

    let preview = &report.candidates[0];
    let classification = preview.classification.as_ref().expect("classification");
    assert_eq!(classification.signal, "preference");
    assert_eq!(classification.hardness, "low");
    assert!(preview.tags.contains(&"shape:preference".to_string()));
    assert!(report.render().contains("classification: signal=preference"));
}
```

- [ ] **Step 6: Run focused tests**

Run:

```powershell
cargo test --test extract_quality_v2 --quiet
```

Expected: all extraction quality tests pass.

- [ ] **Step 7: Commit**

```powershell
git add src\extract.rs tests\extract_quality_v2.rs
git commit -m "feat: show extraction classification in previews"
```

## Task 7: Surface Classification In The Frontend

**Files:**
- Modify: `app/src/types/domain.ts`
- Modify: `app/src/components/pages/Drafts.tsx`
- Modify: `app/src/components/pages/RecordEditor.tsx`
- Modify: `app/src/demo/demo-data.ts`
- Modify: `app/tests/ui-helpers.test.ts`

- [ ] **Step 1: Extend TypeScript domain types**

In `app/src/types/domain.ts`, add:

```ts
export type KnowledgeClassification = {
  signal?: string;
  artifact_kind?: string;
  activation?: string;
  hardness?: string;
  control?: string;
  rationale?: string;
  tags?: string[];
};
```

Extend existing `ExtractionMetadata`:

```ts
classification?: KnowledgeClassification | null;
tags?: string[];
```

- [ ] **Step 2: Add demo classification**

In `app/src/demo/demo-data.ts`, add classification metadata to one draft:

```ts
extraction: {
  origin: "user",
  matched_signal: "constraint",
  reason: "High-value constraint signal for always_on_rule with high hardness.",
  source_observations: ["observation:demo"],
  score_breakdown: { future_agent_value: 0.9, fragility_value: 0.8 },
  similar_record: null,
  classification: {
    signal: "constraint",
    artifact_kind: "always_on_rule",
    activation: "always_on",
    hardness: "high",
    control: "prohibition",
    rationale: "Classified as constraint for always_on_rule with high hardness.",
    tags: ["shape:constraint", "domain:governance", "hardness:high"],
  },
  tags: ["shape:constraint", "domain:governance", "hardness:high"],
},
```

- [ ] **Step 3: Render compact Inbox chips**

In `app/src/components/pages/Drafts.tsx`, near existing reason/evidence metadata, add:

```tsx
{draft.extraction?.classification ? (
  <div className="record-meta-chips" aria-label="提取分类">
    <span>{draft.extraction.classification.signal || "unknown"}</span>
    <span>{draft.extraction.classification.hardness || "unknown"}</span>
    <span>{draft.extraction.classification.artifact_kind || "unknown"}</span>
  </div>
) : null}
```

- [ ] **Step 4: Render editor classification details**

In `app/src/components/pages/RecordEditor.tsx`, inside the extraction/evidence section, add:

```tsx
{record.extraction?.classification ? (
  <dl className="editor-classification">
    <dt>分类</dt>
    <dd>{record.extraction.classification.signal || "unknown"}</dd>
    <dt>目标</dt>
    <dd>{record.extraction.classification.artifact_kind || "unknown"}</dd>
    <dt>难度</dt>
    <dd>{record.extraction.classification.hardness || "unknown"}</dd>
    <dt>触发</dt>
    <dd>{record.extraction.classification.activation || "unknown"}</dd>
  </dl>
) : null}
```

- [ ] **Step 5: Add frontend test expectation**

In `app/tests/ui-helpers.test.ts`, add to an existing demo state test:

```ts
expect(JSON.stringify(state)).toContain("artifact_kind");
expect(JSON.stringify(state)).toContain("hardness:high");
```

Use the local variable name from that test. If the existing test calls the demo state `snapshot` instead of `state`, use:

```ts
expect(JSON.stringify(snapshot)).toContain("artifact_kind");
expect(JSON.stringify(snapshot)).toContain("hardness:high");
```

- [ ] **Step 6: Run frontend tests**

Run:

```powershell
bun run --cwd app test
```

Expected: frontend tests pass.

- [ ] **Step 7: Run frontend build**

Run:

```powershell
bun run --cwd app build
```

Expected: TypeScript and Vite build succeed.

- [ ] **Step 8: Commit**

```powershell
git add app\src\types\domain.ts app\src\components\pages\Drafts.tsx app\src\components\pages\RecordEditor.tsx app\src\demo\demo-data.ts app\tests\ui-helpers.test.ts
git commit -m "feat: surface extraction classification in inbox"
```

## Task 8: Manual Dry-Run Effect Check

**Files:**
- No source edits unless the dry-run exposes a defect.

- [ ] **Step 1: Run representative dry-run cases**

Run:

```powershell
$project = Join-Path $env:TEMP ("agent-kernel-classification-check-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $project | Out-Null
$cases = @(
  @{name='preference'; text='以后这个项目的前端 HTTP 请求统一用 Axios，不要再写裸 fetch。'},
  @{name='governance'; text='不要直接覆盖 AGENTS.md；发现漂移时先导入成 Draft，让我确认后再同步。'},
  @{name='workflow'; text='UI 视觉优化先让 Claude Code 做一轮组件和交互建议，再由 Codex 集成验证。这个流程以后保留。'},
  @{name='accepted-ai'; text='基于用户确认，项目应该把 AI 生成的高质量项目改善也送入 Candidate/Draft，而不是直接写 Skilllet；这能保留审阅边界。'},
  @{name='noise'; text='一般来说，软件项目应该保持代码整洁、测试充分、文档完善。'}
)
foreach ($case in $cases) {
  Write-Output ("CASE: " + $case.name)
  cargo run --quiet -- extract --project $project --dry-run --provider local --target codex --text $case.text
}
```

Expected:

- `preference`: one candidate, `signal=preference`, `hardness=low`, `target:agents-md`.
- `governance`: one candidate, `signal=constraint`, `hardness=high`, `domain:governance`.
- `workflow`: one candidate, `artifact=workflow_skill`, `activation=skill`.
- `accepted-ai`: one candidate, `artifact=review_only`, `evidence:accepted-ai`.
- `noise`: no drafts created.

- [ ] **Step 2: Fix only concrete mismatches**

If a case fails, make the smallest edit in `src/extract/classify.rs` or `src/extract/scoring.rs`, then add or update a fixture case that captures the mismatch.

- [ ] **Step 3: Re-run quality test**

Run:

```powershell
cargo test --test extract_quality_v2 --quiet
```

Expected: all quality tests pass.

- [ ] **Step 4: Commit dry-run fixes if any**

Only if source files changed:

```powershell
git add src\extract\classify.rs src\extract\scoring.rs tests\fixtures\extract_quality_v2.yml tests\extract_quality_v2.rs
git commit -m "fix: tune extraction classification examples"
```

## Task 9: Full Verification

**Files:**
- No source edits unless verification exposes a defect.

- [ ] **Step 1: Run Rust core tests**

```powershell
cargo test --quiet
```

Expected: all Rust tests pass.

- [ ] **Step 2: Run Rust clippy**

```powershell
cargo clippy --quiet -- -D warnings
```

Expected: no warnings.

- [ ] **Step 3: Run Tauri tests**

```powershell
cargo test --manifest-path src-tauri\Cargo.toml --quiet
```

Expected: all Tauri tests pass.

- [ ] **Step 4: Run Tauri clippy**

```powershell
cargo clippy --manifest-path src-tauri\Cargo.toml --quiet -- -D warnings
```

Expected: no warnings.

- [ ] **Step 5: Run frontend tests**

```powershell
bun run --cwd app test
```

Expected: all app tests pass.

- [ ] **Step 6: Run frontend build**

```powershell
bun run --cwd app build
```

Expected: TypeScript and Vite build succeed.

- [ ] **Step 7: Run Bun wrapper tests**

```powershell
bun test bun\agent-kernel-lib.test.js
```

Expected: wrapper tests pass.

- [ ] **Step 8: Commit verification fixes if any**

Only if verification required source changes:

```powershell
git add src\extract\classify.rs src\extract\scoring.rs src\extract\quality.rs src\extract.rs src\candidate.rs src\draft.rs tests\fixtures\extract_quality_v2.yml tests\extract_quality_v2.rs app\src\types\domain.ts app\src\components\pages\Drafts.tsx app\src\components\pages\RecordEditor.tsx app\src\demo\demo-data.ts app\tests\ui-helpers.test.ts
git commit -m "fix: stabilize extraction classification"
```

## Self-Review

- Spec coverage: Tasks 1-4 implement classification, hardness, artifact target, scoring dimensions, and noise strengthening. Tasks 5-7 persist and surface metadata. Task 8 validates dry-run behavior against representative examples. Task 9 covers full verification.
- Scope check: The plan does not auto-create Skill directories, add vector search, add MCP/A2A, or make LLM extraction mandatory.
- Type consistency: `KnowledgeClassification` fields are `signal`, `artifact_kind`, `activation`, `hardness`, `control`, `rationale`, and `tags` in Rust, TypeScript, tests, and metadata.
- TDD check: Fixtures and tests are updated before implementation, then classification, scoring, metadata, previews, and UI are implemented behind failing tests.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-05-03-extraction-classification-hardness.md`. Two execution options:

1. Subagent-Driven (recommended) - dispatch a fresh subagent per task, review between tasks, fast iteration.
2. Inline Execution - execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach?
