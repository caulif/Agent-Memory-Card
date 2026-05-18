//! 调试用：把 STRIP 后被标 system role 的消息全部打印，便于扩充
//! `looks_like_system_prompt` 名单。
//!
//! 跑：`cargo test --test strip_inspect_system -- --ignored --nocapture`

use agent_kernel::extract::strip::strip_observations;
use agent_kernel::observation::load_observations;
use std::path::Path;

#[test]
#[ignore]
fn print_system_role_messages() {
    let project_root = Path::new(".");
    let records = load_observations(project_root).expect("load");
    let stripped = strip_observations(&records);
    let systems: Vec<_> = stripped.iter().filter(|m| m.role == "system").collect();
    println!("=== {} system-role messages ===", systems.len());
    for (i, m) in systems.iter().enumerate() {
        let head: String = m.body.chars().take(180).collect();
        println!(
            "[{:02}] obs={} len={} head=\"{}\"",
            i + 1,
            m.observation_id,
            m.stripped_len,
            head.replace('\n', " | ")
        );
    }
}
