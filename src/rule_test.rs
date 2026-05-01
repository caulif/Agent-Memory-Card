use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config;
use crate::fsutil;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleTest {
    pub name: String,
    pub target: String,
    #[serde(default)]
    pub artifact: Option<String>,
    #[serde(default)]
    pub expect: RuleExpect,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuleExpect {
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug)]
pub struct RuleTestReport {
    pub passed: usize,
    pub failed: usize,
    pub rows: Vec<RuleTestRow>,
}

#[derive(Debug)]
pub struct RuleTestRow {
    pub name: String,
    pub status: String,
    pub details: Vec<String>,
}

impl RuleTestReport {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Agent-Kernel Rule CI\n\n");
        for row in &self.rows {
            out.push_str(&format!("- {}: {}\n", row.name, row.status));
            for detail in &row.details {
                out.push_str(&format!("  - {detail}\n"));
            }
        }
        out.push_str(&format!(
            "\nSummary: {} passed, {} failed\n",
            self.passed, self.failed
        ));
        out
    }
}

pub fn run_rule_tests(project_root: &Path) -> Result<RuleTestReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    let tests = load_rule_tests(&root)?;
    let project = config::load_or_default_project_config(&root)?;
    let mut rows = Vec::new();
    let mut passed = 0;
    let mut failed = 0;

    for test in tests {
        let artifact = resolve_artifact(&root, &project, &test)?;
        let mut details = Vec::new();
        let text = fs::read_to_string(&artifact)
            .with_context(|| format!("read artifact {}", artifact.display()))?;

        for needle in &test.expect.include {
            if !text.contains(needle) {
                details.push(format!("missing required text: {needle}"));
            }
        }
        for needle in &test.expect.exclude {
            if text.contains(needle) {
                details.push(format!("found forbidden text: {needle}"));
            }
        }

        if details.is_empty() {
            passed += 1;
            rows.push(RuleTestRow {
                name: test.name,
                status: "pass".to_string(),
                details,
            });
        } else {
            failed += 1;
            rows.push(RuleTestRow {
                name: test.name,
                status: "fail".to_string(),
                details,
            });
        }
    }

    Ok(RuleTestReport {
        passed,
        failed,
        rows,
    })
}

fn load_rule_tests(project_root: &Path) -> Result<Vec<RuleTest>> {
    let dir = config::kernel_dir(project_root).join("tests");
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut tests = Vec::new();
    for entry in walkdir::WalkDir::new(dir).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|value| value.to_str()) != Some("yml") {
            continue;
        }
        let text = fs::read_to_string(entry.path())?;
        tests.push(serde_yaml::from_str::<RuleTest>(&text)?);
    }
    tests.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(tests)
}

fn resolve_artifact(
    project_root: &Path,
    project: &config::ProjectConfig,
    test: &RuleTest,
) -> Result<PathBuf> {
    if let Some(artifact) = &test.artifact {
        return Ok(project_root.join(artifact));
    }
    let Some(agent) = project.agents.get(&test.target) else {
        anyhow::bail!(
            "test `{}` targets unknown agent `{}`",
            test.name,
            test.target
        );
    };
    let Some(instructions) = &agent.exports.instructions else {
        anyhow::bail!(
            "test `{}` target `{}` has no instruction artifact",
            test.name,
            test.target
        );
    };
    Ok(project_root.join(instructions))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_rule_test_passes_and_fails() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(config::kernel_dir(temp.path()).join("tests")).expect("tests dir");
        fs::write(temp.path().join("AGENTS.md"), "Use Axios\nPrefer pnpm").expect("artifact");
        fs::write(
            config::kernel_dir(temp.path())
                .join("tests")
                .join("codex.yml"),
            "name: codex-rules\ntarget: codex\nexpect:\n  include:\n    - Axios\n  exclude:\n    - Fetch\n",
        )
        .expect("test file");

        let report = run_rule_tests(temp.path()).expect("run tests");

        assert_eq!(report.passed, 1);
        assert_eq!(report.failed, 0);
    }
}
