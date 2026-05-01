use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use eframe::egui;

use crate::catalog;
use crate::config;
use crate::draft;
use crate::observation;
use crate::project_registry::{self, ProjectRegistry, RegisteredProject};
use crate::review;

const DRAFT_INBOX_TITLE: &str = "Draft Inbox";
const CATALOG_TITLE: &str = "Skilllet Catalog";
const APPROVE_LABEL: &str = "Approve";
const REJECT_LABEL: &str = "Reject";
const INSTALL_CODEX_LABEL: &str = "Install to Codex";
const INSTALL_CLAUDE_LABEL: &str = "Install to Claude Code";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft;

    #[test]
    fn native_draft_decision_approves_and_refreshes_review() {
        let temp = tempfile::tempdir().expect("tempdir");
        draft::add_draft(
            temp.path(),
            draft::NewDraft {
                id: "project:prefer-bun".to_string(),
                title: "Prefer Bun".to_string(),
                body: "Use Bun for JavaScript package management and scripts.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "native app test".to_string(),
            },
        )
        .expect("add draft");

        let report =
            apply_native_draft_decision(temp.path(), "project:prefer-bun", true).expect("approve");

        assert_eq!(report.summary.drafts_pending, 0);
        assert!(draft::load_drafts(temp.path()).expect("drafts").is_empty());
    }

    #[test]
    fn native_draft_decision_rejects_and_refreshes_review() {
        let temp = tempfile::tempdir().expect("tempdir");
        draft::add_draft(
            temp.path(),
            draft::NewDraft {
                id: "project:prefer-bun".to_string(),
                title: "Prefer Bun".to_string(),
                body: "Use Bun for JavaScript package management and scripts.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "native app test".to_string(),
            },
        )
        .expect("add draft");

        let report =
            apply_native_draft_decision(temp.path(), "project:prefer-bun", false).expect("reject");

        assert_eq!(report.summary.drafts_pending, 0);
        assert!(draft::load_drafts(temp.path()).expect("drafts").is_empty());
    }

    #[test]
    fn native_review_inbox_labels_are_stable() {
        assert_eq!(DRAFT_INBOX_TITLE, "Draft Inbox");
        assert_eq!(CATALOG_TITLE, "Skilllet Catalog");
        assert_eq!(APPROVE_LABEL, "Approve");
        assert_eq!(REJECT_LABEL, "Reject");
        assert_eq!(INSTALL_CODEX_LABEL, "Install to Codex");
        assert_eq!(INSTALL_CLAUDE_LABEL, "Install to Claude Code");
    }

    #[test]
    fn native_catalog_install_refreshes_status() {
        let temp = tempfile::tempdir().expect("tempdir");

        let status = install_native_catalog_package(
            temp.path(),
            "core:rust-quality-gate",
            vec!["codex".to_string()],
        )
        .expect("install catalog package");

        let item = status
            .items
            .iter()
            .find(|item| item.package.id == "core:rust-quality-gate")
            .expect("installed package");
        assert!(item.installed);
    }

    #[test]
    fn native_catalog_install_merges_agent_targets() {
        let temp = tempfile::tempdir().expect("tempdir");

        install_native_catalog_package(
            temp.path(),
            "core:rust-quality-gate",
            vec!["codex".to_string()],
        )
        .expect("install codex");
        install_native_catalog_package(
            temp.path(),
            "core:rust-quality-gate",
            vec!["claude-code".to_string()],
        )
        .expect("install claude");

        let config = crate::config::load_or_default_project_config(temp.path()).expect("config");
        let included = config
            .skilllets
            .include
            .iter()
            .find(|item| item.id == "core:rust-quality-gate")
            .expect("catalog skilllet");
        assert_eq!(included.targets, vec!["claude-code", "codex"]);
    }
}

pub fn run(home: PathBuf, scan_roots: Vec<PathBuf>, max_depth: usize) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let roots = if scan_roots.is_empty() {
        project_registry::default_scan_roots(&cwd)
    } else {
        scan_roots
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([920.0, 620.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Agent-Kernel",
        options,
        Box::new(move |_cc| Ok(Box::new(AgentKernelApp::new(home, roots, max_depth)))),
    )
    .map_err(|err| anyhow!(err.to_string()))
}

fn apply_native_draft_decision(
    project_root: &Path,
    draft_id: &str,
    approve: bool,
) -> Result<review::ReviewReport> {
    let decision = if approve {
        review::ReviewDecision::ApproveDraft(draft_id.to_string())
    } else {
        review::ReviewDecision::RejectDraft(draft_id.to_string())
    };
    review::apply_review_decisions(project_root, &[decision])?;
    review::review_project(project_root)
}

fn install_native_catalog_package(
    project_root: &Path,
    package_id: &str,
    targets: Vec<String>,
) -> Result<catalog::CatalogStatus> {
    let targets = merge_existing_catalog_targets(project_root, package_id, targets)?;
    catalog::install_catalog_package(project_root, package_id, targets)?;
    catalog::catalog_status(project_root)
}

fn merge_existing_catalog_targets(
    project_root: &Path,
    package_id: &str,
    targets: Vec<String>,
) -> Result<Vec<String>> {
    let config = config::load_or_default_project_config(project_root)?;
    let mut merged = config
        .skilllets
        .include
        .iter()
        .find(|item| item.id == package_id)
        .map(|item| item.targets.clone())
        .unwrap_or_default();
    merged.extend(targets);
    merged.sort();
    merged.dedup();
    Ok(merged)
}

struct AgentKernelApp {
    home: PathBuf,
    scan_roots: Vec<PathBuf>,
    max_depth: usize,
    registry: ProjectRegistry,
    selected_path: Option<String>,
    status: String,
    report: String,
}

impl AgentKernelApp {
    fn new(home: PathBuf, scan_roots: Vec<PathBuf>, max_depth: usize) -> Self {
        let mut app = Self {
            home,
            scan_roots,
            max_depth,
            registry: ProjectRegistry {
                version: 1,
                projects: Vec::new(),
            },
            selected_path: None,
            status: String::new(),
            report: String::new(),
        };
        app.refresh_with_scan();
        app
    }

    fn refresh_with_scan(&mut self) {
        match project_registry::scan_and_register(&self.home, &self.scan_roots, self.max_depth) {
            Ok(report) => {
                self.status = format!(
                    "Scanned {} project(s). Registry now has {}.",
                    report.discovered.len(),
                    report.total
                );
                self.registry = project_registry::load_registry(&self.home).unwrap_or_default();
                if self.selected_path.is_none() {
                    self.selected_path = self.registry.projects.first().map(|p| p.path.clone());
                }
            }
            Err(err) => {
                self.status = format!("Scan failed: {err}");
                self.registry = project_registry::load_registry(&self.home).unwrap_or_default();
            }
        }
    }

    fn selected_project(&self) -> Option<RegisteredProject> {
        let selected = self.selected_path.as_ref()?;
        self.registry
            .projects
            .iter()
            .find(|project| &project.path == selected)
            .cloned()
    }

    fn review_selected(&mut self) {
        let Some(project) = self.selected_project() else {
            self.report = "Select a project first.".to_string();
            return;
        };
        match review::review_project(&PathBuf::from(&project.path)) {
            Ok(report) => {
                self.report = format!(
                    "Review for {}\n\nDrafts pending: {}\nRule CI failures: {}\nArtifact drifts: {}",
                    project.name,
                    report.summary.drafts_pending,
                    report.summary.rule_tests_failed,
                    report.summary.artifact_drifts
                );
            }
            Err(err) => {
                self.report = format!("Review failed for {}: {err}", project.name);
            }
        }
    }

    fn evolve_selected(&mut self, dry_run: bool) {
        let Some(project) = self.selected_project() else {
            self.report = "Select a project first.".to_string();
            return;
        };
        let targets = vec!["codex".to_string(), "claude-code".to_string()];
        match observation::evolve_local_conversations(
            &PathBuf::from(&project.path),
            &self.home,
            targets,
            dry_run,
        ) {
            Ok(report) => {
                self.report = report.render();
            }
            Err(err) => {
                self.report = format!("Conversation evolution failed for {}: {err}", project.name);
            }
        }
    }

    fn decide_draft_for_selected(&mut self, draft_id: &str, approve: bool) {
        let Some(project) = self.selected_project() else {
            self.report = "Select a project first.".to_string();
            return;
        };
        match apply_native_draft_decision(&PathBuf::from(&project.path), draft_id, approve) {
            Ok(report) => {
                let action = if approve { "Approved" } else { "Rejected" };
                self.report = format!(
                    "{action} `{draft_id}` for {}\n\nDrafts pending: {}\nRule CI failures: {}\nArtifact drifts: {}",
                    project.name,
                    report.summary.drafts_pending,
                    report.summary.rule_tests_failed,
                    report.summary.artifact_drifts
                );
            }
            Err(err) => {
                self.report = format!("Draft decision failed for `{draft_id}`: {err}");
            }
        }
    }

    fn install_catalog_for_selected(&mut self, package_id: &str, targets: Vec<String>) {
        let Some(project) = self.selected_project() else {
            self.report = "Select a project first.".to_string();
            return;
        };
        let target_summary = if targets.is_empty() {
            "all enabled agents".to_string()
        } else {
            targets.join(", ")
        };
        match install_native_catalog_package(&PathBuf::from(&project.path), package_id, targets) {
            Ok(status) => {
                let installed = status.items.iter().filter(|item| item.installed).count();
                self.report = format!(
                    "Installed `{package_id}` for {target_summary} in {}\n\nInstalled catalog packages: {installed}/{}",
                    project.name,
                    status.items.len()
                );
            }
            Err(err) => {
                self.report = format!("Catalog install failed for `{package_id}`: {err}");
            }
        }
    }
}

impl eframe::App for AgentKernelApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.horizontal(|ui| {
            ui.heading("Agent-Kernel");
            ui.separator();
            ui.label("Native project console for Claude Code / Codex Skilllet evolution");
        });
        ui.separator();

        ui.columns(2, |columns| {
            self.render_project_list(&mut columns[0]);
            self.render_project_workspace(&mut columns[1]);
        });
    }
}

impl AgentKernelApp {
    fn render_project_list(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Projects");
            if ui.button("Scan").clicked() {
                self.refresh_with_scan();
            }
        });
        ui.label(format!(
            "Registry: {}",
            project_registry::registry_path(&self.home).display()
        ));
        ui.add_space(8.0);

        if self.registry.projects.is_empty() {
            ui.label("No projects found yet.");
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            for project in &self.registry.projects {
                let selected = self.selected_path.as_deref() == Some(project.path.as_str());
                if ui
                    .selectable_label(
                        selected,
                        format!("{}  [{}]", project.name, project.agents.join(", ")),
                    )
                    .clicked()
                {
                    self.selected_path = Some(project.path.clone());
                }
                ui.small(&project.path);
                ui.add_space(6.0);
            }
        });
    }

    fn render_project_workspace(&mut self, ui: &mut egui::Ui) {
        if !self.status.is_empty() {
            ui.label(&self.status);
            ui.separator();
        }

        let Some(project) = self.selected_project() else {
            ui.heading("Select a project");
            ui.label("Scan common local folders or register a project from the CLI.");
            return;
        };

        ui.heading(&project.name);
        ui.monospace(&project.path);
        ui.add_space(8.0);

        ui.horizontal_wrapped(|ui| {
            for agent in &project.agents {
                ui.label(format!("Agent: {agent}"));
            }
            for marker in &project.markers {
                ui.label(format!("Marker: {marker}"));
            }
        });

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.button("Open Folder").clicked() {
                let _ = open::that(&project.path);
            }
            if ui.button("Review").clicked() {
                self.review_selected();
            }
            if ui.button("Preview Conversation Evolution").clicked() {
                self.evolve_selected(true);
            }
            if ui.button("Evolve Conversations to Drafts").clicked() {
                self.evolve_selected(false);
            }
        });

        ui.add_space(12.0);
        self.render_draft_inbox(ui, &project);
        ui.add_space(12.0);
        self.render_catalog_store(ui, &project);
        ui.add_space(12.0);
        ui.separator();
        ui.heading("Output");
        egui::ScrollArea::vertical().show(ui, |ui| {
            if self.report.is_empty() {
                ui.label("Run Review or Conversation Evolution to inspect this project.");
            } else {
                ui.monospace(&self.report);
            }
        });
    }

    fn render_draft_inbox(&mut self, ui: &mut egui::Ui, project: &RegisteredProject) {
        ui.heading(DRAFT_INBOX_TITLE);
        let drafts = match draft::load_drafts(&PathBuf::from(&project.path)) {
            Ok(drafts) => drafts,
            Err(err) => {
                ui.label(format!("Could not load drafts: {err}"));
                return;
            }
        };

        if drafts.is_empty() {
            ui.label("No pending drafts.");
            return;
        }

        let mut decision: Option<(String, bool)> = None;
        egui::ScrollArea::vertical()
            .max_height(240.0)
            .show(ui, |ui| {
                for draft in &drafts {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.strong(&draft.title);
                            ui.label(format!("[{} / {}]", draft.kind, draft.scope));
                        });
                        ui.monospace(&draft.id);
                        ui.label(&draft.body);
                        if !draft.targets.is_empty() {
                            ui.small(format!("Targets: {}", draft.targets.join(", ")));
                        }
                        ui.horizontal(|ui| {
                            if ui.button(APPROVE_LABEL).clicked() {
                                decision = Some((draft.id.clone(), true));
                            }
                            if ui.button(REJECT_LABEL).clicked() {
                                decision = Some((draft.id.clone(), false));
                            }
                        });
                    });
                    ui.add_space(6.0);
                }
            });

        if let Some((draft_id, approve)) = decision {
            self.decide_draft_for_selected(&draft_id, approve);
        }
    }

    fn render_catalog_store(&mut self, ui: &mut egui::Ui, project: &RegisteredProject) {
        ui.heading(CATALOG_TITLE);
        let root = PathBuf::from(&project.path);
        let validation = match catalog::load_or_default_catalog(&root) {
            Ok(catalog) => catalog::validate_catalog(&catalog),
            Err(err) => {
                ui.label(format!("Could not load catalog: {err}"));
                return;
            }
        };
        ui.label(format!(
            "Catalog Health: {} errors, {} warnings",
            validation.errors, validation.warnings
        ));

        let status = match catalog::catalog_status(&root) {
            Ok(status) => status,
            Err(err) => {
                ui.label(format!("Could not load catalog status: {err}"));
                return;
            }
        };

        if status.items.is_empty() {
            ui.label("No catalog packages found.");
            return;
        }

        let mut install: Option<(String, Vec<String>)> = None;
        egui::ScrollArea::vertical()
            .max_height(280.0)
            .show(ui, |ui| {
                for item in &status.items {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.strong(&item.package.title);
                            ui.label(if item.installed {
                                "[installed]"
                            } else {
                                "[available]"
                            });
                            ui.small(format!("@{}", item.package.version));
                        });
                        ui.monospace(&item.package.id);
                        ui.label(&item.package.description);
                        ui.small(format!("Source: {}", item.package.source_url));
                        if !item.package.tags.is_empty() {
                            ui.small(format!("Tags: {}", item.package.tags.join(", ")));
                        }
                        ui.horizontal(|ui| {
                            if ui.button(INSTALL_CODEX_LABEL).clicked() {
                                install =
                                    Some((item.package.id.clone(), vec!["codex".to_string()]));
                            }
                            if ui.button(INSTALL_CLAUDE_LABEL).clicked() {
                                install = Some((
                                    item.package.id.clone(),
                                    vec!["claude-code".to_string()],
                                ));
                            }
                        });
                    });
                    ui.add_space(6.0);
                }
            });

        if let Some((package_id, targets)) = install {
            self.install_catalog_for_selected(&package_id, targets);
        }
    }
}
