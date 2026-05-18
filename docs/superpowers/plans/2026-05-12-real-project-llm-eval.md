# Real Project LLM Eval Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a real-project evaluation flow that builds high-quality reference cards from local project observations, compares them with pipeline and baseline outputs, and uses an LLM judge to score which approach is closer to the desired standard.

**Architecture:** Reuse existing project discovery and observation import data. Add a dedicated eval command that runs three passes per project: reference synthesis, candidate pipeline output, and blind pairwise judging. Keep the judge separate from the generator, write all outputs to timestamped run folders, and preserve human review boundaries so nothing auto-promotes into approved memory cards.

**Tech Stack:** Rust, existing `agent-kernel` CLI, existing provider abstraction, YAML/JSON output, local project registry, observation store, existing extraction pipeline modules, prompt files in `prompts/`.

---

### Task 1: Define the evaluation surface and data model

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/main.rs`
- Create: `src/eval.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn eval_command_is_listed_and_dispatches() {
    let commands = agent_kernel::cli::Cli::command();
    let eval = commands
        .get_subcommands()
        .any(|cmd| cmd.get_name() == "eval");
    assert!(eval);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test eval_command_is_listed_and_dispatches -- --nocapture`
Expected: FAIL because `eval` subcommand does not exist yet.

- [ ] **Step 3: Write minimal implementation**

Add an `Eval` CLI command that accepts:

```rust
#[derive(Subcommand)]
pub(crate) enum Commands {
    // ...
    Eval {
        #[arg(long, default_value = ".")]
        project: PathBuf,
        #[arg(long)]
        projects: Vec<PathBuf>,
        #[arg(long, default_value_t = 3)]
        max_projects: usize,
        #[arg(long)]
        json: bool,
    },
}
```

Add a `RealProjectEvalRequest` / `RealProjectEvalReport` module in `src/eval.rs` that will hold:

```rust
pub struct RealProjectEvalRequest {
    pub projects: Vec<PathBuf>,
    pub max_projects: usize,
    pub provider_name: String,
}

pub struct RealProjectEvalReport {
    pub projects: Vec<ProjectEvalReport>,
    pub summary: EvalSummary,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test eval_command_is_listed_and_dispatches -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/cli.rs src/main.rs src/lib.rs src/eval.rs tests/<new-test-file>.rs
git commit -m "feat: add real-project eval command scaffold"
```

### Task 2: Add reference-card synthesis and judge prompts

**Files:**
- Create: `prompts/reference_synthesis.md`
- Create: `prompts/reference_quality_judge.md`
- Create: `prompts/pairwise_card_judge.md`
- Modify: `src/provider.rs` only if a new provider role is needed
- Create: `src/eval/reference.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn reference_prompt_files_exist() {
    assert!(std::path::Path::new("prompts/reference_synthesis.md").exists());
    assert!(std::path::Path::new("prompts/reference_quality_judge.md").exists());
    assert!(std::path::Path::new("prompts/pairwise_card_judge.md").exists());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test reference_prompt_files_exist -- --nocapture`
Expected: FAIL because the prompt files do not exist yet.

- [ ] **Step 3: Write minimal implementation**

Create prompts that do only these jobs:

```md
# reference_synthesis.md
- take raw observation material for one project
- produce candidate high-quality memory cards
- require source observation ids and short verbatim evidence snippets
- reject one-off, noisy, or weakly grounded cards
```

```md
# reference_quality_judge.md
- score each candidate for grounding, durability, reusability, and format quality
- accept only cards that are stable enough to be used as the reference set
- return structured JSON with pass/fail and reasons
```

```md
# pairwise_card_judge.md
- compare reference cards against pipeline or baseline outputs
- output pairwise win/loss/tie per dimension and a final preference
- stay blind to which side came from which system
```

Build `src/eval/reference.rs` so it can:

```rust
pub fn synthesize_reference_cards(
    project_root: &Path,
    observations: &[ObservationRecord],
    provider: &dyn ProviderLike,
) -> Result<Vec<ReferenceCard>>;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test reference_prompt_files_exist -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add prompts/reference_synthesis.md prompts/reference_quality_judge.md prompts/pairwise_card_judge.md src/eval/reference.rs
git commit -m "feat: add reference synthesis prompts"
```

### Task 3: Build project evaluation orchestration and output files

**Files:**
- Create: `src/eval/run.rs`
- Modify: `src/main.rs`
- Modify: `src/lib.rs`
- Create: `tests/eval_real_projects.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn eval_runner_writes_project_reports() {
    let report = agent_kernel::eval::run_real_project_eval_for_paths(
        &PathBuf::from("."),
        vec![PathBuf::from("C:/Users/15893/Documents/New project")],
        1,
        false,
    )
    .expect("eval");
    assert_eq!(report.projects.len(), 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test eval_runner_writes_project_reports -- --nocapture`
Expected: FAIL because the eval runner is not implemented yet.

- [ ] **Step 3: Write minimal implementation**

Make the runner:

```rust
pub fn run_real_project_eval_for_paths(
    project_root: &Path,
    project_paths: Vec<PathBuf>,
    max_projects: usize,
    json_only: bool,
) -> Result<RealProjectEvalReport> {
    // discover observations
    // run reference synthesis
    // run current pipeline
    // run legacy baseline when available
    // write docs/runs/eval-<timestamp>/...
}
```

Write per-project artifacts:

```text
docs/runs/eval-YYYYMMDDTHHMMSS/
  projects.yml
  reference/<project>.yml
  pipeline/<project>.json
  baseline/<project>.json
  judge/<project>.json
  summary.md
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test eval_runner_writes_project_reports -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/eval/run.rs src/main.rs src/lib.rs tests/eval_real_projects.rs
git commit -m "feat: add real-project eval orchestration"
```

### Task 4: Use real project data and add blind pairwise judging

**Files:**
- Create: `src/eval/judge.rs`
- Modify: `src/eval/run.rs`
- Create: `tests/eval_pairwise_judge.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn pairwise_judge_output_is_blind_and_structured() {
    let output = agent_kernel::eval::format_pairwise_judge_prompt(
        &reference_cards(),
        &pipeline_cards(),
        &baseline_cards(),
    );
    assert!(!output.contains("pipeline"));
    assert!(!output.contains("baseline"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test pairwise_judge_output_is_blind_and_structured -- --nocapture`
Expected: FAIL because the blind formatting does not exist yet.

- [ ] **Step 3: Write minimal implementation**

Build a judge API that returns:

```rust
pub struct PairwiseJudgeResult {
    pub winner: String,
    pub score_reference: f32,
    pub score_pipeline: f32,
    pub score_baseline: Option<f32>,
    pub notes: Vec<String>,
}
```

The judge prompt must:
- hide system names
- compare only content and evidence quality
- score against the reference set, not against internal implementation
- return machine-readable JSON

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test pairwise_judge_output_is_blind_and_structured -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/eval/judge.rs src/eval/run.rs tests/eval_pairwise_judge.rs
git commit -m "feat: add blind pairwise LLM judge"
```

### Task 5: Fix repo-rule size failures while keeping pipeline behavior intact

**Files:**
- Modify: `src/memory_card.rs`
- Modify: `src/observation.rs`
- Create or modify focused helper modules as needed
- Update: `tests/repo_rules.rs` only if it needs new explicit allowances

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn production_rust_files_stay_under_limit() {
    let memory_card_lines = std::fs::read_to_string("src/memory_card.rs").unwrap().lines().count();
    let observation_lines = std::fs::read_to_string("src/observation.rs").unwrap().lines().count();
    assert!(memory_card_lines < 1000);
    assert!(observation_lines < 1000);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test production_rust_files_stay_under_limit -- --nocapture`
Expected: FAIL until the split is done.

- [ ] **Step 3: Write minimal implementation**

Move helper functions and eval-specific glue out of the two oversized files into focused modules, keeping public behavior unchanged.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test production_rust_files_stay_under_limit -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/memory_card.rs src/observation.rs src/<new-helper>.rs tests/<new-test-file>.rs
git commit -m "refactor: shrink oversized core modules"
```

### Task 6: End-to-end verification on real local projects

**Files:**
- Modify: `tests/eval_real_projects.rs`
- Modify: `docs/runs/eval-*/summary.md` generated output

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn real_project_eval_has_at_least_one_project_with_reference_cards() {
    let report = agent_kernel::eval::run_real_project_eval_for_paths(
        &PathBuf::from("."),
        vec![
            PathBuf::from("C:/Users/15893/Documents/New project"),
            PathBuf::from("C:/aionui/AionUi"),
            PathBuf::from("C:/blog"),
        ],
        3,
        false,
    )
    .expect("eval");
    assert!(report.projects.iter().any(|p| !p.reference_cards.is_empty()));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test real_project_eval_has_at_least_one_project_with_reference_cards -- --nocapture`
Expected: FAIL until the full eval runner and judge are connected.

- [ ] **Step 3: Write minimal implementation**

Run the full eval against the top local projects, then compare:
- reference cards vs pipeline cards
- reference cards vs baseline cards
- judge summary by project and overall

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test real_project_eval_has_at_least_one_project_with_reference_cards -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add tests/eval_real_projects.rs docs/runs/eval-*/
git commit -m "test: add real project eval coverage"
```

---

**Spec coverage check**
- Real-project evaluation command: Task 1, Task 3
- Reference card generation: Task 2
- Blind LLM judge: Task 4
- Real local project execution: Task 6
- Repo rule cleanup needed for green test suite: Task 5

**Known gaps to watch during implementation**
- The judge can drift into rewarding style over substance; keep the rubric grounded in the reference cards and evidence quality.
- The reference set generator must stay separate from the final pipeline output, or the comparison becomes circular.
- The repo-rule line-count failure must be fixed before completion, or the suite will remain red even if the eval feature works.
