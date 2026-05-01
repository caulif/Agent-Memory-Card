use std::path::PathBuf;

use anyhow::{Result, anyhow};
use eframe::egui;

use crate::observation;
use crate::project_registry::{self, ProjectRegistry, RegisteredProject};
use crate::review;

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
}
