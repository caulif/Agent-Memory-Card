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

pub(super) fn parse_agent_candidates(output: &str) -> Result<Vec<AgentSkillletCandidate>> {
    let trimmed = output.trim();
    if let Ok(candidates) = serde_json::from_str::<Vec<AgentSkillletCandidate>>(trimmed) {
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

pub(super) fn is_usable_agent_candidate(candidate: &AgentSkillletCandidate) -> bool {
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
