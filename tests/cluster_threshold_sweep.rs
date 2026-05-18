//! 聚类阈值扫描：扫描 jaccard long/short 阈值的不同组合，把每组
//! recall/precision 数字输出到 docs/runs/cluster-sweep-<timestamp>.md。
//!
//! **不是常规 CI 测试**：标了 `#[ignore]`，需要手动跑：
//!
//! ```bash
//! cargo test --test cluster_threshold_sweep -- --ignored --nocapture
//! ```
//!
//! 目的：当 STRIP/INDUCE 改进后，重新跑这个 sweep 决定 jaccard fallback
//! 阈值的新默认值。

use std::fs;
use std::path::PathBuf;

use agent_kernel::extract::cluster::ClusterOptions;
use agent_kernel::extract::pipeline::{PipelineOptions, run_pipeline_with};
use agent_kernel::observation::ObservationRecord;
use agent_kernel::provider::ProviderRequest;
use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct GoldenSet {
    positives: Vec<Case>,
    negatives: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Case {
    id: String,
    user_messages: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
    description: String,
}

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
            id: format!("obs:sweep:{case_id}:{idx}"),
            source_kind: "claude-code-session".to_string(),
            source_path: format!("tests/golden/{case_id}.jsonl"),
            agent: Some("claude-code".to_string()),
            body: body.clone(),
            evidence: "sweep".to_string(),
            redacted: false,
            created_at: format!("2026-05-01T00:{:02}:00+00:00", idx),
        })
        .collect()
}

fn evaluate(jaccard_long: f32, jaccard_short: f32, golden: &GoldenSet) -> (f32, f32) {
    let options = PipelineOptions {
        cluster: ClusterOptions {
            force_jaccard: true,
            jaccard_long_threshold: jaccard_long,
            jaccard_short_threshold: jaccard_short,
            keep_singletons: true,
            ..ClusterOptions::default()
        },
        induce: Default::default(),
        skip_induce: true,
    };

    let mut positive_hits = 0;
    for case in &golden.positives {
        let obs = build_observations(&case.id, &case.user_messages);
        let report = run_pipeline_with(&obs, &NoopProvider, &options).unwrap();
        if report.stripped_kept >= 2 && report.clusters_in < report.stripped_kept {
            positive_hits += 1;
        }
    }
    let recall = (positive_hits as f32) * 100.0 / (golden.positives.len() as f32);

    let mut negative_rejected = 0;
    for case in &golden.negatives {
        let obs = build_observations(&case.id, &case.user_messages);
        let report = run_pipeline_with(&obs, &NoopProvider, &options).unwrap();
        if report.stripped_kept == 0 || report.clusters_in >= report.stripped_kept {
            negative_rejected += 1;
        }
    }
    let precision = (negative_rejected as f32) * 100.0 / (golden.negatives.len() as f32);

    (recall, precision)
}

fn manifest_path(relative: &[&str]) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for part in relative {
        p.push(part);
    }
    p
}

#[test]
#[ignore]
fn sweep_jaccard_thresholds_and_write_report() {
    let golden_text =
        fs::read_to_string(manifest_path(&["tests", "golden", "golden_set.yml"])).unwrap();
    let golden: GoldenSet = serde_yaml::from_str(&golden_text).unwrap();

    let long_grid = [0.10_f32, 0.15, 0.20, 0.25, 0.30, 0.40];
    let short_grid = [0.20_f32, 0.30, 0.40, 0.55];

    let mut rows: Vec<(f32, f32, f32, f32, f32)> = Vec::new();
    for &lt in &long_grid {
        for &st in &short_grid {
            let (recall, precision) = evaluate(lt, st, &golden);
            let f1 = if recall + precision > 0.0 {
                2.0 * recall * precision / (recall + precision)
            } else {
                0.0
            };
            rows.push((lt, st, recall, precision, f1));
        }
    }

    rows.sort_by(|a, b| b.4.partial_cmp(&a.4).unwrap_or(std::cmp::Ordering::Equal));

    let mut report = String::new();
    report.push_str("# Cluster Threshold Sweep\n\n");
    report.push_str("黄金集 6 positives / 7 negatives 上的 jaccard 阈值扫描。按 F1 降序。\n\n");
    report.push_str("| jaccard_long | jaccard_short | recall % | precision % | F1 |\n");
    report.push_str("|---|---|---|---|---|\n");
    for (lt, st, recall, precision, f1) in &rows {
        report.push_str(&format!(
            "| {:.2} | {:.2} | {:.0} | {:.0} | {:.1} |\n",
            lt, st, recall, precision, f1
        ));
    }

    let dir = manifest_path(&["docs", "runs"]);
    fs::create_dir_all(&dir).unwrap();
    let ts = chrono::Utc::now().format("%Y%m%dT%H%M%S");
    let path = dir.join(format!("cluster-sweep-{ts}.md"));
    fs::write(&path, &report).unwrap();

    eprintln!("Wrote {}", path.display());
    eprintln!("\n{report}");

    let top = rows[0];
    assert!(
        top.4 >= 40.0,
        "最佳 F1 仅 {:.1}，低于 40 警戒线；阈值组合 long={:.2} short={:.2}",
        top.4,
        top.0,
        top.1
    );
}
