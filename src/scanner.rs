use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use regex::Regex;
use serde::Serialize;

use crate::config::{self, SkillIndex, SkillRecord};
use crate::fsutil;

#[derive(Debug, Serialize)]
pub struct ScanReport {
    pub project_root: String,
    pub rules: Vec<RuleFile>,
    pub skills: Vec<SkillRecord>,
}

#[derive(Debug, Serialize)]
pub struct RuleFile {
    pub kind: String,
    pub path: String,
    pub hash: String,
}

#[derive(Debug)]
pub struct ImportSummary {
    pub project_root: String,
    pub project_config: String,
    pub skill_index: String,
    pub rule_count: usize,
    pub skill_count: usize,
}

impl ScanReport {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Agent Memory Kernel Scan Report\n\n");
        out.push_str(&format!("Project: {}\n", self.project_root));
        out.push_str(&format!("Rules: {}\n", self.rules.len()));
        for rule in &self.rules {
            out.push_str(&format!("- {}: {}\n", rule.kind, rule.path));
        }
        out.push_str(&format!("\nSkills: {}\n", self.skills.len()));
        for skill in &self.skills {
            out.push_str(&format!(
                "- {} ({})\n  {}\n",
                skill.id, skill.source_kind, skill.source_path
            ));
        }
        out
    }
}

impl ImportSummary {
    pub fn render(&self) -> String {
        format!(
            "Agent Memory Kernel import complete\n\nProject: {}\nRules indexed: {}\nSkills indexed: {}\nConfig: {}\nSkill index: {}\n\nNext:\n- Run `agent-kernel ui`\n- Or add a mirror: `agent-kernel mirror --skill <id> --agent codex`\n- Preview build: `agent-kernel build --preview`\n",
            self.project_root,
            self.rule_count,
            self.skill_count,
            self.project_config,
            self.skill_index,
        )
    }
}

pub fn import_project(project_root: &Path, scan_home: bool) -> Result<ImportSummary> {
    let root = fsutil::normalize_project_root(project_root)?;
    let report = scan_project(&root, scan_home)?;

    config::ensure_kernel_dir(&root)?;
    let config_path = config::project_config_path(&root);
    if !config_path.exists() {
        let project_config = config::default_project_config(&root);
        config::save_project_config(&root, &project_config)?;
    }

    let index = SkillIndex {
        generated_at: Utc::now().to_rfc3339(),
        skills: report.skills.clone(),
    };
    config::save_skill_index(&root, &index)?;

    let imported_rules = serde_yaml::to_string(&report.rules)?;
    fs::write(
        config::kernel_dir(&root).join("imported-rules.yml"),
        imported_rules,
    )
    .context("write imported-rules.yml")?;

    Ok(ImportSummary {
        project_root: fsutil::path_to_slash(&root),
        project_config: fsutil::path_to_slash(&config_path),
        skill_index: fsutil::path_to_slash(&config::skill_index_path(&root)),
        rule_count: report.rules.len(),
        skill_count: report.skills.len(),
    })
}

pub fn scan_project(project_root: &Path, scan_home: bool) -> Result<ScanReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    let mut rules = scan_rule_files(&root)?;
    rules.sort_by(|a, b| a.path.cmp(&b.path));

    let mut skills = Vec::new();
    for skill_root in skill_roots(&root, scan_home) {
        if skill_root.exists() {
            skills.extend(scan_skill_root(&skill_root, &root)?);
        }
    }
    dedupe_skills(&mut skills);

    Ok(ScanReport {
        project_root: fsutil::path_to_slash(&root),
        rules,
        skills,
    })
}

fn scan_rule_files(root: &Path) -> Result<Vec<RuleFile>> {
    let mut rules = Vec::new();
    let candidates = [
        ("codex", root.join("AGENTS.md")),
        ("claude-code", root.join("CLAUDE.md")),
        ("cursor-legacy", root.join(".cursorrules")),
        ("cline", root.join(".clinerules")),
    ];

    for (kind, path) in candidates {
        if path.exists() {
            rules.push(RuleFile {
                kind: kind.to_string(),
                path: fsutil::path_to_slash(&path),
                hash: fsutil::sha256_file(&path)?,
            });
        }
    }

    let cursor_rules = root.join(".cursor").join("rules");
    if cursor_rules.exists() {
        for entry in walkdir::WalkDir::new(cursor_rules).follow_links(false) {
            let entry = entry?;
            if entry.file_type().is_file() {
                let path = entry.path();
                if matches!(
                    path.extension().and_then(|v| v.to_str()),
                    Some("md" | "mdc")
                ) {
                    rules.push(RuleFile {
                        kind: "cursor".to_string(),
                        path: fsutil::path_to_slash(path),
                        hash: fsutil::sha256_file(path)?,
                    });
                }
            }
        }
    }

    Ok(rules)
}

fn skill_roots(root: &Path, scan_home: bool) -> Vec<PathBuf> {
    let mut roots = vec![
        root.join(".claude").join("skills"),
        root.join(".agents").join("skills"),
        root.join(".github").join("skills"),
    ];

    if scan_home && let Some(home) = fsutil::home_dir() {
        roots.push(home.join(".agents").join("skills"));
        roots.push(home.join(".claude").join("skills"));
        roots.push(home.join(".codex").join("skills"));
        roots.push(home.join(".codex").join("superpowers").join("skills"));
        roots.push(home.join(".codex").join("plugins").join("cache"));
        roots.push(home.join(".copilot").join("skills"));
    }

    roots
}

fn scan_skill_root(skill_root: &Path, project_root: &Path) -> Result<Vec<SkillRecord>> {
    let mut records = Vec::new();
    for entry in walkdir::WalkDir::new(skill_root).follow_links(true) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.file_name() != "SKILL.md" {
            continue;
        }

        let skill_dir = entry
            .path()
            .parent()
            .context("SKILL.md should have a parent directory")?;
        if skill_dir.join(".agent-kernel-mirror.yml").exists() {
            continue;
        }
        let text = fs::read_to_string(entry.path())
            .with_context(|| format!("read {}", entry.path().display()))?;
        let (frontmatter, fallback_name) = parse_skill_frontmatter(&text, skill_dir);
        let name = frontmatter
            .get("name")
            .cloned()
            .unwrap_or_else(|| fallback_name.clone());
        let description = frontmatter.get("description").cloned().unwrap_or_default();
        let warnings = validate_skill(&frontmatter, &name, &description);
        let id = skill_id(skill_root, skill_dir, &name, project_root);
        let source_kind = source_kind(skill_dir, project_root);

        records.push(SkillRecord {
            id,
            name,
            description,
            source_path: fsutil::path_to_slash(skill_dir),
            source_kind,
            source_hash: fsutil::sha256_dir(skill_dir)?,
            warnings,
        });
    }
    Ok(records)
}

fn parse_skill_frontmatter(text: &str, skill_dir: &Path) -> (BTreeMap<String, String>, String) {
    let fallback = skill_dir
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("skill")
        .to_string();

    let mut map = BTreeMap::new();
    if !text.starts_with("---") {
        return (map, fallback);
    }
    let Some(end) = text[3..].find("\n---") else {
        return (map, fallback);
    };
    let yaml = &text[3..3 + end];
    if let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(yaml)
        && let Some(obj) = value.as_mapping()
    {
        for (key, value) in obj {
            if let (Some(key), Some(value)) = (key.as_str(), value.as_str()) {
                map.insert(key.to_string(), value.to_string());
            }
        }
    }
    (map, fallback)
}

fn validate_skill(
    frontmatter: &BTreeMap<String, String>,
    name: &str,
    description: &str,
) -> Vec<String> {
    let mut warnings = Vec::new();
    if !frontmatter.contains_key("name") {
        warnings.push("missing frontmatter field: name".to_string());
    }
    if !frontmatter.contains_key("description") {
        warnings.push("missing frontmatter field: description".to_string());
    }
    if name.trim().is_empty() {
        warnings.push("skill name is empty".to_string());
    }
    if description.trim().len() < 24 {
        warnings.push("description is short; agents may not dispatch it reliably".to_string());
    }
    warnings
}

fn skill_id(skill_root: &Path, skill_dir: &Path, name: &str, project_root: &Path) -> String {
    let rel = skill_dir.strip_prefix(skill_root).unwrap_or(skill_dir);
    let parts = rel
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .map(slug)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();

    if parts.len() >= 2 {
        return format!("{}:{}", parts[0], parts[parts.len() - 1]);
    }
    if skill_dir.starts_with(project_root) {
        format!("project:{}", slug(name))
    } else {
        format!("local:{}", slug(name))
    }
}

fn source_kind(skill_dir: &Path, project_root: &Path) -> String {
    let path = fsutil::path_to_slash(skill_dir).to_lowercase();
    if skill_dir.starts_with(project_root) {
        "project".to_string()
    } else if path.contains("/.codex/superpowers/")
        || path.contains("\\.codex\\superpowers\\")
        || path.contains("/.agents/skills/superpowers/")
        || path.contains("\\.agents\\skills\\superpowers\\")
    {
        "superpowers".to_string()
    } else if path.contains("/.codex/plugins/cache/") || path.contains("\\.codex\\plugins\\cache\\")
    {
        "plugin".to_string()
    } else {
        "referenced".to_string()
    }
}

fn slug(input: &str) -> String {
    let re = Regex::new(r"[^a-zA-Z0-9_-]+").expect("valid regex");
    let lowered = input.trim().to_lowercase();
    re.replace_all(&lowered, "-").trim_matches('-').to_string()
}

fn dedupe_skills(skills: &mut [SkillRecord]) {
    skills.sort_by(|a, b| a.id.cmp(&b.id).then(a.source_path.cmp(&b.source_path)));
    let mut seen = BTreeMap::<String, usize>::new();
    for skill in skills.iter_mut() {
        let count = seen.entry(skill.id.clone()).or_insert(0);
        if *count > 0 {
            skill.id = format!("{}-{}", skill.id, *count + 1);
        }
        *count += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parses_skill_frontmatter() {
        let temp = tempfile::tempdir().expect("tempdir");
        let skill_dir = temp.path().join("demo");
        fs::create_dir_all(&skill_dir).expect("skill dir");
        let text = "---\nname: demo\ndescription: Use when testing parser behavior\n---\n# Demo\n";

        let (frontmatter, fallback) = parse_skill_frontmatter(text, &skill_dir);

        assert_eq!(fallback, "demo");
        assert_eq!(frontmatter.get("name").map(String::as_str), Some("demo"));
        assert_eq!(
            frontmatter.get("description").map(String::as_str),
            Some("Use when testing parser behavior")
        );
    }

    #[test]
    fn warns_on_missing_description() {
        let mut frontmatter = BTreeMap::new();
        frontmatter.insert("name".to_string(), "thin".to_string());

        let warnings = validate_skill(&frontmatter, "thin", "");

        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("description"))
        );
    }
}
