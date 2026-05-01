use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use regex::Regex;
use serde::Deserialize;

use crate::config;
use crate::draft::{self, NewDraft};
use crate::fsutil;
use crate::provider;

#[derive(Debug)]
pub struct ExtractReport {
    pub created: Vec<String>,
    pub skipped: Vec<String>,
    pub candidates: Vec<ExtractCandidatePreview>,
    pub dry_run: bool,
    pub provider: String,
    pub redacted: bool,
}

impl ExtractReport {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Agent-Kernel extract report\n\n");
        out.push_str(&format!("Provider: {}\n\n", self.provider));
        if self.redacted {
            out.push_str("Secrets: redacted\n\n");
        }
        if self.created.is_empty() {
            if self.dry_run && !self.candidates.is_empty() {
                out.push_str("Draft candidates:\n");
                for candidate in &self.candidates {
                    out.push_str(&format!("- {}: {}\n", candidate.id, candidate.body));
                }
            } else {
                out.push_str("No drafts created.\n");
            }
        } else {
            out.push_str("Drafts created:\n");
            for id in &self.created {
                out.push_str(&format!("- {id}\n"));
            }
        }
        if !self.skipped.is_empty() {
            out.push_str("\nSkipped:\n");
            for item in &self.skipped {
                out.push_str(&format!("- {item}\n"));
            }
        }
        out
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExtractCandidatePreview {
    pub id: String,
    pub title: String,
    pub body: String,
    pub kind: String,
    pub scope: String,
    pub evidence: String,
}

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

#[derive(Debug, Clone)]
struct Candidate {
    title: String,
    body: String,
    kind: String,
    scope: String,
    evidence: String,
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

pub fn extract_to_drafts(
    project_root: &Path,
    text: Option<String>,
    file: Option<PathBuf>,
    targets: Vec<String>,
    provider_name: Option<String>,
    dry_run: bool,
) -> Result<ExtractReport> {
    let (input, source) = match (text, file) {
        (Some(text), None) => (text, "inline text".to_string()),
        (None, Some(path)) => (
            fs::read_to_string(&path)?,
            format!("file:{}", path.to_string_lossy()),
        ),
        _ => return Err(anyhow!("provide exactly one of --text or --file")),
    };

    extract_text_to_drafts(
        project_root,
        &input,
        targets,
        &source,
        provider_name,
        dry_run,
    )
}

pub fn extract_text_to_drafts(
    project_root: &Path,
    input: &str,
    targets: Vec<String>,
    source: &str,
    provider_name: Option<String>,
    dry_run: bool,
) -> Result<ExtractReport> {
    let provider_name = provider_name.unwrap_or_else(|| {
        provider::load_or_default_provider_config(project_root)
            .map(|cfg| cfg.default)
            .unwrap_or_else(|_| "local".to_string())
    });
    if provider_name != "local" && provider::provider_exists(project_root, &provider_name)? {
        return Ok(ExtractReport {
            created: Vec::new(),
            skipped: vec![format!(
                "provider `{provider_name}` is configured but not implemented yet; use `--provider local`"
            )],
            candidates: Vec::new(),
            dry_run,
            provider: provider_name,
            redacted: false,
        });
    }
    if provider_name != "local" {
        return Err(anyhow!("unknown provider `{provider_name}`"));
    }

    let provider_cfg = provider::load_or_default_provider_config(project_root)?;
    let redacted_input = if provider_cfg.privacy.redact_secrets {
        provider::redact_secrets(input)
    } else {
        input.to_string()
    };
    let redacted = redacted_input != input;
    let preferences = load_known_preferences(project_root)?;
    let candidates = extract_candidates_with_preferences(&redacted_input, &preferences);
    let previews = candidates
        .iter()
        .map(|candidate| ExtractCandidatePreview {
            id: draft_id(candidate),
            title: candidate.title.clone(),
            body: candidate.body.clone(),
            kind: candidate.kind.clone(),
            scope: candidate.scope.clone(),
            evidence: format!("{source}: {}", candidate.evidence),
        })
        .collect::<Vec<_>>();
    if dry_run {
        return Ok(ExtractReport {
            created: Vec::new(),
            skipped: Vec::new(),
            candidates: previews,
            dry_run,
            provider: provider_name,
            redacted,
        });
    }

    let mut created = Vec::new();
    let mut skipped = Vec::new();
    for candidate in candidates {
        let id = draft_id(&candidate);
        let result = draft::add_draft(
            project_root,
            NewDraft {
                id: id.clone(),
                title: candidate.title,
                kind: candidate.kind,
                scope: candidate.scope,
                body: candidate.body,
                targets: targets.clone(),
                evidence: format!("{source}: {}", candidate.evidence),
            },
        );
        match result {
            Ok(()) => created.push(id),
            Err(error) => skipped.push(format!("{id}: {error}")),
        }
    }
    Ok(ExtractReport {
        created,
        skipped,
        candidates: previews,
        dry_run,
        provider: provider_name,
        redacted,
    })
}

#[cfg(test)]
fn extract_candidates(input: &str) -> Vec<Candidate> {
    let preferences = built_in_preferences();
    extract_candidates_with_preferences(input, &preferences)
}

fn extract_candidates_with_preferences(
    input: &str,
    preferences: &[KnownPreference],
) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    for sentence in split_sentences(input) {
        if !looks_like_rule(sentence) {
            continue;
        }
        if let Some(candidate) = normalize_known_preference(sentence, preferences) {
            candidates.push(candidate);
            continue;
        }
        let body = normalize_body(sentence);
        if body.len() < 12 {
            continue;
        }
        let title = title_from_body(&body);
        candidates.push(Candidate {
            title,
            body,
            kind: classify_kind(sentence).to_string(),
            scope: "project".to_string(),
            evidence: sentence.to_string(),
        });
    }
    dedupe_candidates(candidates)
}

fn normalize_known_preference(
    sentence: &str,
    preferences: &[KnownPreference],
) -> Option<Candidate> {
    let lower = sentence.to_lowercase();
    preferences
        .iter()
        .find(|preference| preference.matches(&lower))
        .map(|preference| Candidate {
            title: preference.title.clone(),
            body: preference.body.clone(),
            kind: "preference".to_string(),
            scope: "project".to_string(),
            evidence: sentence.to_string(),
        })
}

struct KnownPreference {
    title: String,
    body: String,
    source: String,
    required: Vec<String>,
    context: Vec<String>,
}

impl KnownPreference {
    fn matches(&self, lower: &str) -> bool {
        self.required.iter().all(|marker| lower.contains(marker))
            && (self.context.is_empty() || self.context.iter().any(|marker| lower.contains(marker)))
    }
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

fn load_known_preferences(project_root: &Path) -> Result<Vec<KnownPreference>> {
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

fn built_in_preferences() -> Vec<KnownPreference> {
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

fn split_sentences(input: &str) -> Vec<&str> {
    input
        .split(['\n', '。', '！', '？', '.', '!', '?'])
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect()
}

fn looks_like_rule(sentence: &str) -> bool {
    let lower = sentence.to_lowercase();
    let markers = [
        "以后",
        "必须",
        "不要",
        "禁止",
        "统一",
        "默认",
        "优先",
        "改用",
        "不再",
        "always",
        "never",
        "must",
        "prefer",
        "use ",
        "don't",
        "do not",
        "default to",
    ];
    markers.iter().any(|marker| lower.contains(marker))
}

fn classify_kind(sentence: &str) -> &'static str {
    let lower = sentence.to_lowercase();
    if lower.contains("不要")
        || lower.contains("禁止")
        || lower.contains("never")
        || lower.contains("do not")
        || lower.contains("don't")
    {
        "constraint"
    } else {
        "preference"
    }
}

fn normalize_body(sentence: &str) -> String {
    let mut body = sentence.trim().to_string();
    let replacements = [
        ("我再说最后一次，", ""),
        ("我再说最后一次", ""),
        ("以后", ""),
        ("请", ""),
        ("记住：", ""),
        ("记住:", ""),
    ];
    for (from, to) in replacements {
        body = body.replace(from, to);
    }
    body = body.trim_matches(['，', ',', ' ']).trim().to_string();
    if body.ends_with(';') {
        body.pop();
    }
    body
}

fn title_from_body(body: &str) -> String {
    let words = body.split_whitespace().collect::<Vec<_>>();
    if words.len() >= 3 {
        return words.iter().take(6).copied().collect::<Vec<_>>().join(" ");
    }
    body.chars().take(24).collect()
}

fn draft_id(candidate: &Candidate) -> String {
    let slug = slug(&candidate.title);
    format!("project:{slug}")
}

fn slug(input: &str) -> String {
    let re = Regex::new(r"[^a-zA-Z0-9\u4e00-\u9fff]+").expect("valid regex");
    let slug = re
        .replace_all(&input.to_lowercase(), "-")
        .trim_matches('-')
        .to_string();
    if slug.is_empty() {
        "draft".to_string()
    } else {
        slug.chars().take(48).collect()
    }
}

fn dedupe_candidates(candidates: Vec<Candidate>) -> Vec<Candidate> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for candidate in candidates {
        let key = candidate.body.to_lowercase();
        if seen.insert(key) {
            out.push(candidate);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_chinese_rule_candidate() {
        let candidates = extract_candidates("以后前端请求统一使用 Axios，不要再用 Fetch。");

        assert_eq!(candidates.len(), 1);
        assert_eq!(draft_id(&candidates[0]), "project:use-axios");
        assert_eq!(candidates[0].title, "Use Axios");
        assert_eq!(candidates[0].body, "Use Axios for frontend HTTP requests.");
    }

    #[test]
    fn extract_command_creates_draft() {
        let temp = tempfile::tempdir().expect("tempdir");

        let report = extract_to_drafts(
            temp.path(),
            Some("Always use Bun for JavaScript package management and scripts.".to_string()),
            None,
            vec!["codex".to_string()],
            Some("local".to_string()),
            false,
        )
        .expect("extract");

        assert_eq!(report.created.len(), 1);
        let drafts = draft::load_drafts(temp.path()).expect("drafts");
        assert_eq!(drafts[0].targets, vec!["codex"]);
    }

    #[test]
    fn dry_run_does_not_create_drafts() {
        let temp = tempfile::tempdir().expect("tempdir");

        let report = extract_to_drafts(
            temp.path(),
            Some("Always use Bun for JavaScript package management and scripts.".to_string()),
            None,
            vec!["codex".to_string()],
            Some("local".to_string()),
            true,
        )
        .expect("extract");

        assert_eq!(report.candidates.len(), 1);
        assert!(draft::load_drafts(temp.path()).expect("drafts").is_empty());
    }

    #[test]
    fn normalizes_bun_package_manager_preference() {
        let temp = tempfile::tempdir().expect("tempdir");

        let report = extract_to_drafts(
            temp.path(),
            Some("以后把 npm 改为 Bun，所有 JS 脚本都用 bun run。".to_string()),
            None,
            vec!["codex".to_string()],
            Some("local".to_string()),
            true,
        )
        .expect("extract");

        assert_eq!(report.candidates.len(), 1);
        assert_eq!(report.candidates[0].id, "project:prefer-bun");
        assert_eq!(report.candidates[0].title, "Prefer Bun");
        assert_eq!(
            report.candidates[0].body,
            "Use Bun for JavaScript package management and scripts."
        );
    }

    #[test]
    fn normalizes_vitest_unit_test_preference() {
        let temp = tempfile::tempdir().expect("tempdir");

        let report = extract_to_drafts(
            temp.path(),
            Some("以后前端单元测试默认使用 Vitest，不要再写 Jest 配置。".to_string()),
            None,
            vec!["codex".to_string()],
            Some("local".to_string()),
            true,
        )
        .expect("extract");

        assert_eq!(report.candidates.len(), 1);
        assert_eq!(report.candidates[0].id, "project:use-vitest");
        assert_eq!(report.candidates[0].title, "Use Vitest");
        assert_eq!(
            report.candidates[0].body,
            "Use Vitest for frontend unit tests."
        );
    }

    #[test]
    fn uses_project_preference_registry() {
        let temp = tempfile::tempdir().expect("tempdir");
        let registry_dir = temp.path().join(".agent-kernel");
        fs::create_dir_all(&registry_dir).expect("registry dir");
        fs::write(
            registry_dir.join("preference-registry.yml"),
            r#"preferences:
  - title: Use Playwright
    body: Use Playwright for browser automation tests.
    required:
      - playwright
    context:
      - cypress
      - browser automation
      - 浏览器自动化
"#,
        )
        .expect("write registry");

        let report = extract_to_drafts(
            temp.path(),
            Some("以后浏览器自动化测试统一使用 Playwright，不要再用 Cypress。".to_string()),
            None,
            vec!["codex".to_string()],
            Some("local".to_string()),
            true,
        )
        .expect("extract");

        assert_eq!(report.candidates.len(), 1);
        assert_eq!(report.candidates[0].id, "project:use-playwright");
        assert_eq!(report.candidates[0].title, "Use Playwright");
        assert_eq!(
            report.candidates[0].body,
            "Use Playwright for browser automation tests."
        );
    }

    #[test]
    fn project_preference_registry_overrides_built_ins() {
        let temp = tempfile::tempdir().expect("tempdir");
        let registry_dir = temp.path().join(".agent-kernel");
        fs::create_dir_all(&registry_dir).expect("registry dir");
        fs::write(
            registry_dir.join("preference-registry.yml"),
            r#"preferences:
  - title: Use Bun Runtime
    body: Use Bun for package management, scripts, and JavaScript runtime tasks.
    required:
      - bun
    context:
      - npm
      - package management
"#,
        )
        .expect("write registry");

        let report = extract_to_drafts(
            temp.path(),
            Some("Always use Bun instead of npm for package management.".to_string()),
            None,
            vec!["codex".to_string()],
            Some("local".to_string()),
            true,
        )
        .expect("extract");

        assert_eq!(report.candidates.len(), 1);
        assert_eq!(report.candidates[0].id, "project:use-bun-runtime");
        assert_eq!(report.candidates[0].title, "Use Bun Runtime");
        assert_eq!(
            report.candidates[0].body,
            "Use Bun for package management, scripts, and JavaScript runtime tasks."
        );
    }

    #[test]
    fn preference_templates_list_project_before_built_ins() {
        let temp = tempfile::tempdir().expect("tempdir");
        let registry_dir = temp.path().join(".agent-kernel");
        fs::create_dir_all(&registry_dir).expect("registry dir");
        fs::write(
            registry_dir.join("preference-registry.yml"),
            r#"preferences:
  - title: Use Playwright
    body: Use Playwright for browser automation tests.
    required:
      - playwright
    context:
      - cypress
"#,
        )
        .expect("write registry");

        let templates = preference_templates(temp.path()).expect("templates");

        assert_eq!(templates[0].source, "project");
        assert_eq!(templates[0].title, "Use Playwright");
        assert!(
            templates
                .iter()
                .any(|template| template.source == "built-in")
        );
    }

    #[test]
    fn init_preference_registry_writes_example() {
        let temp = tempfile::tempdir().expect("tempdir");

        let created = init_preference_registry(temp.path()).expect("init registry");
        let path = config::kernel_dir(temp.path()).join("preference-registry.yml");

        assert!(created);
        assert!(path.exists());
        let text = fs::read_to_string(path).expect("registry");
        assert!(text.contains("Use Playwright"));
        assert!(text.contains("preferences:"));
    }

    #[test]
    fn validate_preference_registry_reports_errors_and_warnings() {
        let temp = tempfile::tempdir().expect("tempdir");
        let registry_dir = temp.path().join(".agent-kernel");
        fs::create_dir_all(&registry_dir).expect("registry dir");
        fs::write(
            registry_dir.join("preference-registry.yml"),
            r#"preferences:
  - title: Use Playwright
    body: Use Playwright for browser automation tests.
    required: []
    context: []
  - title: Use Playwright
    body: Duplicate title.
    required:
      - playwright
    context:
      - cypress
  - title: ""
    body: Missing title.
    required:
      - vitest
"#,
        )
        .expect("write registry");

        let report = validate_preference_registry(temp.path()).expect("validate");

        assert_eq!(report.errors, 3);
        assert_eq!(report.warnings, 2);
        assert!(
            report
                .messages
                .iter()
                .any(|message| message.contains("duplicate title"))
        );
    }

    #[test]
    fn extraction_redacts_secret_evidence() {
        let temp = tempfile::tempdir().expect("tempdir");

        extract_to_drafts(
            temp.path(),
            Some("Always use token=supersecret123456789 before pushing.".to_string()),
            None,
            vec!["codex".to_string()],
            Some("local".to_string()),
            false,
        )
        .expect("extract");

        let drafts = draft::load_drafts(temp.path()).expect("drafts");
        assert!(drafts[0].evidence.contains("[REDACTED]"));
        assert!(!drafts[0].evidence.contains("supersecret123456789"));
    }
}
