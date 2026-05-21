//! 端到端 smoke：99 个真实 observations + mock LLM provider，看流水线
//! 行为是否合理。
//!
//! 跑：`AGENT_KERNEL_DISABLE_FASTEMBED=1 cargo test --test pipeline_full_real -- --ignored --nocapture`

use agent_kernel::extract::cluster::ClusterOptions;
use agent_kernel::extract::induce::InduceProvider;
use agent_kernel::extract::pipeline::{PipelineOptions, run_pipeline_with};
use agent_kernel::observation::load_observations;
use agent_kernel::provider::ProviderRequest;
use anyhow::Result;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Mock provider：根据传入的 user_prompt 推测 cluster 内容，
/// 返回一个看起来合理的 candidate 或 reject。这不是真实 LLM，
/// 只是验证流水线连通性 + 数据 shape。
struct DemoProvider {
    counter: AtomicUsize,
}

impl InduceProvider for DemoProvider {
    fn call(&self, request: &ProviderRequest, _max_tokens: usize) -> Result<String> {
        let n = self.counter.fetch_add(1, Ordering::SeqCst);
        let user = &request.user_prompt;
        // 取簇内第一条 observation_id 与一段文本作为伪 evidence
        let obs_id = user
            .lines()
            .find_map(|l| {
                l.strip_prefix("--- [1] obs=")
                    .and_then(|rest| rest.split_whitespace().next())
            })
            .unwrap_or("o0")
            .to_string();
        // 评估前 3 个 cluster 给 accept；其余 reject 测试拒绝路径
        if n < 3 {
            // 取簇内第一条消息的前 20 字符作为 evidence_quote text
            // 找到 "--- [1] obs=... ---" 标题行之后的第一行真正的观测文本，
            // 这样 body_snippet 一定是 obs.full_text() 的子串，能通过证据校验。
            let body_snippet: String = {
                let mut lines = user.lines().skip_while(|l| !l.starts_with("--- [1] obs="));
                lines.next(); // 跳过 "--- [1] obs=... ---" 标题行
                let first_body_line = lines.next().unwrap_or("");
                first_body_line.chars().take(20).collect()
            };
            Ok(format!(
                r#"{{
                    "title": "测试卡片标题保持长度合规{n}",
                    "when": "在端到端 smoke 验证流水线时",
                    "what": "把每个簇按时序排列后归纳出可复用规则不要造证据",
                    "why": "确保流水线 5 层数据契约都能跑通",
                    "kind": "procedure",
                    "scope": "global",
                    "evidence_quotes": [
                        {{"observation_id": "{obs_id}", "text": "{}"}}
                    ],
                    "temporal_status": "stable",
                    "confidence": 0.78
                }}"#,
                body_snippet.replace('"', "'").replace('\\', "\\\\"),
            ))
        } else {
            Ok(r#"{"reject": true, "reason": "demo: only first 3 clusters accepted"}"#.to_string())
        }
    }
}

#[test]
#[ignore]
fn full_pipeline_with_real_observations_and_mock_llm() {
    // 强制 jaccard，因为环境无 fastembed 网络
    unsafe { std::env::set_var("AGENT_KERNEL_DISABLE_FASTEMBED", "1") };

    let project_root = Path::new(".");
    let observations = load_observations(project_root).expect("load");

    let provider = DemoProvider {
        counter: AtomicUsize::new(0),
    };
    let options = PipelineOptions {
        cluster: ClusterOptions::default(),
        induce: Default::default(),
        skip_induce: false,
    };

    let report = run_pipeline_with(&observations, &provider, &options).expect("pipeline ok");

    println!("=== Pipeline full smoke ===");
    println!("  obs_in           : {}", report.observations_in);
    println!("  stripped_kept    : {}", report.stripped_kept);
    println!("  truncated_total  : {}", report.truncated_count);
    println!("  truncated_long   : {}", report.truncated_long_messages);
    println!("  clusters         : {}", report.clusters_in);
    println!("  induce_accepted  : {}", report.induce_accepted);
    println!("  induce_rejected  : {}", report.induce_rejected);
    println!("  induce_failed    : {}", report.induce_failed);
    println!("  crystallize_acc  : {}", report.crystallize_accepted);
    println!("  crystallize_rej  : {}", report.crystallize_rejected);
    println!("  cards            : {}", report.cards.len());

    if !report.failures_preview.is_empty() {
        println!("\n--- failures preview ---");
        for line in &report.failures_preview {
            println!("  {line}");
        }
    }

    println!("\n--- accepted cards ---");
    for (i, card) in report.cards.iter().enumerate() {
        println!(
            "[{}] cluster={} kind={} scope={} activation={}",
            i + 1,
            card.cluster_id,
            card.kind,
            card.scope,
            card.activation
        );
        println!("    title: {}", card.title);
        println!("    body : {}", card.body);
        println!("    brief: {}", card.brief);
        println!("    tags : {:?}", card.tags);
    }

    // 不强校验数字，只确保链路不 panic
    assert!(report.observations_in > 0);
}
