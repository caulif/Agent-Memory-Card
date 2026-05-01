use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::fsutil;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub version: u32,
    pub project: ProjectMeta,
    pub agents: BTreeMap<String, AgentConfig>,
    #[serde(default)]
    pub rules: RuleSelection,
    #[serde(default)]
    pub skilllets: SkillletSelection,
    #[serde(default)]
    pub skills: SkillSelection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub name: String,
    pub root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub enabled: bool,
    pub exports: AgentExports,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentExports {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules_dir: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuleSelection {
    #[serde(default)]
    pub include: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillletSelection {
    #[serde(default)]
    pub include: Vec<SkillletRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillletRef {
    pub id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillSelection {
    #[serde(default)]
    pub mirrors: Vec<MirrorDecl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorDecl {
    #[serde(rename = "ref")]
    pub reference: String,
    pub targets: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillIndex {
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub skills: Vec<SkillRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRecord {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source_path: String,
    pub source_kind: String,
    pub source_hash: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectLock {
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub mirrors: Vec<MirrorState>,
    #[serde(default)]
    pub artifacts: Vec<ArtifactState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorState {
    pub source: String,
    pub target: String,
    pub agent: String,
    pub source_hash: String,
    pub target_hash: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactState {
    pub path: String,
    pub hash: String,
    pub kind: String,
}

pub fn kernel_dir(project_root: &Path) -> PathBuf {
    project_root.join(".agent-kernel")
}

pub fn project_config_path(project_root: &Path) -> PathBuf {
    kernel_dir(project_root).join("project.yml")
}

pub fn skill_index_path(project_root: &Path) -> PathBuf {
    kernel_dir(project_root).join("skill-index.yml")
}

pub fn lock_path(project_root: &Path) -> PathBuf {
    kernel_dir(project_root).join("project.lock.yml")
}

pub fn ensure_kernel_dir(project_root: &Path) -> Result<()> {
    fs::create_dir_all(kernel_dir(project_root)).context("create .agent-kernel directory")
}

pub fn default_project_config(project_root: &Path) -> ProjectConfig {
    let name = project_root
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("project")
        .to_string();

    let mut agents = BTreeMap::new();
    agents.insert(
        "codex".to_string(),
        AgentConfig {
            enabled: true,
            exports: AgentExports {
                instructions: Some("AGENTS.md".to_string()),
                skills_dir: Some(".agents/skills".to_string()),
                rules_dir: None,
            },
        },
    );
    agents.insert(
        "claude-code".to_string(),
        AgentConfig {
            enabled: true,
            exports: AgentExports {
                instructions: Some("CLAUDE.md".to_string()),
                skills_dir: Some(".claude/skills".to_string()),
                rules_dir: None,
            },
        },
    );
    agents.insert(
        "cursor".to_string(),
        AgentConfig {
            enabled: true,
            exports: AgentExports {
                instructions: None,
                skills_dir: None,
                rules_dir: Some(".cursor/rules".to_string()),
            },
        },
    );
    agents.insert(
        "cline".to_string(),
        AgentConfig {
            enabled: false,
            exports: AgentExports {
                instructions: None,
                skills_dir: None,
                rules_dir: Some(".clinerules".to_string()),
            },
        },
    );

    ProjectConfig {
        version: 1,
        project: ProjectMeta {
            name,
            root: ".".to_string(),
        },
        agents,
        rules: RuleSelection::default(),
        skilllets: SkillletSelection::default(),
        skills: SkillSelection::default(),
    }
}

pub fn load_or_default_project_config(project_root: &Path) -> Result<ProjectConfig> {
    let path = project_config_path(project_root);
    let config = if path.exists() {
        let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        serde_yaml::from_str(&text).with_context(|| format!("parse {}", path.display()))?
    } else {
        default_project_config(project_root)
    };
    Ok(migrate_project_config(config))
}

fn migrate_project_config(mut config: ProjectConfig) -> ProjectConfig {
    if let Some(cline) = config.agents.get_mut("cline")
        && cline.exports.instructions.as_deref() == Some(".clinerules")
        && cline.exports.rules_dir.is_none()
    {
        cline.exports.instructions = None;
        cline.exports.rules_dir = Some(".clinerules".to_string());
    }
    config
}

pub fn save_project_config(project_root: &Path, config: &ProjectConfig) -> Result<()> {
    ensure_kernel_dir(project_root)?;
    let text = serde_yaml::to_string(config)?;
    fs::write(project_config_path(project_root), text).context("write project.yml")
}

pub fn load_skill_index(project_root: &Path) -> Result<SkillIndex> {
    let path = skill_index_path(project_root);
    if !path.exists() {
        return Ok(SkillIndex::default());
    }
    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    serde_yaml::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

pub fn save_skill_index(project_root: &Path, index: &SkillIndex) -> Result<()> {
    ensure_kernel_dir(project_root)?;
    let text = serde_yaml::to_string(index)?;
    fs::write(skill_index_path(project_root), text).context("write skill-index.yml")
}

pub fn save_lock(project_root: &Path, lock: &ProjectLock) -> Result<()> {
    ensure_kernel_dir(project_root)?;
    let text = serde_yaml::to_string(lock)?;
    fs::write(lock_path(project_root), text).context("write project.lock.yml")
}

pub fn add_mirror(project_root: &Path, skill_id: &str, agent: &str) -> Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    let index = load_skill_index(&root)?;
    if !index.skills.iter().any(|skill| skill.id == skill_id) {
        return Err(anyhow!(
            "skill `{skill_id}` was not found in .agent-kernel/skill-index.yml"
        ));
    }

    let mut config = load_or_default_project_config(&root)?;
    if !config.agents.contains_key(agent) {
        return Err(anyhow!("agent `{agent}` is not configured in project.yml"));
    }

    if let Some(existing) = config
        .skills
        .mirrors
        .iter_mut()
        .find(|mirror| mirror.reference == skill_id)
    {
        if !existing.targets.iter().any(|target| target == agent) {
            existing.targets.push(agent.to_string());
            existing.targets.sort();
        }
    } else {
        config.skills.mirrors.push(MirrorDecl {
            reference: skill_id.to_string(),
            targets: vec![agent.to_string()],
        });
    }

    save_project_config(&root, &config)
}

pub fn set_agent_enabled(project_root: &Path, agent: &str, enabled: bool) -> Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    let mut config = load_or_default_project_config(&root)?;
    let Some(agent_config) = config.agents.get_mut(agent) else {
        return Err(anyhow!("agent `{agent}` is not configured in project.yml"));
    };
    agent_config.enabled = enabled;
    save_project_config(&root, &config)
}

#[cfg(test)]
mod agent_tests {
    use super::*;

    #[test]
    fn set_agent_enabled_updates_project_config() {
        let temp = tempfile::tempdir().expect("tempdir");

        set_agent_enabled(temp.path(), "cline", true).expect("enable cline");
        let enabled = load_or_default_project_config(temp.path()).expect("config");
        assert!(enabled.agents.get("cline").expect("cline").enabled);

        set_agent_enabled(temp.path(), "cline", false).expect("disable cline");
        let disabled = load_or_default_project_config(temp.path()).expect("config");
        assert!(!disabled.agents.get("cline").expect("cline").enabled);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_codex_and_claude_targets() {
        let temp = tempfile::tempdir().expect("tempdir");
        let config = default_project_config(temp.path());

        assert!(config.agents.contains_key("codex"));
        assert!(config.agents.contains_key("claude-code"));
        assert_eq!(
            config
                .agents
                .get("codex")
                .and_then(|agent| agent.exports.skills_dir.as_deref()),
            Some(".agents/skills")
        );
    }

    #[test]
    fn default_config_uses_cline_rules_directory() {
        let temp = tempfile::tempdir().expect("tempdir");
        let config = default_project_config(temp.path());
        let cline = config.agents.get("cline").expect("cline");

        assert_eq!(cline.exports.instructions.as_deref(), None);
        assert_eq!(cline.exports.rules_dir.as_deref(), Some(".clinerules"));
    }

    #[test]
    fn load_migrates_legacy_cline_single_file_export() {
        let temp = tempfile::tempdir().expect("tempdir");
        ensure_kernel_dir(temp.path()).expect("kernel dir");
        fs::write(
            project_config_path(temp.path()),
            r#"version: 1
project:
  name: demo
  root: .
agents:
  cline:
    enabled: true
    exports:
      instructions: .clinerules
"#,
        )
        .expect("write project");

        let config = load_or_default_project_config(temp.path()).expect("load config");
        let cline = config.agents.get("cline").expect("cline");

        assert_eq!(cline.exports.instructions.as_deref(), None);
        assert_eq!(cline.exports.rules_dir.as_deref(), Some(".clinerules"));
    }
}
