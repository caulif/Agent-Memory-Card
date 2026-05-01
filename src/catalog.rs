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
    #[serde(default = "default_package_version")]
    pub version: String,
    #[serde(default)]
    pub source_url: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub kind: String,
    pub scope: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogStatus {
    pub items: Vec<CatalogPackageStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogPackageStatus {
    pub package: CatalogPackage,
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogValidationReport {
    pub errors: usize,
    pub warnings: usize,
    pub rows: Vec<CatalogValidationRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogValidationRow {
    pub package_id: String,
    pub level: String,
    pub code: String,
    pub message: String,
}

impl CatalogValidationReport {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Agent-Kernel Catalog Validation\n\n");
        if self.rows.is_empty() {
            out.push_str("No issues found.\n");
        } else {
            for row in &self.rows {
                out.push_str(&format!(
                    "- {} {}: {} ({})\n",
                    row.level, row.package_id, row.message, row.code
                ));
            }
        }
        out.push_str(&format!(
            "\nSummary: {} errors, {} warnings\n",
            self.errors, self.warnings
        ));
        out
    }
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

pub fn validate_catalog(catalog: &Catalog) -> CatalogValidationReport {
    let mut rows = Vec::new();
    let mut seen = std::collections::BTreeSet::new();

    for package in &catalog.packages {
        if !seen.insert(package.id.clone()) {
            rows.push(validation_row(
                &package.id,
                "error",
                "duplicate-id",
                "Package id appears more than once.",
            ));
        }
        if package.version.trim().is_empty() {
            rows.push(validation_row(
                &package.id,
                "error",
                "missing-version",
                "Package version is required for provenance.",
            ));
        }
        if package.source_url.trim().is_empty() {
            rows.push(validation_row(
                &package.id,
                "error",
                "missing-source",
                "Package source_url is required for provenance.",
            ));
        }
        if package.body.trim().is_empty() {
            rows.push(validation_row(
                &package.id,
                "error",
                "empty-body",
                "Package body cannot be empty.",
            ));
        }
        if package.tags.is_empty() {
            rows.push(validation_row(
                &package.id,
                "warning",
                "missing-tags",
                "Package tags improve browsing and review.",
            ));
        }
    }

    CatalogValidationReport {
        errors: rows.iter().filter(|row| row.level == "error").count(),
        warnings: rows.iter().filter(|row| row.level == "warning").count(),
        rows,
    }
}

pub fn catalog_status(project_root: &Path) -> Result<CatalogStatus> {
    let catalog = load_or_default_catalog(project_root)?;
    let installed = skilllet::load_skilllets(project_root)?
        .into_iter()
        .map(|record| record.id)
        .collect::<std::collections::BTreeSet<_>>();
    Ok(CatalogStatus {
        items: catalog
            .packages
            .into_iter()
            .map(|package| CatalogPackageStatus {
                installed: installed.contains(&package.id),
                package,
            })
            .collect(),
    })
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

fn validation_row(
    package_id: &str,
    level: &str,
    code: &str,
    message: &str,
) -> CatalogValidationRow {
    CatalogValidationRow {
        package_id: package_id.to_string(),
        level: level.to_string(),
        code: code.to_string(),
        message: message.to_string(),
    }
}

fn catalog_path(project_root: &Path) -> std::path::PathBuf {
    config::kernel_dir(project_root).join("catalog.yml")
}

fn default_package_version() -> String {
    "0.1.0".to_string()
}

fn default_catalog() -> Catalog {
    Catalog {
        packages: vec![
            CatalogPackage {
                id: "core:rust-quality-gate".to_string(),
                title: "Rust Quality Gate".to_string(),
                description: "Require fmt, clippy, and tests before claiming Rust changes are complete.".to_string(),
                version: "0.1.0".to_string(),
                source_url: "builtin:agent-kernel/core/rust-quality-gate".to_string(),
                tags: vec!["rust".to_string(), "quality".to_string(), "verification".to_string()],
                kind: "procedure".to_string(),
                scope: "project".to_string(),
                body: "Before reporting Rust work complete, run cargo fmt --check, cargo clippy -- -D warnings, and cargo test. Report failures with exact command output.".to_string(),
            },
            CatalogPackage {
                id: "core:generated-artifacts".to_string(),
                title: "Generated Artifacts".to_string(),
                description: "Treat Agent instruction files as build artifacts instead of hand-maintained source.".to_string(),
                version: "0.1.0".to_string(),
                source_url: "builtin:agent-kernel/core/generated-artifacts".to_string(),
                tags: vec!["build".to_string(), "agents".to_string(), "safety".to_string()],
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

    #[test]
    fn catalog_status_marks_installed_packages() {
        let temp = tempfile::tempdir().expect("tempdir");

        let before = catalog_status(temp.path()).expect("status");
        assert!(!before.items[0].installed);

        install_catalog_package(temp.path(), "core:rust-quality-gate", Vec::new())
            .expect("install package");

        let after = catalog_status(temp.path()).expect("status");
        let rust_gate = after
            .items
            .iter()
            .find(|item| item.package.id == "core:rust-quality-gate")
            .expect("rust gate package");
        assert!(rust_gate.installed);
    }

    #[test]
    fn default_catalog_packages_include_provenance() {
        let temp = tempfile::tempdir().expect("tempdir");

        let status = catalog_status(temp.path()).expect("status");
        let first = &status.items[0].package;

        assert_eq!(first.version, "0.1.0");
        assert!(first.source_url.starts_with("builtin:"));
        assert!(first.tags.iter().any(|tag| tag == "quality"));
    }

    #[test]
    fn validate_catalog_reports_duplicate_ids_and_missing_provenance() {
        let catalog = Catalog {
            packages: vec![
                CatalogPackage {
                    id: "demo:one".to_string(),
                    title: "One".to_string(),
                    description: "First package".to_string(),
                    version: "0.1.0".to_string(),
                    source_url: "builtin:demo/one".to_string(),
                    tags: vec!["demo".to_string()],
                    kind: "preference".to_string(),
                    scope: "project".to_string(),
                    body: "Use one.".to_string(),
                },
                CatalogPackage {
                    id: "demo:one".to_string(),
                    title: "Duplicate".to_string(),
                    description: "Duplicate package".to_string(),
                    version: "".to_string(),
                    source_url: "".to_string(),
                    tags: Vec::new(),
                    kind: "preference".to_string(),
                    scope: "project".to_string(),
                    body: "".to_string(),
                },
            ],
        };

        let report = validate_catalog(&catalog);

        assert_eq!(report.errors, 4);
        assert!(report.rows.iter().any(|row| row.code == "duplicate-id"));
        assert!(report.rows.iter().any(|row| row.code == "missing-version"));
        assert!(report.rows.iter().any(|row| row.code == "missing-source"));
        assert!(report.rows.iter().any(|row| row.code == "empty-body"));
        assert!(report.rows.iter().any(|row| row.code == "missing-tags"));
    }
}
