use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Deserialize;

use crate::config;
use crate::fsutil;

use super::signals::split_sentences;
use super::{Candidate, draft_id, looks_like_rule};

#[derive(Debug, Clone, serde::Serialize)]
pub struct PreferenceTemplatePreview {
    pub title: String,
    pub body: String,
    pub source: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PreferenceRegistryValidationReport {
    pub errors: usize,
    pub warnings: usize,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PreferenceTestReport {
    pub matches: Vec<PreferenceTestMatch>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PreferenceTestMatch {
    pub draft_id: String,
    pub title: String,
    pub body: String,
    pub source: String,
    pub required: Vec<String>,
    pub context: Vec<String>,
}

impl PreferenceTestReport {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Agent-Kernel preference test\n\n");
        if self.matches.is_empty() {
            out.push_str("No preference templates matched.\n");
            return out;
        }
        out.push_str("Matches:\n");
        for item in &self.matches {
            out.push_str(&format!(
                "- {} [{}] {}: {}\n",
                item.draft_id, item.source, item.title, item.body
            ));
            out.push_str(&format!("  required: {}\n", item.required.join(", ")));
            out.push_str(&format!("  context: {}\n", item.context.join(", ")));
        }
        out
    }
}

impl PreferenceRegistryValidationReport {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Agent-Kernel preference registry validation\n\n");
        if self.messages.is_empty() {
            out.push_str("No issues found.\n");
        } else {
            for message in &self.messages {
                out.push_str(&format!("- {message}\n"));
            }
        }
        out.push_str(&format!(
            "\nSummary: {} errors, {} warnings\n",
            self.errors, self.warnings
        ));
        out
    }
}

pub fn preference_templates(project_root: &Path) -> Result<Vec<PreferenceTemplatePreview>> {
    Ok(load_known_preferences(project_root)?
        .into_iter()
        .map(|preference| PreferenceTemplatePreview {
            title: preference.title,
            body: preference.body,
            source: preference.source,
        })
        .collect())
}

pub fn init_preference_registry(project_root: &Path) -> Result<bool> {
    let root = fsutil::normalize_project_root(project_root)?;
    config::ensure_kernel_dir(&root)?;
    let path = preference_registry_path(&root);
    if path.exists() {
        return Ok(false);
    }
    fs::write(path, DEFAULT_PREFERENCE_REGISTRY)?;
    Ok(true)
}

pub fn validate_preference_registry(
    project_root: &Path,
) -> Result<PreferenceRegistryValidationReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    let path = preference_registry_path(&root);
    if !path.exists() {
        return Ok(PreferenceRegistryValidationReport {
            errors: 0,
            warnings: 1,
            messages: vec![
                "warning: .agent-kernel/preference-registry.yml does not exist".to_string(),
            ],
        });
    }

    let text = fs::read_to_string(&path)?;
    let registry: ProjectPreferenceRegistry = serde_yaml::from_str(&text)?;
    let mut report = PreferenceRegistryValidationReport {
        errors: 0,
        warnings: 0,
        messages: Vec::new(),
    };
    let mut seen_titles = BTreeSet::new();
    for (index, preference) in registry.preferences.iter().enumerate() {
        let number = index + 1;
        let title = preference.title.trim();
        if title.is_empty() {
            report.errors += 1;
            report
                .messages
                .push(format!("error: preference #{number} has an empty title"));
        } else if !seen_titles.insert(title.to_lowercase()) {
            report.errors += 1;
            report.messages.push(format!(
                "error: preference #{number} has duplicate title `{title}`"
            ));
        }
        if preference.body.trim().is_empty() {
            report.errors += 1;
            report
                .messages
                .push(format!("error: preference #{number} has an empty body"));
        }
        if preference.required.is_empty() {
            report.errors += 1;
            report.messages.push(format!(
                "error: preference #{number} has no required markers"
            ));
        }
        if preference.context.is_empty() {
            report.warnings += 1;
            report.messages.push(format!(
                "warning: preference #{number} has no context markers and may match too broadly"
            ));
        }
    }
    Ok(report)
}

pub fn test_preference_text(project_root: &Path, text: &str) -> Result<PreferenceTestReport> {
    let preferences = load_known_preferences(project_root)?;
    let mut matches = Vec::new();
    for sentence in split_sentences(text) {
        if !looks_like_rule(sentence) {
            continue;
        }
        let lower = sentence.to_lowercase();
        for preference in &preferences {
            if let Some(reason) = preference.match_reason(&lower) {
                let candidate = Candidate {
                    title: preference.title.clone(),
                    body: preference.body.clone(),
                    kind: "preference".to_string(),
                    scope: "project".to_string(),
                    evidence: sentence.to_string(),
                    confidence: Some(0.92),
                    reason: Some(format_match_reason(&reason)),
                    matched_template: Some(format!("{}:{}", preference.source, preference.title)),
                };
                matches.push(PreferenceTestMatch {
                    draft_id: draft_id(&candidate),
                    title: preference.title.clone(),
                    body: preference.body.clone(),
                    source: preference.source.clone(),
                    required: reason.required,
                    context: reason.context,
                });
                break;
            }
        }
    }
    Ok(PreferenceTestReport { matches })
}

pub(super) fn normalize_known_preference(
    sentence: &str,
    preferences: &[KnownPreference],
) -> Option<Candidate> {
    let lower = sentence.to_lowercase();
    preferences.iter().find_map(|preference| {
        preference.match_reason(&lower).map(|reason| Candidate {
            title: preference.title.clone(),
            body: preference.body.clone(),
            kind: "preference".to_string(),
            scope: "project".to_string(),
            evidence: sentence.to_string(),
            confidence: Some(0.92),
            reason: Some(format_match_reason(&reason)),
            matched_template: Some(format!("{}:{}", preference.source, preference.title)),
        })
    })
}

pub(super) struct KnownPreference {
    title: String,
    body: String,
    source: String,
    required: Vec<String>,
    context: Vec<String>,
}

impl KnownPreference {
    fn match_reason(&self, lower: &str) -> Option<KnownPreferenceMatchReason> {
        let required = self
            .required
            .iter()
            .filter(|marker| lower.contains(marker.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if required.len() != self.required.len() {
            return None;
        }
        let context = self
            .context
            .iter()
            .filter(|marker| lower.contains(marker.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if !self.context.is_empty() && context.is_empty() {
            return None;
        }
        Some(KnownPreferenceMatchReason { required, context })
    }
}

struct KnownPreferenceMatchReason {
    required: Vec<String>,
    context: Vec<String>,
}

fn format_match_reason(reason: &KnownPreferenceMatchReason) -> String {
    let required = if reason.required.is_empty() {
        "none".to_string()
    } else {
        reason.required.join(", ")
    };
    let context = if reason.context.is_empty() {
        "none".to_string()
    } else {
        reason.context.join(", ")
    };
    format!("Matched preference template; required: {required}; context: {context}")
}

#[derive(Debug, Default, Deserialize)]
struct ProjectPreferenceRegistry {
    #[serde(default)]
    preferences: Vec<ProjectPreference>,
}

#[derive(Debug, Deserialize)]
struct ProjectPreference {
    #[serde(default)]
    title: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    required: Vec<String>,
    #[serde(default)]
    context: Vec<String>,
}

pub(super) fn load_known_preferences(project_root: &Path) -> Result<Vec<KnownPreference>> {
    let mut preferences = load_project_preferences(project_root)?;
    preferences.extend(built_in_preferences());
    Ok(preferences)
}

fn load_project_preferences(project_root: &Path) -> Result<Vec<KnownPreference>> {
    let root = fsutil::normalize_project_root(project_root)?;
    let path = preference_registry_path(&root);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(&path)?;
    let registry: ProjectPreferenceRegistry = serde_yaml::from_str(&text)?;
    Ok(registry
        .preferences
        .into_iter()
        .filter(|preference| {
            !preference.title.trim().is_empty() && !preference.body.trim().is_empty()
        })
        .map(|preference| KnownPreference {
            title: preference.title,
            body: preference.body,
            source: "project".to_string(),
            required: preference
                .required
                .into_iter()
                .map(|marker| marker.to_lowercase())
                .collect(),
            context: preference
                .context
                .into_iter()
                .map(|marker| marker.to_lowercase())
                .collect(),
        })
        .collect())
}

fn preference_registry_path(project_root: &Path) -> PathBuf {
    config::kernel_dir(project_root).join("preference-registry.yml")
}

const DEFAULT_PREFERENCE_REGISTRY: &str = r#"preferences:
  - title: Use Playwright
    body: Use Playwright for browser automation tests.
    required:
      - playwright
    context:
      - cypress
      - browser automation
      - 浏览器自动化
"#;

pub(super) fn built_in_preferences() -> Vec<KnownPreference> {
    vec![
        KnownPreference {
            title: "Prefer Bun".to_string(),
            body: "Use Bun for JavaScript package management and scripts.".to_string(),
            source: "built-in".to_string(),
            required: strings(&["bun"]),
            context: strings(&[
                "npm",
                "pnpm",
                "yarn",
                "bun run",
                "bunx",
                "包管理",
                "package manager",
                "package management",
                "javascript package",
                "js 脚本",
            ]),
        },
        KnownPreference {
            title: "Use Axios".to_string(),
            body: "Use Axios for frontend HTTP requests.".to_string(),
            source: "built-in".to_string(),
            required: strings(&["axios"]),
            context: strings(&[
                "fetch",
                "http",
                "request",
                "requests",
                "api",
                "前端请求",
                "请求",
                "接口",
            ]),
        },
        KnownPreference {
            title: "Use Vitest".to_string(),
            body: "Use Vitest for frontend unit tests.".to_string(),
            source: "built-in".to_string(),
            required: strings(&["vitest"]),
            context: strings(&[
                "jest",
                "unit test",
                "unit tests",
                "frontend test",
                "frontend tests",
                "单元测试",
                "测试",
            ]),
        },
    ]
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}
