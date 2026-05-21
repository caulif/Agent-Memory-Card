use agent_kernel::eval::{EvalCard, ReferenceCard, format_pairwise_judge_prompt};

fn reference_card() -> ReferenceCard {
    ReferenceCard {
        title: "优先真实历史回归".to_string(),
        body: "评估提炼质量或修改提炼逻辑时，优先使用真实历史会话做回归验证；不要只依赖静态样例。"
            .to_string(),
        brief: "用于让提炼规则接受真实历史检验。".to_string(),
        kind: "procedure".to_string(),
        scope: "global".to_string(),
        source_observations: vec!["obs:test:1".to_string()],
        evidence_quotes: vec!["用真实历史会话做回归验证".to_string()],
        confidence: Some(0.91),
        reason: Some("durable evaluation preference".to_string()),
    }
}

fn eval_card(title: &str, source: &str) -> EvalCard {
    EvalCard {
        title: title.to_string(),
        body: "评估提炼质量时，使用真实历史会话做回归验证。".to_string(),
        brief: "用于验证提炼规则。".to_string(),
        kind: "procedure".to_string(),
        scope: "global".to_string(),
        evidence: "obs:test:1: 用真实历史会话做回归验证".to_string(),
        confidence: Some(0.84),
        source: source.to_string(),
    }
}

#[test]
fn reference_prompt_files_exist() {
    assert!(std::path::Path::new("prompts/reference_synthesis.md").exists());
    assert!(std::path::Path::new("prompts/reference_quality_judge.md").exists());
    assert!(std::path::Path::new("prompts/pairwise_card_judge.md").exists());
}

#[test]
fn pairwise_judge_prompt_is_blind_to_system_names() {
    let prompt = format_pairwise_judge_prompt(
        &[reference_card()],
        &[eval_card("方案 A 候选", "pipeline")],
        &[eval_card("方案 B 候选", "baseline")],
    );

    assert!(!prompt.to_lowercase().contains("pipeline"));
    assert!(!prompt.to_lowercase().contains("baseline"));
    assert!(prompt.contains("Set A"));
    assert!(prompt.contains("Set B"));
}

#[test]
fn eval_command_is_available_from_cli_help() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_agent-kernel"))
        .arg("eval")
        .arg("--help")
        .output()
        .expect("run help");

    assert!(
        output.status.success(),
        "help failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Evaluate real local projects"));
    assert!(stdout.contains("--max-projects"));
    assert!(stdout.contains("--timeout-secs"));
}
