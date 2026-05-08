use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::fsutil;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectRegistry {
    pub version: u32,
    #[serde(default)]
    pub projects: Vec<RegisteredProject>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredProject {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub markers: Vec<String>,
    #[serde(default)]
    pub agents: Vec<String>,
    pub last_seen: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectScanReport {
    pub registry_path: PathBuf,
    pub discovered: Vec<RegisteredProject>,
    pub total: usize,
}

impl ProjectScanReport {
    pub fn render(&self) -> String {
        let mut output = String::from("Agent Memory Kernel project scan\n\n");
        output.push_str(&format!("Registry: {}\n", self.registry_path.display()));
        output.push_str(&format!("Discovered: {}\n", self.discovered.len()));
        output.push_str(&format!("Total registered: {}\n", self.total));
        if !self.discovered.is_empty() {
            output.push_str("\nProjects:\n");
            for project in &self.discovered {
                output.push_str(&format!(
                    "- {} [{}]\n  {}\n",
                    project.name,
                    project.agents.join(", "),
                    project.path
                ));
            }
        }
        output
    }
}

impl RegisteredProject {
    pub fn from_path(path: &Path, markers: Vec<String>) -> Result<Self> {
        let canonical = fsutil::normalize_project_root(path)?;
        let name = canonical
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("project")
            .to_string();
        let agents = agents_from_markers(&markers);

        Ok(Self {
            name,
            path: fsutil::path_to_slash(&canonical),
            markers,
            agents,
            last_seen: Utc::now().to_rfc3339(),
        })
    }
}

pub fn registry_path(home: &Path) -> PathBuf {
    home.join(".agent-kernel").join("projects.yml")
}

pub fn load_registry(home: &Path) -> Result<ProjectRegistry> {
    let path = registry_path(home);
    if !path.exists() {
        return Ok(ProjectRegistry {
            version: 1,
            projects: Vec::new(),
        });
    }

    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let registry =
        serde_yaml::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    Ok(dedupe_registry(registry))
}

pub fn load_registry_with_agent_projects(home: &Path) -> Result<ProjectRegistry> {
    let mut registry = load_registry(home)?;
    registry.projects.extend(discover_agent_projects(home)?);
    Ok(dedupe_registry(registry))
}

pub fn save_registry(home: &Path, registry: &ProjectRegistry) -> Result<()> {
    let path = registry_path(home);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let deduped = dedupe_registry(registry.clone());
    let text = serde_yaml::to_string(&deduped)?;
    fs::write(&path, text).with_context(|| format!("write {}", path.display()))
}

pub fn add_project(home: &Path, project: &Path) -> Result<RegisteredProject> {
    let markers = markers_for_project(project);
    let record = RegisteredProject::from_path(project, markers)?;
    let mut registry = load_registry(home)?;
    registry.projects.push(record.clone());
    save_registry(home, &registry)?;
    Ok(record)
}

pub fn scan_and_register(
    home: &Path,
    roots: &[PathBuf],
    max_depth: usize,
) -> Result<ProjectScanReport> {
    let mut registry = load_registry(home)?;
    let mut discovered = Vec::new();

    for root in roots {
        for project in discover_projects(root, max_depth)? {
            registry.projects.push(project.clone());
            discovered.push(project);
        }
    }
    for project in discover_agent_projects(home)? {
        registry.projects.push(project.clone());
        discovered.push(project);
    }

    save_registry(home, &registry)?;
    let total = load_registry(home)?.projects.len();
    Ok(ProjectScanReport {
        registry_path: registry_path(home),
        discovered,
        total,
    })
}

pub fn discover_projects(root: &Path, max_depth: usize) -> Result<Vec<RegisteredProject>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut projects = BTreeMap::new();
    let walker = walkdir::WalkDir::new(root)
        .max_depth(max_depth)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| should_enter(entry.path()));

    for entry in walker {
        let entry = entry?;
        if !entry.file_type().is_dir() {
            continue;
        }
        let path = entry.path();
        let markers = markers_for_project(path);
        if markers.is_empty() {
            continue;
        }
        let project = RegisteredProject::from_path(path, markers)?;
        projects.insert(project.path.clone(), project);
    }

    Ok(projects.into_values().collect())
}

pub fn discover_agent_projects(home: &Path) -> Result<Vec<RegisteredProject>> {
    let mut projects = BTreeMap::new();

    collect_project_paths_from_jsonl(
        &home.join(".claude").join("history.jsonl"),
        &["project", "cwd"],
        "claude-code",
        &mut projects,
    )?;
    collect_project_paths_from_jsonl(
        &home.join(".codex").join("history.jsonl"),
        &["cwd", "project"],
        "codex",
        &mut projects,
    )?;
    collect_project_paths_from_jsonl(
        &home.join(".codex").join("session_index.jsonl"),
        &["cwd", "project"],
        "codex",
        &mut projects,
    )?;
    collect_project_paths_from_session_files(
        &home.join(".claude").join("projects"),
        "claude-code",
        &mut projects,
    )?;
    collect_project_paths_from_session_files(
        &home.join(".claude").join("sessions"),
        "claude-code",
        &mut projects,
    )?;
    collect_project_paths_from_session_files(
        &home.join(".codex").join("sessions"),
        "codex",
        &mut projects,
    )?;

    Ok(projects.into_values().collect())
}

pub fn default_scan_roots(cwd: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = fsutil::home_dir() {
        roots.push(home.join("Documents"));
        roots.push(home.join("Desktop"));
    }
    if let Some(parent) = cwd.parent() {
        roots.push(parent.to_path_buf());
    } else {
        roots.push(cwd.to_path_buf());
    }
    dedupe_paths(roots)
}

fn dedupe_registry(registry: ProjectRegistry) -> ProjectRegistry {
    let mut projects = BTreeMap::new();
    for project in registry.projects {
        projects.insert(project.path.clone(), project);
    }

    ProjectRegistry {
        version: registry.version.max(1),
        projects: projects.into_values().collect(),
    }
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for path in paths {
        let key = fsutil::path_to_slash(&path);
        if seen.insert(key) {
            deduped.push(path);
        }
    }
    deduped
}

fn should_enter(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return true;
    };
    !matches!(
        name,
        ".git" | "target" | "node_modules" | ".next" | "dist" | "build"
    )
}

fn markers_for_project(path: &Path) -> Vec<String> {
    let mut markers = Vec::new();
    if path.join(".agent-kernel").is_dir() {
        markers.push(".agent-kernel/project".to_string());
    }
    if path.join("AGENTS.md").is_file() {
        markers.push("AGENTS.md".to_string());
    }
    if path.join("CLAUDE.md").is_file() {
        markers.push("CLAUDE.md".to_string());
    }
    if path.join(".agents").join("skills").is_dir() {
        markers.push(".agents/skills".to_string());
    }
    if path.join(".claude").join("skills").is_dir() {
        markers.push(".claude/skills".to_string());
    }
    if path.join(".git").is_dir() {
        markers.push(".git".to_string());
    }
    markers
}

fn markers_for_agent_project(path: &Path, agent: &str) -> Vec<String> {
    let mut markers = markers_for_project(path);
    match agent {
        "claude-code" => markers.push("claude-code/session".to_string()),
        "codex" => markers.push("codex/session".to_string()),
        _ => markers.push("agent/session".to_string()),
    }
    markers.sort();
    markers.dedup();
    markers
}

fn agents_from_markers(markers: &[String]) -> Vec<String> {
    let mut agents = BTreeSet::new();
    if markers
        .iter()
        .any(|marker| marker == "CLAUDE.md" || marker == ".claude/skills")
        || markers.iter().any(|marker| marker == "claude-code/session")
    {
        agents.insert("claude-code".to_string());
    }
    if markers
        .iter()
        .any(|marker| marker == "AGENTS.md" || marker == ".agents/skills")
        || markers.iter().any(|marker| marker == "codex/session")
    {
        agents.insert("codex".to_string());
    }
    if agents.is_empty() {
        agents.insert("unknown".to_string());
    }
    agents.into_iter().collect()
}

fn collect_project_paths_from_session_files(
    dir: &Path,
    agent: &str,
    projects: &mut BTreeMap<String, RegisteredProject>,
) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in walkdir::WalkDir::new(dir).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|value| value.to_str()) != Some("jsonl") {
            continue;
        }
        collect_project_paths_from_jsonl(entry.path(), &["cwd", "project"], agent, projects)?;
    }
    Ok(())
}

fn collect_project_paths_from_jsonl(
    file: &Path,
    keys: &[&str],
    agent: &str,
    projects: &mut BTreeMap<String, RegisteredProject>,
) -> Result<()> {
    if !file.exists() {
        return Ok(());
    }
    let text = fs::read_to_string(file).with_context(|| format!("read {}", file.display()))?;
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        for path in extract_project_paths_from_value(&value, keys) {
            if !looks_like_local_project_path(&path) {
                continue;
            }
            let path = PathBuf::from(path);
            let markers = markers_for_agent_project(&path, agent);
            let Ok(project) = RegisteredProject::from_path(&path, markers) else {
                continue;
            };
            projects
                .entry(project.path.clone())
                .and_modify(|existing| {
                    existing.markers.extend(project.markers.clone());
                    existing.markers.sort();
                    existing.markers.dedup();
                    existing.agents = agents_from_markers(&existing.markers);
                    existing.last_seen = Utc::now().to_rfc3339();
                })
                .or_insert(project);
        }
    }
    Ok(())
}

fn extract_project_paths_from_value(value: &Value, keys: &[&str]) -> Vec<String> {
    let mut paths = Vec::new();
    match value {
        Value::Object(map) => {
            for key in keys {
                if let Some(Value::String(path)) = map.get(*key) {
                    paths.push(path.clone());
                }
            }
            for child in map.values() {
                paths.extend(extract_project_paths_from_value(child, keys));
            }
        }
        Value::Array(items) => {
            for item in items {
                paths.extend(extract_project_paths_from_value(item, keys));
            }
        }
        _ => {}
    }
    paths
}

fn looks_like_local_project_path(path: &str) -> bool {
    let trimmed = path.trim();
    if trimmed.len() < 3 {
        return false;
    }
    trimmed.contains(":\\")
        || trimmed.starts_with("\\\\")
        || trimmed.starts_with('/')
        || trimmed.starts_with("~/")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn discovers_projects_with_agent_markers() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project = temp.path().join("demo");
        fs::create_dir_all(project.join(".agent-kernel")).expect("kernel dir");
        fs::write(project.join("AGENTS.md"), "codex").expect("agents");
        fs::write(project.join("CLAUDE.md"), "claude").expect("claude");

        let discovered = discover_projects(temp.path(), 3).expect("discover");

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].name, "demo");
        assert_eq!(discovered[0].agents, vec!["claude-code", "codex"]);
        assert!(
            discovered[0]
                .markers
                .contains(&".agent-kernel/project".to_string())
        );
    }

    #[test]
    fn discovers_projects_from_agent_histories() {
        let home = tempfile::tempdir().expect("home");
        let claude_project = home.path().join("claude-project");
        let codex_project = home.path().join("codex-project");
        fs::create_dir_all(&claude_project).expect("claude project");
        fs::create_dir_all(&codex_project).expect("codex project");
        fs::create_dir_all(home.path().join(".claude")).expect("claude home");
        fs::create_dir_all(home.path().join(".codex").join("sessions")).expect("codex sessions");
        fs::write(
            home.path().join(".claude").join("history.jsonl"),
            serde_json::json!({ "project": claude_project.to_string_lossy() }).to_string(),
        )
        .expect("claude history");
        fs::write(
            home.path()
                .join(".codex")
                .join("sessions")
                .join("rollout.jsonl"),
            serde_json::json!({
                "type": "session_meta",
                "payload": { "cwd": codex_project.to_string_lossy() }
            })
            .to_string(),
        )
        .expect("codex session");

        let discovered = discover_agent_projects(home.path()).expect("discover");

        let claude_project_path = fsutil::path_to_slash(
            &fsutil::normalize_project_root(&claude_project).expect("canonical claude path"),
        );
        let codex_project_path = fsutil::path_to_slash(
            &fsutil::normalize_project_root(&codex_project).expect("canonical codex path"),
        );

        assert!(discovered.iter().any(|project| {
            project.path == claude_project_path && project.agents == vec!["claude-code"]
        }));
        assert!(discovered.iter().any(|project| {
            project.path == codex_project_path && project.agents == vec!["codex"]
        }));
    }

    #[test]
    fn registry_roundtrip_dedupes_by_canonical_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("home");
        let project = temp.path().join("project");
        fs::create_dir_all(project.join(".agent-kernel")).expect("kernel dir");

        let record = RegisteredProject::from_path(&project, vec![".agent-kernel/project".into()])
            .expect("record");
        save_registry(
            &home,
            &ProjectRegistry {
                version: 1,
                projects: vec![record.clone(), record],
            },
        )
        .expect("save");

        let loaded = load_registry(&home).expect("load");

        assert_eq!(loaded.projects.len(), 1);
        assert_eq!(loaded.projects[0].name, "project");
    }
}
