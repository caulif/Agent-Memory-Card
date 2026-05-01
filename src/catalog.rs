use std::fs;
use std::path::Path;

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::config;
use crate::fsutil;
use crate::skilllet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    #[serde(default)]
    pub packages: Vec<CatalogPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogPackage {
    pub id: String,
    pub title: String,
    pub description: String,
    pub kind: String,
    pub scope: String,
    pub body: String,
}

pub fn load_or_default_catalog(project_root: &Path) -> Result<Catalog> {
    let root = fsutil::normalize_project_root(project_root)?;
    let path = catalog_path(&root);
    if path.exists() {
        let text = fs::read_to_string(&path)?;
        Ok(serde_yaml::from_str(&text)?)
    } else {
        Ok(default_catalog())
    }
}

pub fn init_catalog(project_root: &Path) -> Result<Catalog> {
    let root = fsutil::normalize_project_root(project_root)?;
    config::ensure_kernel_dir(&root)?;
    let catalog = default_catalog();
    fs::write(catalog_path(&root), serde_yaml::to_string(&catalog)?)?;
    Ok(catalog)
}

pub fn install_catalog_package(
    project_root: &Path,
    id: &str,
    targets: Vec<String>,
) -> Result<CatalogPackage> {
    let catalog = load_or_default_catalog(project_root)?;
    let package = catalog
        .packages
        .into_iter()
        .find(|package| package.id == id)
        .ok_or_else(|| anyhow!("catalog package `{id}` was not found"))?;

    skilllet::add_skilllet(
        project_root,
        &package.id,
        &package.title,
        &package.body,
        &package.kind,
        &package.scope,
        targets,
    )?;

    Ok(package)
}

fn catalog_path(project_root: &Path) -> std::path::PathBuf {
    config::kernel_dir(project_root).join("catalog.yml")
}

fn default_catalog() -> Catalog {
    Catalog {
        packages: vec![
            CatalogPackage {
                id: "core:rust-quality-gate".to_string(),
                title: "Rust Quality Gate".to_string(),
                description: "Require fmt, clippy, and tests before claiming Rust changes are complete.".to_string(),
                kind: "procedure".to_string(),
                scope: "project".to_string(),
                body: "Before reporting Rust work complete, run cargo fmt --check, cargo clippy -- -D warnings, and cargo test. Report failures with exact command output.".to_string(),
            },
            CatalogPackage {
                id: "core:generated-artifacts".to_string(),
                title: "Generated Artifacts".to_string(),
                description: "Treat Agent instruction files as build artifacts instead of hand-maintained source.".to_string(),
                kind: "constraint".to_string(),
                scope: "project".to_string(),
                body: "Treat Agent instruction files as generated artifacts. Edit Skilllets or project.yml, then rebuild generated Agent files.".to_string(),
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_catalog_package_as_owned_skilllet() {
        let temp = tempfile::tempdir().expect("tempdir");

        let result = install_catalog_package(
            temp.path(),
            "core:rust-quality-gate",
            vec!["codex".to_string()],
        )
        .expect("install package");

        assert_eq!(result.id, "core:rust-quality-gate");
        let skilllets = crate::skilllet::load_skilllets(temp.path()).expect("skilllets");
        assert_eq!(skilllets[0].id, "core:rust-quality-gate");

        let project = crate::config::load_or_default_project_config(temp.path()).expect("project");
        assert_eq!(project.skilllets.include[0].targets, vec!["codex"]);
    }
}
