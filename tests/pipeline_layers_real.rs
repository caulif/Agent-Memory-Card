//! Smoke test: STRIP → TRUNCATE → CLUSTER 三层端到端真实数据。
//!
//! 跑：`cargo test --test pipeline_layers_real -- --ignored --nocapture`

use agent_kernel::extract::cluster::cluster_messages;
use agent_kernel::extract::strip::strip_observations;
use agent_kernel::extract::truncate::truncate_messages;
use agent_kernel::observation::load_observations;
use std::path::Path;

#[test]
#[ignore]
fn three_layers_against_real_observations() {
    let project_root = Path::new(".");
    let records = load_observations(project_root).expect("load");
    println!("=== Pipeline Layers 1-3 smoke test ===");
    println!("Layer 0 LOAD: {} observations", records.len());

    let stripped = strip_observations(&records);
    println!("Layer 1 STRIP: {} messages kept", stripped.len());

    let truncated = truncate_messages(&stripped);
    let truncate_count = truncated.iter().filter(|m| m.was_truncated).count();
    println!(
        "Layer 2 TRUNCATE: {} truncated / {} total",
        truncate_count,
        truncated.len()
    );

    let clusters = cluster_messages(&truncated).expect("cluster");
    println!("Layer 3 CLUSTER: {} multi-message clusters", clusters.len());

    println!("\n--- Top 8 clusters by recurrence ---");
    for (i, cluster) in clusters.iter().take(8).enumerate() {
        println!(
            "\n[Cluster #{}] id={} recurrence={} mean_sim={:.3}",
            i + 1,
            cluster.cluster_id,
            cluster.recurrence,
            cluster.mean_similarity
        );
        for (j, m) in cluster.messages.iter().enumerate() {
            let head: String = m.body.chars().take(140).collect();
            println!(
                "    [{}] obs={} created={} body=\"{}\"",
                j + 1,
                m.observation_id,
                m.created_at,
                head.replace('\n', " | ")
            );
        }
    }

    let single = truncated.len() - clusters.iter().map(|c| c.messages.len()).sum::<usize>();
    println!(
        "\n--- {} singleton messages dropped (would be kept with --include-singletons)",
        single
    );
}
