//! 黄金集回归：从 tests/golden/golden_set.yml 加载 positives/negatives，
//! 跑前 3 层流水线（STRIP → TRUNCATE → CLUSTER），断言：
//!
//! - **positive**：multi-message 表达应聚成同一簇（recurrence ≥ 2）
//! - **negative**：单条噪声 / 一次性请求应该被 STRIP 拦截或保持 singleton
//!
//! 跳过 LLM 调用，让测试在 CI 无网络/无 LLM 时仍可跑。
//! INDUCE/CRYSTALLIZE 的端到端测试见 tests/pipeline_full_real.rs。
//!
//! 跑：`cargo test --test golden_set_regression -- --nocapture`

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use agent_kernel::extract::cluster::ClusterOptions;
use agent_kernel::extract::pipeline::{PipelineOptions, run_pipeline_with};
use agent_kernel::observation::ObservationRecord;
use agent_kernel::provider::ProviderRequest;
use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct GoldenSet {
    positives: Vec<PositiveCase>,
    negatives: Vec<NegativeCase>,
}

#[derive(Debug, Deserialize)]
struct PositiveCase {
    id: String,
    user_messages: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
    description: String,
}

#[derive(Debug, Deserialize)]
struct NegativeCase {
    id: String,
    user_messages: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
    description: String,
}

/// 测试不需要真实 INDUCE，给个永远 reject 的 stub 即可（流水线只跑 layer 1-3）。
struct NoopProvider;
impl agent_kernel::extract::induce::InduceProvider for NoopProvider {
    fn call(&self, _: &ProviderRequest, _: usize) -> Result<String> {
        Ok(r#"{"reject": true, "reason": "stub"}"#.to_string())
    }
}

fn build_observations(case_id: &str, messages: &[String]) -> Vec<ObservationRecord> {
    messages
        .iter()
        .enumerate()
        .map(|(idx, body)| ObservationRecord {
            id: format!("obs:golden:{case_id}:{idx}"),
            source_kind: "claude-code-session".to_string(),
            source_path: format!("tests/golden/{case_id}.jsonl"),
            agent: Some("claude-code".to_string()),
            body: body.clone(),
            evidence: "golden".to_string(),
            redacted: false,
            // 假装每条间隔 1 分钟，强化时序排序的可见性
            created_at: format!("2026-05-01T00:{:02}:00+00:00", idx),
        })
        .collect()
}

fn load_golden_set() -> GoldenSet {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join("golden_set.yml");
    let text =
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    serde_yaml::from_str(&text).unwrap_or_else(|err| panic!("parse {}: {err}", path.display()))
}

fn run_layers_1_to_3(
    observations: &[ObservationRecord],
) -> agent_kernel::extract::pipeline::PipelineReport {
    let options = PipelineOptions {
        cluster: ClusterOptions {
            // 用 jaccard，避免 CI 走 fastembed 网络。
            // 阈值用 cluster.rs 的默认值——黄金集回归测的就是"出厂质量"。
            force_jaccard: true,
            keep_singletons: true,
            ..ClusterOptions::default()
        },
        induce: Default::default(),
        skip_induce: true,
    };
    run_pipeline_with(observations, &NoopProvider, &options).expect("pipeline runs")
}

#[test]
fn positives_cluster_with_recurrence_at_least_two() {
    let golden = load_golden_set();
    let mut clustered = 0usize;
    let mut details: Vec<String> = Vec::new();
    let total = golden.positives.len();

    for case in &golden.positives {
        let obs = build_observations(&case.id, &case.user_messages);
        let report = run_layers_1_to_3(&obs);
        let stripped_n = report.stripped_kept;
        let clusters_n = report.clusters_in;
        // 期望：STRIP 至少保留 2 条 + 聚类合并出至少一个 size ≥ 2 的簇
        let ok = stripped_n >= 2 && clusters_n < stripped_n;
        if ok {
            clustered += 1;
        }
        details.push(format!(
            "  [{}] stripped={}, clusters={} → {}",
            case.id,
            stripped_n,
            clusters_n,
            if ok { "OK" } else { "MISS" }
        ));
    }

    let recall = (clustered as f32) * 100.0 / (total as f32);
    eprintln!(
        "黄金集 positive recall = {:.0}% ({}/{})\n{}",
        recall,
        clustered,
        total,
        details.join("\n")
    );

    // jaccard fallback 不能只靠二元字符重叠；工程语义锚点也应让多轮同义偏好
    // 聚成可归纳证据。黄金集 positive recall 应保持在 80% 以上。
    assert!(
        recall >= 80.0,
        "positive recall {recall:.0}% 低于 80% 质量门"
    );
}

#[test]
fn negatives_do_not_produce_meaningful_clusters() {
    let golden = load_golden_set();
    let mut rejected = 0usize;
    let mut details: Vec<String> = Vec::new();
    let total = golden.negatives.len();

    for case in &golden.negatives {
        let obs = build_observations(&case.id, &case.user_messages);
        let report = run_layers_1_to_3(&obs);
        let stripped_n = report.stripped_kept;
        let clusters_n = report.clusters_in;
        // 期望：要么 STRIP 全干掉，要么聚类后全是 singleton（没产出 size ≥ 2 的簇）
        let ok = stripped_n == 0 || clusters_n >= stripped_n;
        if ok {
            rejected += 1;
        }
        details.push(format!(
            "  [{}] stripped={}, clusters={} → {}",
            case.id,
            stripped_n,
            clusters_n,
            if ok { "OK (rejected)" } else { "LEAKED" }
        ));
    }

    let precision = (rejected as f32) * 100.0 / (total as f32);
    eprintln!(
        "黄金集 negative precision = {:.0}% ({}/{})\n{}",
        precision,
        rejected,
        total,
        details.join("\n")
    );
    assert!(
        precision >= 85.0,
        "negative precision {precision:.0}% 低于 85% 基线"
    );
}

#[test]
fn golden_set_has_minimum_size() {
    let golden = load_golden_set();
    assert!(
        golden.positives.len() >= 17,
        "黄金集 positives 至少 17 条，当前 {}",
        golden.positives.len()
    );
    assert!(
        golden.negatives.len() >= 28,
        "黄金集 negatives 至少 28 条，当前 {}",
        golden.negatives.len()
    );
    // id 唯一
    let mut seen = HashSet::new();
    for case in &golden.positives {
        assert!(seen.insert(&case.id), "重复 positive id: {}", case.id);
    }
    for case in &golden.negatives {
        assert!(seen.insert(&case.id), "重复 negative id: {}", case.id);
    }
}
