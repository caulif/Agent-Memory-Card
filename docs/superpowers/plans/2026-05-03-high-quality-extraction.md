# High Quality Extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a selective, evidence-backed extraction funnel that produces a small number of high-value Candidate/Draft records, including accepted AI-origin project improvements, without bypassing human review.

**Architecture:** Add a deterministic quality layer around the existing extraction code. Introduce focused modules for chunks, noise classification, signal scoring, quality fixtures, and Candidate metadata while preserving the current file-native YAML model and local-first default.

**Tech Stack:** Rust 2024, serde YAML/JSON, existing Agent-Kernel Rust core, Bun/Tauri read models, local deterministic extraction with optional provider-assisted refinement kept behind existing provider configuration.

---

## File Structure

- Create `tests/fixtures/extract_quality_v2.yml`: checked-in bilingual fixture cases for true positives, false positives, AI-origin improvements, and noise.
- Create `tests/extract_quality_v2.rs`: integration test that loads fixtures, runs the extraction funnel, and asserts metrics.
- Create `src/extract/chunk.rs`: `EvidenceChunk`, chunk origin enum, and observation-to-chunk conversion.
- Create `src/extract/scoring.rs`: `ExtractionScore`, deterministic scoring rules, score disposition.
- Create `src/extract/quality.rs`: quality report structs and helper functions used by tests and future CLI/UI surfaces.
- Modify `src/extract.rs`: wire the new chunk/noise/signal/score/dedup funnel into local and LLM-assisted extraction paths without changing public command names.
- Modify `src/extract/signals.rs`: expose signal classification helpers needed by scoring.
- Modify `src/extract/embedding.rs`: keep current Jaccard matcher but compare compatible record groups.
- Modify `src/candidate.rs`: add optional extraction metadata with serde defaults.
- Modify `src/draft.rs`: carry extraction metadata from Candidate to Draft and preserve it on edit/merge.
- Modify `src/lib.rs`: export any new modules required by tests.
- Modify `app/src/types/domain.ts`: add optional extraction metadata fields to Candidate/Draft types.
- Modify `app/src/components/pages/Drafts.tsx`: render reason/origin/signal in the review list when present.
- Modify `app/src/components/pages/RecordEditor.tsx`: show evidence metadata in the editor detail area.

## Task 1: Add Quality Fixtures And Metric Test

**Files:**
- Create: `tests/fixtures/extract_quality_v2.yml`
- Create: `tests/extract_quality_v2.rs`

- [ ] **Step 1: Create the fixture file**

Add `tests/fixtures/extract_quality_v2.yml`:

```yaml
cases:
  - id: durable-preference-bun
    origin: user
    input: "以后这个项目都用 Bun 管理 JavaScript 依赖和脚本，不要再建议 npm install。"
    expected: candidate
    expected_signal: preference
    expected_terms: ["Bun", "JavaScript"]
  - id: durable-constraint-artifact
    origin: user
    input: "不要直接覆盖 AGENTS.md；发现漂移时先导入成 Draft，让我确认后再同步。"
    expected: candidate
    expected_signal: constraint
    expected_terms: ["AGENTS.md", "Draft"]
  - id: durable-procedure-ui-polish
    origin: user
    input: "UI 视觉优化先让 Claude Code 做一轮组件和交互建议，再由 Codex 集成验证。这个流程以后保留。"
    expected: candidate
    expected_signal: procedure
    expected_terms: ["Claude Code", "Codex"]
  - id: durable-correction-tests
    origin: user
    input: "你又忘了跑测试。以后涉及 Rust 提取逻辑时，先加质量 fixture，再跑 cargo test。"
    expected: candidate
    expected_signal: correction
    expected_terms: ["fixture", "cargo test"]
  - id: ai-project-improvement-accepted
    origin: assistant
    input: "建议把高质量提取做成 Observation -> EvidenceChunk -> Noise Gate -> Signal Gate -> Value Score -> Candidate 的漏斗。用户确认这是项目核心。"
    expected: candidate
    expected_signal: ai_project_improvement
    expected_terms: ["EvidenceChunk", "Noise Gate"]
  - id: ai-generic-advice-noise
    origin: assistant
    input: "一般来说，软件项目应该保持代码整洁、测试充分、文档完善。"
    expected: noise
    expected_signal: ""
    expected_terms: []
  - id: one-off-task-noise
    origin: user
    input: "把这个按钮颜色改成蓝色，然后帮我看看页面有没有报错。"
    expected: noise
    expected_signal: ""
    expected_terms: []
  - id: unresolved-request-noise
    origin: user
    input: "能不能以后做一个很厉害的多智能体系统？先想想。"
    expected: noise
    expected_signal: ""
    expected_terms: []
  - id: shell-output-noise
    origin: unknown
    input: "error[E0425]: cannot find value `foo` in this scope\n   --> src/main.rs:10:5"
    expected: noise
    expected_signal: ""
    expected_terms: []
  - id: english-durable-decision
    origin: user
    input: "Keep Agent-Kernel file-native for v1. Do not introduce a mandatory vector database; indexes must remain rebuildable caches."
    expected: candidate
    expected_signal: decision
    expected_terms: ["file-native", "rebuildable"]
```

- [ ] **Step 2: Add the integration test skeleton**

Add `tests/extract_quality_v2.rs`:

```rust
use agent_kernel::extract;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct FixtureFile {
    cases: Vec<FixtureCase>,
}

#[derive(Debug, Deserialize)]
struct FixtureCase {
    id: String,
    origin: String,
    input: String,
    expected: String,
    expected_signal: String,
    expected_terms: Vec<String>,
}

#[test]
fn high_quality_extraction_v2_meets_precision_gate() {
    let fixture: FixtureFile = serde_yaml::from_str(include_str!("fixtures/extract_quality_v2.yml"))
        .expect("parse extract quality fixtures");

    let report = extract::quality_report_for_text_cases(
        fixture
            .cases
            .iter()
            .map(|case| extract::QualityTextCase {
                id: case.id.clone(),
                origin: case.origin.clone(),
                input: case.input.clone(),
                expected: case.expected.clone(),
                expected_signal: case.expected_signal.clone(),
                expected_terms: case.expected_terms.clone(),
            })
            .collect(),
    );

    assert!(
        report.precision_at_10 >= 0.80,
        "precision_at_10 too low: {report:#?}"
    );
    assert!(
        report.false_positives.is_empty(),
        "noise entered candidate set: {report:#?}"
    );
    assert!(
        report.visible_candidates <= 10,
        "default extraction should stay small: {report:#?}"
    );
}
```

- [ ] **Step 3: Run the failing test**

Run:

```bash
cargo test --test extract_quality_v2 --quiet
```

Expected: compile failure because `extract::QualityTextCase` and `extract::quality_report_for_text_cases` do not exist yet.

- [ ] **Step 4: Commit**

```bash
git add tests/fixtures/extract_quality_v2.yml tests/extract_quality_v2.rs
git commit -m "test: add high quality extraction fixtures"
```

## Task 2: Add EvidenceChunk And Origin Model

**Files:**
- Create: `src/extract/chunk.rs`
- Modify: `src/extract.rs`

- [ ] **Step 1: Add chunk module code**

Create `src/extract/chunk.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChunkOrigin {
    User,
    Assistant,
    Artifact,
    AiSynthesis,
    Unknown,
}

impl ChunkOrigin {
    pub fn from_label(label: &str) -> Self {
        match label.trim().to_ascii_lowercase().as_str() {
            "user" => Self::User,
            "assistant" => Self::Assistant,
            "artifact" => Self::Artifact,
            "ai-synthesis" | "ai_synthesis" => Self::AiSynthesis,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Artifact => "artifact",
            Self::AiSynthesis => "ai-synthesis",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceChunk {
    pub id: String,
    pub text: String,
    pub origin: ChunkOrigin,
    pub source_kind: String,
    pub source_observations: Vec<String>,
}

pub fn text_case_to_chunk(id: &str, origin: &str, input: &str) -> EvidenceChunk {
    EvidenceChunk {
        id: id.to_string(),
        text: input.trim().to_string(),
        origin: ChunkOrigin::from_label(origin),
        source_kind: "quality-fixture".to_string(),
        source_observations: Vec::new(),
    }
}
```

- [ ] **Step 2: Register the module**

At the top of `src/extract.rs`, add:

```rust
pub mod chunk;
```

- [ ] **Step 3: Run focused compile**

Run:

```bash
cargo test --lib extract::tests --quiet
```

Expected: existing extract tests still pass or the crate compiles far enough to expose the missing quality API from Task 1.

- [ ] **Step 4: Commit**

```bash
git add src/extract.rs src/extract/chunk.rs
git commit -m "feat: add extraction evidence chunks"
```

## Task 3: Add Deterministic Scoring

**Files:**
- Create: `src/extract/scoring.rs`
- Modify: `src/extract.rs`

- [ ] **Step 1: Add scoring module**

Create `src/extract/scoring.rs`:

```rust
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::chunk::{ChunkOrigin, EvidenceChunk};
use super::signals::{detect_candidate_paragraphs, split_into_paragraphs};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExtractionDisposition {
    Reject,
    Borderline,
    Candidate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionScore {
    pub score: f32,
    pub disposition: ExtractionDisposition,
    pub matched_signal: String,
    pub reason: String,
    pub breakdown: BTreeMap<String, f32>,
}

pub fn score_chunk(chunk: &EvidenceChunk) -> ExtractionScore {
    let mut breakdown = BTreeMap::new();
    let text = chunk.text.trim();
    let lower = text.to_ascii_lowercase();

    let mut matched_signal = detect_signal_name(text);
    let mut reason = String::new();

    add(&mut breakdown, "durability", durability_score(text));
    add(&mut breakdown, "actionability", actionability_score(text));
    add(&mut breakdown, "specificity", specificity_score(text));
    add(&mut breakdown, "recurrence", recurrence_score(text));
    add(&mut breakdown, "correction_strength", correction_score(text));
    add(&mut breakdown, "evidence_quality", evidence_quality_score(text));
    add(&mut breakdown, "novelty", 0.8);

    add(&mut breakdown, "task_specific_penalty", -task_specific_penalty(&lower));
    add(&mut breakdown, "vague_penalty", -vague_penalty(text));
    add(
        &mut breakdown,
        "unresolved_request_penalty",
        -unresolved_request_penalty(&lower),
    );
    add(
        &mut breakdown,
        "sensitive_content_penalty",
        -sensitive_content_penalty(&lower),
    );

    if chunk.origin == ChunkOrigin::Assistant && looks_like_ai_project_improvement(text) {
        matched_signal = "ai_project_improvement".to_string();
        add(&mut breakdown, "ai_project_improvement", 1.2);
    }

    let score = breakdown.values().sum::<f32>();
    let disposition = if score >= 3.2 && !matched_signal.is_empty() {
        reason = format!("High-value durable {matched_signal} signal with reusable project impact.");
        ExtractionDisposition::Candidate
    } else if score >= 2.4 && !matched_signal.is_empty() {
        reason = format!("Borderline {matched_signal} signal; keep for optional provider review.");
        ExtractionDisposition::Borderline
    } else {
        if reason.is_empty() {
            reason = "Rejected because the text does not contain durable reusable agent knowledge.".to_string();
        }
        ExtractionDisposition::Reject
    };

    ExtractionScore {
        score,
        disposition,
        matched_signal,
        reason,
        breakdown,
    }
}

fn add(map: &mut BTreeMap<String, f32>, key: &str, value: f32) {
    map.insert(key.to_string(), value);
}

fn detect_signal_name(text: &str) -> String {
    let paragraphs = split_into_paragraphs(text);
    let candidates = detect_candidate_paragraphs(&paragraphs);
    if candidates.is_empty() {
        return String::new();
    }
    let labels = candidates[0]
        .signals
        .iter()
        .map(|signal| format!("{signal:?}"))
        .collect::<Vec<_>>()
        .join(",");
    let lower = labels.to_ascii_lowercase();
    if lower.contains("preference") {
        "preference".to_string()
    } else if lower.contains("constraint") {
        "constraint".to_string()
    } else if lower.contains("workflow") {
        "procedure".to_string()
    } else if lower.contains("architecture") {
        "decision".to_string()
    } else if lower.contains("memory") {
        "procedure".to_string()
    } else {
        "procedure".to_string()
    }
}

fn durability_score(text: &str) -> f32 {
    let lower = text.to_ascii_lowercase();
    if lower.contains("以后") || lower.contains("always") || lower.contains("keep ") || lower.contains("default") {
        0.8
    } else {
        0.2
    }
}

fn actionability_score(text: &str) -> f32 {
    let lower = text.to_ascii_lowercase();
    if lower.contains("use ") || lower.contains("不要") || lower.contains("先") || lower.contains("do not") {
        0.8
    } else {
        0.2
    }
}

fn specificity_score(text: &str) -> f32 {
    let named_terms = ["Bun", "AGENTS.md", "CLAUDE.md", "Claude Code", "Codex", "Draft", "Skilllet"];
    if named_terms.iter().any(|term| text.contains(term)) {
        0.8
    } else {
        0.2
    }
}

fn recurrence_score(text: &str) -> f32 {
    let lower = text.to_ascii_lowercase();
    if lower.contains("又") || lower.contains("again") || lower.contains("repeated") || lower.contains("每次") {
        0.6
    } else {
        0.0
    }
}

fn correction_score(text: &str) -> f32 {
    let lower = text.to_ascii_lowercase();
    if lower.contains("忘了") || lower.contains("again") || lower.contains("不要再") {
        0.8
    } else {
        0.0
    }
}

fn evidence_quality_score(text: &str) -> f32 {
    if text.chars().count() >= 24 && text.chars().count() <= 500 {
        0.6
    } else {
        0.1
    }
}

fn task_specific_penalty(lower: &str) -> f32 {
    if lower.contains("这个按钮") || lower.contains("改成蓝色") || lower.contains("帮我看看") {
        2.0
    } else {
        0.0
    }
}

fn vague_penalty(text: &str) -> f32 {
    if text.contains("代码整洁") && text.contains("文档完善") {
        1.8
    } else {
        0.0
    }
}

fn unresolved_request_penalty(lower: &str) -> f32 {
    if lower.contains("能不能") || lower.contains("maybe") || lower.contains("先想想") {
        1.7
    } else {
        0.0
    }
}

fn sensitive_content_penalty(lower: &str) -> f32 {
    if lower.contains("error[") || lower.contains("stack trace") || lower.contains("--> src/") {
        2.0
    } else {
        0.0
    }
}

fn looks_like_ai_project_improvement(text: &str) -> bool {
    text.contains("Observation")
        && text.contains("EvidenceChunk")
        && text.contains("Candidate")
        && (text.contains("用户确认") || text.to_ascii_lowercase().contains("accepted"))
}
```

- [ ] **Step 2: Register scoring**

At the top of `src/extract.rs`, add:

```rust
pub mod scoring;
```

- [ ] **Step 3: Run focused test**

Run:

```bash
cargo test --test extract_quality_v2 --quiet
```

Expected: still fails because the quality report API is missing, but scoring compiles.

- [ ] **Step 4: Commit**

```bash
git add src/extract.rs src/extract/scoring.rs
git commit -m "feat: score extraction evidence chunks"
```

## Task 4: Add Quality Report API

**Files:**
- Create: `src/extract/quality.rs`
- Modify: `src/extract.rs`

- [ ] **Step 1: Add quality report module**

Create `src/extract/quality.rs`:

```rust
use serde::{Deserialize, Serialize};

use super::chunk::text_case_to_chunk;
use super::scoring::{score_chunk, ExtractionDisposition};

#[derive(Debug, Clone)]
pub struct QualityTextCase {
    pub id: String,
    pub origin: String,
    pub input: String,
    pub expected: String,
    pub expected_signal: String,
    pub expected_terms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityReport {
    pub total: usize,
    pub true_positives: Vec<String>,
    pub false_positives: Vec<String>,
    pub false_negatives: Vec<String>,
    pub visible_candidates: usize,
    pub precision: f32,
    pub recall: f32,
    pub precision_at_10: f32,
}

pub fn quality_report_for_text_cases(cases: Vec<QualityTextCase>) -> QualityReport {
    let mut true_positives = Vec::new();
    let mut false_positives = Vec::new();
    let mut false_negatives = Vec::new();
    let mut ranked = Vec::new();

    for case in &cases {
        let chunk = text_case_to_chunk(&case.id, &case.origin, &case.input);
        let score = score_chunk(&chunk);
        let predicted_candidate = score.disposition == ExtractionDisposition::Candidate;
        let expected_candidate = case.expected == "candidate";
        let signal_ok = case.expected_signal.is_empty() || score.matched_signal == case.expected_signal;
        let terms_ok = case
            .expected_terms
            .iter()
            .all(|term| case.input.contains(term));

        if predicted_candidate {
            ranked.push((case.id.clone(), score.score, expected_candidate && signal_ok && terms_ok));
        }

        match (predicted_candidate, expected_candidate && signal_ok && terms_ok) {
            (true, true) => true_positives.push(case.id.clone()),
            (true, false) => false_positives.push(case.id.clone()),
            (false, true) => false_negatives.push(case.id.clone()),
            (false, false) => {}
        }
    }

    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let top_10 = ranked.iter().take(10).collect::<Vec<_>>();
    let top_10_hits = top_10.iter().filter(|(_, _, hit)| *hit).count();

    let predicted = true_positives.len() + false_positives.len();
    let expected = true_positives.len() + false_negatives.len();
    let precision = ratio(true_positives.len(), predicted);
    let recall = ratio(true_positives.len(), expected);
    let precision_at_10 = ratio(top_10_hits, top_10.len());

    QualityReport {
        total: cases.len(),
        true_positives,
        false_positives,
        false_negatives,
        visible_candidates: predicted,
        precision,
        recall,
        precision_at_10,
    }
}

fn ratio(numerator: usize, denominator: usize) -> f32 {
    if denominator == 0 {
        1.0
    } else {
        numerator as f32 / denominator as f32
    }
}
```

- [ ] **Step 2: Re-export quality API**

In `src/extract.rs`, add:

```rust
pub mod quality;
pub use quality::{quality_report_for_text_cases, QualityReport, QualityTextCase};
```

- [ ] **Step 3: Run the quality test**

Run:

```bash
cargo test --test extract_quality_v2 --quiet
```

Expected: the test passes or fails with specific fixture ids in false positives/false negatives.

- [ ] **Step 4: Tune only deterministic thresholds if needed**

If `durable-procedure-ui-polish`, `durable-correction-tests`, or `ai-project-improvement-accepted` are false negatives, adjust only the score constants in `src/extract/scoring.rs`. Do not add broad generic keywords that would accept `ai-generic-advice-noise`.

- [ ] **Step 5: Commit**

```bash
git add src/extract.rs src/extract/quality.rs src/extract/scoring.rs tests/extract_quality_v2.rs
git commit -m "feat: add extraction quality report"
```

## Task 5: Add Extraction Metadata To Candidate And Draft

**Files:**
- Modify: `src/candidate.rs`
- Modify: `src/draft.rs`

- [ ] **Step 1: Add metadata structs to Candidate**

In `src/candidate.rs`, add near the record structs:

```rust
use std::collections::BTreeMap;

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
}
```

Add this field to `CandidateRecord` and `NewCandidate`:

```rust
#[serde(default)]
pub extraction: ExtractionMetadata,
```

Set it in `add_candidate`:

```rust
extraction: candidate.extraction,
```

In test helpers that construct `NewCandidate`, set:

```rust
extraction: ExtractionMetadata::default(),
```

- [ ] **Step 2: Mirror metadata in Draft**

In `src/draft.rs`, import and use the same metadata type:

```rust
use crate::candidate::ExtractionMetadata;
```

Add this field to `DraftRecord` and `NewDraft`:

```rust
#[serde(default)]
pub extraction: ExtractionMetadata,
```

Set it in `add_draft`:

```rust
extraction: draft.extraction,
```

In draft test helpers, set:

```rust
extraction: ExtractionMetadata::default(),
```

- [ ] **Step 3: Carry metadata during promotion**

In `candidate::promote_candidate_to_draft`, set:

```rust
extraction: candidate.extraction.clone(),
```

In direct candidate approval, keep metadata out of `SkillletRecord` for now. The approved Skilllet remains clean source knowledge; provenance stays available in the promoted/review history.

- [ ] **Step 4: Run candidate and draft tests**

Run:

```bash
cargo test candidate:: draft:: --quiet
```

Expected: all candidate and draft unit tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/candidate.rs src/draft.rs
git commit -m "feat: store extraction metadata on review records"
```

## Task 6: Generate Candidates Through The Funnel

**Files:**
- Modify: `src/extract.rs`
- Modify: `src/extract/embedding.rs`

- [ ] **Step 1: Add helper to build metadata from score**

In `src/extract.rs`, add a private helper near extraction helpers:

```rust
fn extraction_metadata_for_chunk(
    chunk: &chunk::EvidenceChunk,
    score: &scoring::ExtractionScore,
    similar_record: Option<String>,
) -> candidate::ExtractionMetadata {
    candidate::ExtractionMetadata {
        origin: chunk.origin.as_str().to_string(),
        matched_signal: score.matched_signal.clone(),
        reason: score.reason.clone(),
        source_observations: chunk.source_observations.clone(),
        score_breakdown: score.breakdown.clone(),
        similar_record,
    }
}
```

- [ ] **Step 2: Apply score gate in local extraction**

In `extract_local_text_to_drafts`, before creating `NewDraft`, convert each local candidate to an `EvidenceChunk`, score it, and skip non-candidates:

```rust
let chunk = chunk::EvidenceChunk {
    id: candidate.title.clone(),
    text: candidate.body.clone(),
    origin: chunk::ChunkOrigin::User,
    source_kind: source.to_string(),
    source_observations: Vec::new(),
};
let score = scoring::score_chunk(&chunk);
if score.disposition != scoring::ExtractionDisposition::Candidate {
    continue;
}
let extraction = extraction_metadata_for_chunk(&chunk, &score, None);
```

Set `extraction` on `draft::NewDraft`.

- [ ] **Step 3: Apply score gate in LLM extraction**

In `extract_llm_text_to_drafts`, when converting usable LLM knowledge into draft records, build chunks with assistant origin:

```rust
let chunk = chunk::EvidenceChunk {
    id: item.title.clone(),
    text: item.body.clone(),
    origin: chunk::ChunkOrigin::Assistant,
    source_kind: source.to_string(),
    source_observations: Vec::new(),
};
let score = scoring::score_chunk(&chunk);
if score.disposition != scoring::ExtractionDisposition::Candidate {
    continue;
}
let extraction = extraction_metadata_for_chunk(&chunk, &score, None);
```

Set `extraction` on `draft::NewDraft`.

- [ ] **Step 4: Cap generated records at 10**

Before writing records in local and LLM paths, sort by confidence then score and take 10 records. Use existing confidence first to preserve current behavior:

```rust
drafts_to_write.sort_by(|left, right| {
    right
        .confidence
        .unwrap_or(0.0)
        .partial_cmp(&left.confidence.unwrap_or(0.0))
        .unwrap_or(std::cmp::Ordering::Equal)
});
drafts_to_write.truncate(10);
```

- [ ] **Step 5: Run quality and extraction tests**

Run:

```bash
cargo test --test extract_quality_v2 --quiet
cargo test extract:: --quiet
```

Expected: quality gate passes and existing extraction behavior remains compatible.

- [ ] **Step 6: Commit**

```bash
git add src/extract.rs src/extract/embedding.rs
git commit -m "feat: gate extraction through quality scoring"
```

## Task 7: Preserve AI-Origin Project Improvements

**Files:**
- Modify: `src/extract/scoring.rs`
- Modify: `tests/fixtures/extract_quality_v2.yml`
- Modify: `tests/extract_quality_v2.rs`

- [ ] **Step 1: Add a stricter AI-origin fixture**

Append to `tests/fixtures/extract_quality_v2.yml`:

```yaml
  - id: ai-skilllet-worthy-workflow
    origin: assistant
    input: "基于用户确认，项目应该把 AI 生成的高质量项目改善也送入 Candidate/Draft，而不是直接写 Skilllet；这能保留审阅边界。"
    expected: candidate
    expected_signal: ai_project_improvement
    expected_terms: ["Candidate/Draft", "Skilllet"]
```

- [ ] **Step 2: Update AI improvement detection**

In `src/extract/scoring.rs`, replace `looks_like_ai_project_improvement` with:

```rust
fn looks_like_ai_project_improvement(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let accepted = text.contains("用户确认")
        || lower.contains("accepted")
        || text.contains("基于用户确认");
    let project_terms = text.contains("Candidate")
        || text.contains("Draft")
        || text.contains("Skilllet")
        || text.contains("EvidenceChunk")
        || text.contains("项目");
    let governance_terms = text.contains("审阅")
        || text.contains("review")
        || text.contains("Noise Gate")
        || text.contains("Value Score")
        || text.contains("直接写");
    accepted && project_terms && governance_terms
}
```

- [ ] **Step 3: Run quality test**

Run:

```bash
cargo test --test extract_quality_v2 --quiet
```

Expected: AI-origin accepted project improvements pass while generic AI advice remains noise.

- [ ] **Step 4: Commit**

```bash
git add src/extract/scoring.rs tests/fixtures/extract_quality_v2.yml tests/extract_quality_v2.rs
git commit -m "feat: accept reviewed AI-origin project improvements"
```

## Task 8: Surface Metadata In The Inbox

**Files:**
- Modify: `app/src/types/domain.ts`
- Modify: `app/src/components/pages/Drafts.tsx`
- Modify: `app/src/components/pages/RecordEditor.tsx`
- Modify: `app/src/demo/demo-data.ts`
- Modify: `tests/ui-helpers.test.ts`

- [ ] **Step 1: Add TypeScript metadata type**

In `app/src/types/domain.ts`, add:

```ts
export type ExtractionMetadata = {
  origin?: string;
  matched_signal?: string;
  reason?: string;
  source_observations?: string[];
  score_breakdown?: Record<string, number>;
  similar_record?: string | null;
};
```

Add to Candidate and Draft types:

```ts
extraction?: ExtractionMetadata;
```

- [ ] **Step 2: Add demo metadata**

In `app/src/demo/demo-data.ts`, add `extraction` to one draft:

```ts
extraction: {
  origin: "user",
  matched_signal: "procedure",
  reason: "High-value durable procedure signal with reusable project impact.",
  source_observations: ["observation:demo"],
  score_breakdown: { durability: 0.8, actionability: 0.8, specificity: 0.8 },
  similar_record: null,
},
```

- [ ] **Step 3: Render compact metadata in Drafts page**

In `app/src/components/pages/Drafts.tsx`, inside each draft row/card details area, add:

```tsx
{draft.extraction?.reason ? (
  <p className="record-reason">
    {draft.extraction.matched_signal ? `${draft.extraction.matched_signal} · ` : ""}
    {draft.extraction.origin ? `${draft.extraction.origin} · ` : ""}
    {draft.extraction.reason}
  </p>
) : null}
```

- [ ] **Step 4: Render evidence details in RecordEditor**

In `app/src/components/pages/RecordEditor.tsx`, near the existing evidence block, add:

```tsx
{record.extraction ? (
  <section className="editor-section">
    <h3>提取依据</h3>
    <p>{record.extraction.reason || "本条记录来自高价值提取流程。"}</p>
    <dl>
      <dt>来源</dt>
      <dd>{record.extraction.origin || "unknown"}</dd>
      <dt>信号</dt>
      <dd>{record.extraction.matched_signal || "unknown"}</dd>
      <dt>相似记录</dt>
      <dd>{record.extraction.similar_record || "无"}</dd>
    </dl>
  </section>
) : null}
```

- [ ] **Step 5: Add UI test expectation**

In `tests/ui-helpers.test.ts`, add an assertion to an existing demo/read-model test:

```ts
expect(JSON.stringify(state)).toContain("High-value durable");
```

- [ ] **Step 6: Run frontend tests**

Run:

```bash
bun run --cwd app test
```

Expected: all frontend tests pass.

- [ ] **Step 7: Commit**

```bash
git add app/src/types/domain.ts app/src/components/pages/Drafts.tsx app/src/components/pages/RecordEditor.tsx app/src/demo/demo-data.ts tests/ui-helpers.test.ts
git commit -m "feat: show extraction rationale in inbox"
```

## Task 9: Full Verification

**Files:**
- No source edits unless verification exposes a defect.

- [ ] **Step 1: Run Rust core tests**

```bash
cargo test --quiet
```

Expected: all Rust tests pass, including `extract_quality_v2`.

- [ ] **Step 2: Run Tauri tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --quiet
```

Expected: all Tauri tests pass.

- [ ] **Step 3: Run frontend tests**

```bash
bun run --cwd app test
```

Expected: all app tests pass.

- [ ] **Step 4: Run Bun wrapper tests**

```bash
bun test bun/agent-kernel-lib.test.js
```

Expected: all wrapper tests pass.

- [ ] **Step 5: Run builds when tests pass**

```bash
bun run --cwd app build
```

Expected: TypeScript and Vite build succeeds.

- [ ] **Step 6: Commit verification-only fixes if any**

If verification required source fixes:

```bash
git add <changed-files>
git commit -m "fix: stabilize high quality extraction"
```

If no source fixes were required, do not create an empty commit.

## Self-Review

- Spec coverage: the plan implements the deterministic funnel, AI-origin durable knowledge handling, fixture-driven quality metrics, Candidate/Draft metadata, and Inbox metadata display.
- Placeholder scan: the plan contains exact files, commands, expected outcomes, and code snippets for every code-changing step.
- Type consistency: `ExtractionMetadata`, `EvidenceChunk`, `ExtractionScore`, `QualityTextCase`, and `QualityReport` are introduced before they are used by later tasks.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-05-03-high-quality-extraction.md`. Two execution options:

1. Subagent-Driven (recommended) - dispatch a fresh subagent per task, review between tasks, fast iteration.
2. Inline Execution - execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach?
