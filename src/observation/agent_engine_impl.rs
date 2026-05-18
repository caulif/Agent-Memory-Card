use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use super::*;

pub(super) fn run_agent_engine(engine: &str, prompt: &str, timeout: Duration) -> Result<String> {
    let mut command = match engine {
        "claude-code" => {
            let mut command = Command::new("claude");
            command.args([
                "-p",
                "--output-format",
                "text",
                "--permission-mode",
                "dontAsk",
                "--max-budget-usd",
                "0.25",
            ]);
            command
        }
        "codex" => {
            let mut command = Command::new(codex_binary());
            command.args([
                "exec",
                "--skip-git-repo-check",
                "--sandbox",
                "read-only",
                "-",
            ]);
            command
        }
        _ => anyhow::bail!("unsupported synthesis engine `{engine}`"),
    };
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().with_context(|| format!("spawn {engine}"))?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(prompt.as_bytes())?;
    }
    drop(child.stdin.take());

    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            let output = child.wait_with_output()?;
            if !output.status.success() {
                anyhow::bail!(
                    "{} exited with {}: {}",
                    engine,
                    output.status,
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }
        if started.elapsed() > timeout {
            let kill_error = child.kill().err();
            let wait_error = child.wait().err();
            anyhow::bail!(
                "{engine} synthesis timed out after {}s; kill_error={kill_error:?}; wait_error={wait_error:?}",
                timeout.as_secs()
            );
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[cfg(windows)]
fn codex_binary() -> &'static str {
    "codex.cmd"
}

#[cfg(not(windows))]
fn codex_binary() -> &'static str {
    "codex"
}

pub(super) fn parse_agent_candidates(output: &str) -> Result<Vec<AgentMemoryCardCandidate>> {
    let trimmed = output.trim();
    if let Ok(candidates) = serde_json::from_str::<Vec<AgentMemoryCardCandidate>>(trimmed) {
        return Ok(candidates);
    }
    let Some(start) = trimmed.find('[') else {
        anyhow::bail!("agent output did not contain a JSON array");
    };
    let Some(end) = trimmed.rfind(']') else {
        anyhow::bail!("agent output did not contain a complete JSON array");
    };
    serde_json::from_str(&trimmed[start..=end]).context("parse agent JSON candidates")
}

pub(super) fn is_usable_agent_candidate(candidate: &AgentMemoryCardCandidate) -> bool {
    let confidence = candidate.confidence.unwrap_or(0.0);
    !candidate.title.trim().is_empty()
        && !candidate.body.trim().is_empty()
        && candidate.body.len() <= 320
        && confidence >= 0.78
}

pub(super) fn normalize_candidate_kind(kind: &str) -> String {
    match kind {
        "preference" | "constraint" | "procedure" | "convention" | "correction"
        | "anti-pattern" => kind.to_string(),
        _ => "procedure".to_string(),
    }
}

pub(super) fn normalize_candidate_scope(scope: &str) -> String {
    match scope {
        "global" | "project" | "agent" => scope.to_string(),
        _ => "project".to_string(),
    }
}

pub(super) fn default_candidate_kind() -> String {
    "procedure".to_string()
}

pub(super) fn default_candidate_scope() -> String {
    "project".to_string()
}

/// 给提取 agent 用的合成 prompt。要求模型从 `--- obs:<id> ---` 分段中
/// 同时返回 evidence_quote 与 source_observation_ids，让产物可回溯到原文。
pub(super) fn agent_synthesis_prompt(material: &str) -> String {
    format!(
        r#"You are helping build Agent Memory Kernel, a local MemoryCard evolution engine.

Extract only durable, high-value agent skills from the local conversation material.

Definition:
- A Skill is an agent capability package: triggerable, reusable, procedural, and useful across future tasks.
- A MemoryCard is lighter: one stable preference, constraint, convention, workflow, correction, project improvement, root-cause learning, architecture decision, or supplement that can be compiled into Claude Code / Codex instructions or attached to a Skill.
- Keep only items that would still improve future work after the current bug or feature request is finished.
- Prefer a balanced set: project rules, cross-project principles, and collaboration preferences.
- Use scope="global" for cross-project principles and durable collaboration preferences; use scope="project" only when the rule depends on this project.
- Keep durable product quality constraints when they state a reusable acceptance bar, for example "outside model reasoning, interactions should not feel stuck or janky".

Reject:
- one-off requests like "continue", "fix this", "optimize UI", "how do I start"
- stack traces, terminal output, base instructions, system/developer prompts
- vague project brainstorming, PRD sections, fixture/gold-set requirements, or priority outlines without a durable future behavior
- unresolved product requests or bug reports such as "add a progress window", "UI is ugly", "tell me why it is stuck"; do not reject a product quality constraint when it includes a durable standard
- secrets, credentials, personal sensitive content
- raw error logs unless they include the reusable cause and fix

Material format:
- The material below is split into segments. Each segment starts with `--- obs:<id> ---` and contains text from one observation. Use these IDs as anchors for citation.

Return only a JSON array, no markdown. Max 12 items.
Each item must contain ALL of: title, body, brief, tags, language, kind, scope, confidence, reason, evidence_quote, source_observation_ids.
- evidence_quote: a verbatim short quote (8-200 chars) copied from the material that supports this item. If you cannot find a verbatim quote, REJECT the item rather than fabricate one.
- source_observation_ids: an array (1 or more) of obs IDs whose `--- obs:<id> ---` segments contributed the evidence. Empty array is forbidden; if no segment cleanly supports the item, drop it.
Allowed kind: preference, constraint, procedure, convention, correction, anti-pattern.
Allowed scope: global, project, agent.
Brief must be a concise Simplified Chinese explanation of what the candidate is for.
Tags must be compact and content-specific, for example axios, bun, frontend, http, js, structured-data, parser, 通用范式, agent-behavior, tool-use, meta-instruction.
Use confidence >= 0.78 only. Body must be concise, general, imperative, and reusable.

Recall targets:
- cross-project: core functionality first, user perspective/experience, planning before edits, real-history validation.
- collaboration: small-change fast tests / large-change broad tests, preserve human review boundaries, prefer candidate quality over quantity.
- product quality constraint: keep stable acceptance bars such as non-reasoning UI operations should remain smooth.

Material:
{material}"#
    )
}
