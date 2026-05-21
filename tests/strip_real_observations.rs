//! Smoke test: 用项目里 99 个真实 observation 跑一遍 STRIP，看压缩比。
//!
//! 默认 `#[ignore]`，因为依赖本地 `.agent-kernel/observations/` 数据；
//! 跑：`cargo test --test strip_real_observations -- --ignored --nocapture`

use agent_kernel::extract::strip::strip_observations;
use agent_kernel::observation::load_observations;
use std::path::Path;

#[test]
#[ignore]
fn strip_against_real_observations() {
    let project_root = Path::new(".");
    let records = load_observations(project_root).expect("load observations");

    println!("=== STRIP smoke test ===");
    println!("Loaded {} observations", records.len());

    let before_total: usize = records.iter().map(|r| r.body.len()).sum();
    let stripped = strip_observations(&records);
    let after_total: usize = stripped.iter().map(|m| m.stripped_len).sum();

    println!(
        "After STRIP: {} messages (kept {:.1}%); chars {} -> {} ({:.1}% retained)",
        stripped.len(),
        100.0 * stripped.len() as f64 / records.len().max(1) as f64,
        before_total,
        after_total,
        100.0 * after_total as f64 / before_total.max(1) as f64
    );

    let role_counts: std::collections::BTreeMap<&str, usize> =
        stripped.iter().fold(Default::default(), |mut map, m| {
            *map.entry(m.role.as_str()).or_insert(0) += 1;
            map
        });
    println!("Role distribution: {:?}", role_counts);

    println!("\n--- First 5 retained messages (head 200 chars each) ---");
    for (i, m) in stripped.iter().take(5).enumerate() {
        let head: String = m.body.chars().take(200).collect();
        println!(
            "[{}] obs={} role={} len={} body=\"{}\"",
            i + 1,
            m.observation_id,
            m.role,
            m.stripped_len,
            head.replace('\n', " | ")
        );
    }

    // 不强校验数字（数据会变），只确保链路不 panic 且消息数没爆
    assert!(stripped.len() <= records.len());
}
