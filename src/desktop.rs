use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use eframe::egui;

use crate::catalog;
use crate::config;
use crate::draft;
use crate::observation;
use crate::project_registry::{self, ProjectRegistry, RegisteredProject};
use crate::review;
use crate::skilllet;

const APP_TITLE: &str = "Agent-Kernel";
const APP_SUBTITLE: &str = "Local Skilllet evolution for Claude Code and Codex";
const DRAFT_INBOX_TITLE: &str = "Draft Inbox";
const CATALOG_TITLE: &str = "Skilllet Catalog";
const SKILLLET_MATRIX_TITLE: &str = "Skilllet Target Matrix";
const APPROVE_LABEL: &str = "Approve";
const REJECT_LABEL: &str = "Reject";
const EDIT_LABEL: &str = "Edit";
const SAVE_LABEL: &str = "Save";
const CANCEL_LABEL: &str = "Cancel";
const INSTALL_CODEX_LABEL: &str = "Install Codex";
const INSTALL_CLAUDE_LABEL: &str = "Install Claude";
const CONFIDENCE_LABEL: &str = "Confidence";
const MATCHED_TEMPLATE_LABEL: &str = "Matched Template";
const REASON_LABEL: &str = "Reason";
const SIDEBAR_SCROLL_ID: &str = "native-project-sidebar-scroll";
const DRAFT_SCROLL_ID: &str = "native-draft-inbox-scroll";
const CATALOG_SCROLL_ID: &str = "native-catalog-scroll";
const MATRIX_SCROLL_ID: &str = "native-skilllet-matrix-scroll";
const OUTPUT_SCROLL_ID: &str = "native-output-scroll";
const WORKSPACE_SCROLL_ID: &str = "native-project-workspace-scroll";
const MAX_HEADER_MARKERS: usize = 3;

#[derive(Debug, Clone, Copy)]
struct UiPalette {
    background: egui::Color32,
    sidebar: egui::Color32,
    card: egui::Color32,
    card_alt: egui::Color32,
    border: egui::Color32,
    text: egui::Color32,
    muted: egui::Color32,
    accent: egui::Color32,
    accent_soft: egui::Color32,
    success: egui::Color32,
    danger: egui::Color32,
}

/// Tracks in-progress edits to a single draft card.
#[derive(Debug, Clone)]
struct DraftEditState {
    draft_id: String,
    title: String,
    body: String,
    kind: String,
    scope: String,
    targets_text: String,
}

enum EditAction {
    Start(DraftEditState),
    Save(DraftEditState),
    Cancel,
}

/// Reduces the current-frame local edit and an optional action into the next draft_edit state.
/// - `None` action: persist local_edit so TextEdit mutations survive between frames.
/// - `Start` action: replace with a fresh edit state.
/// - `Save` action: set state for downstream save (caller clears after save_draft_edit_for_selected).
/// - `Cancel` action: clear edit state.
fn reduce_edit_state(
    local_edit: Option<DraftEditState>,
    action: &Option<EditAction>,
) -> Option<DraftEditState> {
    match action {
        Some(EditAction::Start(state)) => Some(state.clone()),
        Some(EditAction::Save(state)) => Some(state.clone()),
        Some(EditAction::Cancel) => None,
        None => local_edit,
    }
}

fn ui_palette() -> UiPalette {
    UiPalette {
        background: egui::Color32::from_rgb(242, 244, 248),
        sidebar: egui::Color32::from_rgb(232, 236, 243),
        card: egui::Color32::from_rgb(255, 255, 255),
        card_alt: egui::Color32::from_rgb(247, 249, 252),
        border: egui::Color32::from_rgb(218, 224, 233),
        text: egui::Color32::from_rgb(27, 31, 38),
        muted: egui::Color32::from_rgb(103, 112, 126),
        accent: egui::Color32::from_rgb(0, 113, 227),
        accent_soft: egui::Color32::from_rgb(224, 239, 255),
        success: egui::Color32::from_rgb(35, 132, 67),
        danger: egui::Color32::from_rgb(190, 61, 61),
    }
}

fn configure_native_style(ctx: &egui::Context) {
    let palette = ui_palette();
    let mut style = (*ctx.global_style()).clone();
    style.visuals = egui::Visuals::light();
    style.visuals.panel_fill = palette.background;
    style.visuals.window_fill = palette.card;
    style.visuals.window_stroke = egui::Stroke::new(1.0, palette.border);
    style.visuals.window_corner_radius = egui::CornerRadius::same(18);
    style.visuals.menu_corner_radius = egui::CornerRadius::same(12);
    style.visuals.faint_bg_color = palette.card_alt;
    style.visuals.extreme_bg_color = palette.card;
    style.visuals.hyperlink_color = palette.accent;
    style.visuals.selection.bg_fill = palette.accent_soft;
    style.visuals.selection.stroke = egui::Stroke::new(1.0, palette.accent);
    style.visuals.widgets.noninteractive.bg_fill = palette.card;
    style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, palette.border);
    style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, palette.text);
    style.visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(12);
    style.visuals.widgets.inactive.bg_fill = palette.card_alt;
    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, palette.border);
    style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, palette.text);
    style.visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(10);
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(238, 244, 252);
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, palette.accent);
    style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, palette.text);
    style.visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(10);
    style.visuals.widgets.active.bg_fill = palette.accent_soft;
    style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, palette.accent);
    style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, palette.text);
    style.visuals.widgets.active.corner_radius = egui::CornerRadius::same(10);
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 7.0);
    style.spacing.window_margin = egui::Margin::same(18);
    ctx.set_global_style(style);
}

fn page_frame() -> egui::Frame {
    let palette = ui_palette();
    egui::Frame::new()
        .fill(palette.background)
        .inner_margin(egui::Margin::same(18))
}

fn card_frame() -> egui::Frame {
    let palette = ui_palette();
    egui::Frame::new()
        .fill(palette.card)
        .stroke(egui::Stroke::new(1.0, palette.border))
        .corner_radius(egui::CornerRadius::same(18))
        .inner_margin(egui::Margin::same(12))
        .outer_margin(egui::Margin::symmetric(0, 4))
}

fn subtle_card_frame() -> egui::Frame {
    let palette = ui_palette();
    egui::Frame::new()
        .fill(palette.card_alt)
        .stroke(egui::Stroke::new(1.0, palette.border))
        .corner_radius(egui::CornerRadius::same(14))
        .inner_margin(egui::Margin::same(10))
        .outer_margin(egui::Margin::symmetric(0, 3))
}

fn primary_button(label: &'static str) -> egui::Button<'static> {
    let palette = ui_palette();
    egui::Button::new(
        egui::RichText::new(label)
            .color(egui::Color32::WHITE)
            .strong(),
    )
    .fill(palette.accent)
    .stroke(egui::Stroke::NONE)
    .corner_radius(egui::CornerRadius::same(12))
    .min_size(egui::vec2(0.0, 34.0))
}

fn secondary_button(label: &'static str) -> egui::Button<'static> {
    let palette = ui_palette();
    egui::Button::new(egui::RichText::new(label).color(palette.text))
        .fill(palette.card)
        .stroke(egui::Stroke::new(1.0, palette.border))
        .corner_radius(egui::CornerRadius::same(12))
        .min_size(egui::vec2(0.0, 34.0))
}

fn danger_button(label: &'static str) -> egui::Button<'static> {
    let palette = ui_palette();
    egui::Button::new(egui::RichText::new(label).color(palette.danger).strong())
        .fill(egui::Color32::from_rgb(255, 245, 245))
        .stroke(egui::Stroke::new(
            1.0,
            egui::Color32::from_rgb(245, 200, 200),
        ))
        .corner_radius(egui::CornerRadius::same(12))
        .min_size(egui::vec2(0.0, 34.0))
}

fn visible_header_markers(markers: &[String]) -> Vec<String> {
    let mut visible = markers
        .iter()
        .take(MAX_HEADER_MARKERS)
        .map(|marker| format!("Marker: {}", compact_marker(marker)))
        .collect::<Vec<_>>();
    if markers.len() > MAX_HEADER_MARKERS {
        visible.push(format!(
            "+{} more markers",
            markers.len() - MAX_HEADER_MARKERS
        ));
    }
    visible
}

fn compact_marker(marker: &str) -> String {
    const MAX_MARKER_CHARS: usize = 24;
    if marker.chars().count() <= MAX_MARKER_CHARS {
        return marker.to_string();
    }
    let tail = marker
        .chars()
        .rev()
        .take(MAX_MARKER_CHARS - 1)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("…{tail}")
}

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
                confidence: None,
                reason: None,
                matched_template: None,
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
                confidence: None,
                reason: None,
                matched_template: None,
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
        assert_eq!(SKILLLET_MATRIX_TITLE, "Skilllet Target Matrix");
        assert_eq!(APPROVE_LABEL, "Approve");
        assert_eq!(REJECT_LABEL, "Reject");
        assert_eq!(EDIT_LABEL, "Edit");
        assert_eq!(SAVE_LABEL, "Save");
        assert_eq!(CANCEL_LABEL, "Cancel");
        assert_eq!(INSTALL_CODEX_LABEL, "Install Codex");
        assert_eq!(INSTALL_CLAUDE_LABEL, "Install Claude");
        assert_eq!(CONFIDENCE_LABEL, "Confidence");
        assert_eq!(MATCHED_TEMPLATE_LABEL, "Matched Template");
        assert_eq!(REASON_LABEL, "Reason");
    }

    #[test]
    fn native_modern_ui_tokens_are_stable() {
        assert_eq!(APP_TITLE, "Agent-Kernel");
        assert_eq!(
            APP_SUBTITLE,
            "Local Skilllet evolution for Claude Code and Codex"
        );
        assert_eq!(SIDEBAR_SCROLL_ID, "native-project-sidebar-scroll");
        assert_eq!(DRAFT_SCROLL_ID, "native-draft-inbox-scroll");
        assert_eq!(CATALOG_SCROLL_ID, "native-catalog-scroll");
        assert_eq!(MATRIX_SCROLL_ID, "native-skilllet-matrix-scroll");
        assert_eq!(OUTPUT_SCROLL_ID, "native-output-scroll");
        assert_eq!(WORKSPACE_SCROLL_ID, "native-project-workspace-scroll");

        let palette = ui_palette();
        assert!(palette.background.r() > 235);
        assert!(palette.card.r() > palette.background.r());
        assert!(palette.accent.b() > palette.accent.r());
    }

    #[test]
    fn native_header_markers_are_summarized_for_narrow_layouts() {
        let markers = vec![
            ".agent-kernel/project".to_string(),
            "AGENTS.md".to_string(),
            "CLAUDE.md".to_string(),
            ".agents/skills".to_string(),
            ".git".to_string(),
        ];

        let visible = visible_header_markers(&markers);

        assert_eq!(visible.len(), 4);
        assert_eq!(visible[3], "+2 more markers");
        assert!(visible.iter().all(|marker| marker.chars().count() <= 40));
    }

    #[test]
    fn compact_marker_preserves_short_paths_and_truncates_long_paths() {
        assert_eq!(compact_marker("AGENTS.md"), "AGENTS.md");

        let compacted = compact_marker(".very/long/path/that/would/wrap/in/a/header");

        assert!(compacted.starts_with('…'));
        assert!(compacted.chars().count() <= 24);
        assert!(compacted.ends_with("wrap/in/a/header"));
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

    #[test]
    fn native_skilllet_target_toggle_updates_matrix() {
        let temp = tempfile::tempdir().expect("tempdir");
        crate::skilllet::add_skilllet(
            temp.path(),
            "project:use-axios",
            "Use Axios",
            "Use Axios for frontend HTTP requests.",
            "preference",
            "project",
            vec!["codex".to_string()],
        )
        .expect("add skilllet");

        let after_add =
            toggle_native_skilllet_target(temp.path(), "project:use-axios", "claude-code")
                .expect("toggle on");
        let row = after_add
            .rows
            .iter()
            .find(|row| row.skilllet_id == "project:use-axios")
            .expect("row");
        assert_eq!(row.targets.get("codex"), Some(&true));
        assert_eq!(row.targets.get("claude-code"), Some(&true));

        let after_remove = toggle_native_skilllet_target(temp.path(), "project:use-axios", "codex")
            .expect("toggle off");
        let row = after_remove
            .rows
            .iter()
            .find(|row| row.skilllet_id == "project:use-axios")
            .expect("row");
        assert_eq!(row.targets.get("codex"), Some(&false));
        assert_eq!(row.targets.get("claude-code"), Some(&true));
    }

    #[test]
    fn native_skilllet_target_toggle_rejects_empty_target_set() {
        let temp = tempfile::tempdir().expect("tempdir");
        crate::skilllet::add_skilllet(
            temp.path(),
            "project:use-axios",
            "Use Axios",
            "Use Axios for frontend HTTP requests.",
            "preference",
            "project",
            vec!["codex".to_string()],
        )
        .expect("add skilllet");

        let err = toggle_native_skilllet_target(temp.path(), "project:use-axios", "codex")
            .expect_err("empty target set should be rejected");

        assert!(err.to_string().contains("at least one target"));
    }

    #[test]
    fn native_scroll_area_ids_are_unique() {
        let ids = &[
            SIDEBAR_SCROLL_ID,
            DRAFT_SCROLL_ID,
            CATALOG_SCROLL_ID,
            MATRIX_SCROLL_ID,
            OUTPUT_SCROLL_ID,
            WORKSPACE_SCROLL_ID,
        ];
        let mut seen = std::collections::HashSet::new();
        for id in ids {
            assert!(seen.insert(id), "duplicate ScrollArea id: {id}");
        }
    }

    #[test]
    fn native_action_labels_are_compact() {
        let labels = &[
            APPROVE_LABEL,
            REJECT_LABEL,
            EDIT_LABEL,
            SAVE_LABEL,
            CANCEL_LABEL,
            INSTALL_CODEX_LABEL,
            INSTALL_CLAUDE_LABEL,
        ];
        for label in labels {
            assert!(
                label.len() <= 22,
                "label `{label}` is too long ({len} chars); keep it short to avoid wrapping",
                len = label.len()
            );
        }
    }

    #[test]
    fn native_page_frame_uses_background_fill() {
        let palette = ui_palette();
        // Verify palette tokens are coherent: card is lighter than background
        assert!(palette.card.r() > palette.background.r());
        assert!(palette.sidebar.r() < palette.card.r());
    }

    #[test]
    fn native_draft_edit_state_captures_fields_from_record() {
        let state = DraftEditState {
            draft_id: "project:prefer-bun".to_string(),
            title: "Prefer Bun".to_string(),
            body: "Use Bun.".to_string(),
            kind: "preference".to_string(),
            scope: "project".to_string(),
            targets_text: "codex, claude-code".to_string(),
        };

        assert_eq!(state.draft_id, "project:prefer-bun");
        assert_eq!(state.title, "Prefer Bun");
        assert_eq!(state.body, "Use Bun.");
        assert_eq!(state.kind, "preference");
        assert_eq!(state.scope, "project");
        assert_eq!(state.targets_text, "codex, claude-code");
    }

    #[test]
    fn native_draft_update_edits_without_approving() {
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
                confidence: None,
                reason: None,
                matched_template: None,
            },
        )
        .expect("add draft");

        let updated = draft::update_draft(
            temp.path(),
            "project:prefer-bun",
            draft::DraftUpdate {
                title: Some("Prefer Bun Runtime".to_string()),
                body: Some("Use Bun everywhere.".to_string()),
                kind: Some("constraint".to_string()),
                scope: Some("global".to_string()),
                targets: Some(vec!["codex".to_string(), "claude-code".to_string()]),
            },
        )
        .expect("update draft");

        assert_eq!(updated.title, "Prefer Bun Runtime");
        assert_eq!(updated.body, "Use Bun everywhere.");
        assert_eq!(updated.kind, "constraint");
        assert_eq!(updated.scope, "global");
        assert_eq!(updated.targets, vec!["claude-code", "codex"]);

        // Draft should still exist after update (not approved)
        let drafts = draft::load_drafts(temp.path()).expect("drafts");
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].status, "draft");
    }

    #[test]
    fn native_draft_edit_persists_text_edits_between_frames() {
        // The production reducer is reduce_edit_state. This test exercises it
        // directly so the regression test will FAIL if the reducer is ever
        // weakened (e.g. TextEdit mutations stop surviving between frames).
        let start_state = DraftEditState {
            draft_id: "test:id".to_string(),
            title: "Original Title".to_string(),
            body: "Original body text.".to_string(),
            kind: "preference".to_string(),
            scope: "project".to_string(),
            targets_text: "codex, claude-code".to_string(),
        };
        let start_action = EditAction::Start(start_state);

        // Frame 1: user clicks Edit, so Start action sets draft_edit.
        let draft_edit = reduce_edit_state(None, &Some(start_action));
        assert_eq!(draft_edit.as_ref().unwrap().title, "Original Title");
        assert_eq!(draft_edit.as_ref().unwrap().body, "Original body text.");

        // Frame 2: render clones draft_edit into local_edit, TextEdit mutates it
        let mut local_edit = draft_edit.clone();
        local_edit.as_mut().unwrap().title = "Modified Title".to_string();
        local_edit.as_mut().unwrap().body = "Modified body text.".to_string();
        // No action this frame; reducer must persist local_edit.
        let draft_edit = reduce_edit_state(local_edit, &None);
        assert_eq!(draft_edit.as_ref().unwrap().title, "Modified Title");
        assert_eq!(draft_edit.as_ref().unwrap().body, "Modified body text.");

        // Frame 3: next clone must see persisted mutations
        let persisted = draft_edit.clone().expect("state still present");
        assert_eq!(persisted.title, "Modified Title");
        assert_eq!(persisted.body, "Modified body text.");

        // Cancel clears the edit state
        let cancel_action = EditAction::Cancel;
        let draft_edit = reduce_edit_state(draft_edit, &Some(cancel_action));
        assert!(draft_edit.is_none(), "Cancel clears edit state");

        // Start overrides with fresh state from the draft record
        let start_action2 = EditAction::Start(persisted.clone());
        // Start uses the state supplied by the selected draft record.
        let draft_edit = reduce_edit_state(None, &Some(start_action2));
        assert_eq!(
            draft_edit.as_ref().unwrap().title,
            "Modified Title",
            "Start uses whatever state is given (from draft record)"
        );

        // Save sets state (so save_draft_edit_for_selected can read it)
        let save_edit = draft_edit.clone().unwrap();
        let save_action = EditAction::Save(save_edit);
        let draft_edit = reduce_edit_state(draft_edit, &Some(save_action));
        assert!(
            draft_edit.is_some(),
            "Save sets state for downstream consumption"
        );
    }

    #[test]
    fn native_edit_labels_are_compact_for_min_width() {
        // All action labels must be short enough to avoid wrapping at min width
        for label in &[EDIT_LABEL, SAVE_LABEL, CANCEL_LABEL] {
            assert!(
                label.len() <= 10,
                "label `{label}` is {len} chars; keep it very short for min-width layout",
                len = label.len()
            );
        }
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
        APP_TITLE,
        options,
        Box::new(move |cc| {
            configure_native_style(&cc.egui_ctx);
            Ok(Box::new(AgentKernelApp::new(home, roots, max_depth)))
        }),
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

fn toggle_native_skilllet_target(
    project_root: &Path,
    skilllet_id: &str,
    agent: &str,
) -> Result<skilllet::SkillletTargetMatrix> {
    let matrix = skilllet::skilllet_target_matrix(project_root)?;
    let Some(row) = matrix
        .rows
        .iter()
        .find(|row| row.skilllet_id == skilllet_id)
    else {
        return Err(anyhow!("skilllet `{skilllet_id}` does not exist"));
    };

    let mut targets = row
        .targets
        .iter()
        .filter_map(|(candidate, assigned)| {
            if *assigned {
                Some(candidate.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if targets.iter().any(|target| target == agent) {
        targets.retain(|target| target != agent);
    } else {
        targets.push(agent.to_string());
    }
    targets.sort();
    targets.dedup();
    if targets.is_empty() {
        return Err(anyhow!(
            "at least one target is required; disable the Agent or remove the Skilllet instead"
        ));
    }

    skilllet::set_skilllet_targets(project_root, skilllet_id, targets)?;
    skilllet::skilllet_target_matrix(project_root)
}

struct AgentKernelApp {
    home: PathBuf,
    scan_roots: Vec<PathBuf>,
    max_depth: usize,
    registry: ProjectRegistry,
    selected_path: Option<String>,
    status: String,
    report: String,
    draft_edit: Option<DraftEditState>,
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
            draft_edit: None,
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

    fn save_draft_edit_for_selected(&mut self) {
        let Some(project) = self.selected_project() else {
            self.report = "Select a project first.".to_string();
            self.draft_edit = None;
            return;
        };
        let Some(ref edit) = self.draft_edit else {
            return;
        };
        let update = draft::DraftUpdate {
            title: Some(edit.title.clone()),
            body: Some(edit.body.clone()),
            kind: Some(edit.kind.clone()),
            scope: Some(edit.scope.clone()),
            targets: {
                let targets: Vec<String> = if edit.targets_text.trim().is_empty() {
                    Vec::new()
                } else {
                    edit.targets_text
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                };
                Some(targets)
            },
        };
        let draft_id = edit.draft_id.clone();
        match draft::update_draft(&PathBuf::from(&project.path), &draft_id, update) {
            Ok(updated) => {
                self.draft_edit = None;
                self.report = format!(
                    "Draft `{}` updated for {}\nTitle: {}\nRun Review to refresh the inbox.",
                    updated.id, project.name, updated.title
                );
            }
            Err(err) => {
                self.draft_edit = None;
                self.report = format!("Draft update failed for `{draft_id}`: {err}");
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

    fn toggle_skilllet_target_for_selected(&mut self, skilllet_id: &str, agent: &str) {
        let Some(project) = self.selected_project() else {
            self.report = "Select a project first.".to_string();
            return;
        };
        match toggle_native_skilllet_target(&PathBuf::from(&project.path), skilllet_id, agent) {
            Ok(matrix) => {
                self.report = format!(
                    "Updated `{skilllet_id}` target `{agent}` for {}\n\n{}",
                    project.name,
                    matrix.render()
                );
            }
            Err(err) => {
                self.report = format!("Skilllet target update failed for `{skilllet_id}`: {err}");
            }
        }
    }
}

impl eframe::App for AgentKernelApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let palette = ui_palette();
        ui.painter()
            .rect_filled(ui.max_rect(), egui::CornerRadius::ZERO, palette.background);
        ui.allocate_ui_with_layout(
            ui.available_size(),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                page_frame().show(ui, |ui| {
                    ui.set_min_size(ui.available_size());
                    self.render_app_shell(ui);
                });
            },
        );
    }
}

impl AgentKernelApp {
    fn render_app_shell(&mut self, ui: &mut egui::Ui) {
        let shell_size = ui.available_size();
        ui.allocate_ui_with_layout(
            shell_size,
            egui::Layout::left_to_right(egui::Align::Min),
            |ui| {
                let sidebar_width = 330.0_f32.min(shell_size.x * 0.34).max(280.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(sidebar_width, shell_size.y),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_min_height(shell_size.y);
                        self.render_project_list(ui);
                    },
                );
                ui.add_space(14.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), shell_size.y),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_min_height(shell_size.y);
                        self.render_project_workspace(ui);
                    },
                );
            },
        );
    }

    fn render_project_list(&mut self, ui: &mut egui::Ui) {
        let palette = ui_palette();
        egui::Frame::new()
            .fill(palette.sidebar)
            .stroke(egui::Stroke::new(1.0, palette.border))
            .corner_radius(egui::CornerRadius::same(22))
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| {
                ui.set_min_height(ui.available_height());
                self.render_project_list_contents(ui);
            });
    }

    fn render_project_list_contents(&mut self, ui: &mut egui::Ui) {
        let palette = ui_palette();
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(APP_TITLE)
                        .size(24.0)
                        .strong()
                        .color(palette.text),
                );
                ui.label(
                    egui::RichText::new(APP_SUBTITLE)
                        .size(12.0)
                        .color(palette.muted),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(secondary_button("Scan")).clicked() {
                    self.refresh_with_scan();
                }
            });
        });
        ui.add_space(14.0);
        ui.label(
            egui::RichText::new(format!(
                "Registry\n{}",
                project_registry::registry_path(&self.home).display()
            ))
            .size(11.0)
            .color(palette.muted),
        );
        ui.add_space(14.0);

        if self.registry.projects.is_empty() {
            subtle_card_frame().show(ui, |ui| {
                ui.label(egui::RichText::new("No projects found yet.").color(palette.muted));
            });
            return;
        }

        egui::ScrollArea::vertical()
            .id_salt(SIDEBAR_SCROLL_ID)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for project in &self.registry.projects {
                    let selected = self.selected_path.as_deref() == Some(project.path.as_str());
                    let fill = if selected {
                        palette.accent_soft
                    } else {
                        palette.card
                    };
                    let stroke = if selected {
                        egui::Stroke::new(1.0, palette.accent)
                    } else {
                        egui::Stroke::new(1.0, palette.border)
                    };
                    egui::Frame::new()
                        .fill(fill)
                        .stroke(stroke)
                        .corner_radius(egui::CornerRadius::same(16))
                        .inner_margin(egui::Margin::same(12))
                        .outer_margin(egui::Margin::symmetric(0, 5))
                        .show(ui, |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(&project.name)
                                            .strong()
                                            .color(palette.text),
                                    )
                                    .fill(egui::Color32::TRANSPARENT)
                                    .stroke(egui::Stroke::NONE)
                                    .corner_radius(egui::CornerRadius::same(12)),
                                )
                                .clicked()
                            {
                                self.selected_path = Some(project.path.clone());
                            }
                            ui.label(
                                egui::RichText::new(project.agents.join("  /  "))
                                    .size(12.0)
                                    .color(palette.accent),
                            );
                            ui.label(
                                egui::RichText::new(&project.path)
                                    .size(11.0)
                                    .color(palette.muted),
                            );
                        });
                }
            });
    }

    fn render_project_workspace(&mut self, ui: &mut egui::Ui) {
        let palette = ui_palette();

        let Some(project) = self.selected_project() else {
            card_frame().show(ui, |ui| {
                ui.heading("Select a project");
                ui.label("Scan common local folders or register a project from the CLI.");
            });
            return;
        };

        egui::ScrollArea::vertical()
            .id_salt(WORKSPACE_SCROLL_ID)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                card_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(&project.name)
                                .size(20.0)
                                .strong()
                                .color(palette.text),
                        );
                        ui.label(
                            egui::RichText::new(&project.path)
                                .size(12.0)
                                .color(palette.muted),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(&self.status)
                                    .size(11.0)
                                    .color(palette.muted),
                            );
                        });
                    });
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        for agent in &project.agents {
                            Self::pill(ui, agent, palette.accent_soft);
                        }
                        for marker in visible_header_markers(&project.markers) {
                            Self::pill(ui, &marker, palette.card_alt);
                        }
                    });
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        if ui.add(secondary_button("Open Folder")).clicked() {
                            let _ = open::that(&project.path);
                        }
                        if ui.add(secondary_button("Review")).clicked() {
                            self.review_selected();
                        }
                        if ui.add(secondary_button("Preview Evolution")).clicked() {
                            self.evolve_selected(true);
                        }
                        if ui.add(primary_button("Evolve to Drafts")).clicked() {
                            self.evolve_selected(false);
                        }
                    });
                });

                self.render_draft_inbox(ui, &project);
                self.render_catalog_store(ui, &project);
                self.render_skilllet_matrix(ui, &project);
                self.render_output(ui);
            });
    }
}

impl AgentKernelApp {
    fn pill(ui: &mut egui::Ui, text: &str, fill: egui::Color32) {
        let palette = ui_palette();
        egui::Frame::new()
            .fill(fill)
            .stroke(egui::Stroke::new(1.0, palette.border))
            .corner_radius(egui::CornerRadius::same(99))
            .inner_margin(egui::Margin::symmetric(10, 5))
            .show(ui, |ui| {
                ui.label(egui::RichText::new(text).size(12.0).color(palette.text));
            });
    }

    fn render_output(&mut self, ui: &mut egui::Ui) {
        let palette = ui_palette();
        card_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Output")
                        .size(16.0)
                        .strong()
                        .color(palette.text),
                );
                ui.label(egui::RichText::new("Latest command result").color(palette.muted));
            });
            ui.add_space(8.0);
            egui::Frame::new()
                .fill(palette.card_alt)
                .corner_radius(egui::CornerRadius::same(14))
                .inner_margin(egui::Margin::same(12))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt(OUTPUT_SCROLL_ID)
                        .max_height(180.0)
                        .show(ui, |ui| {
                            if self.report.is_empty() {
                                ui.label(
                                    egui::RichText::new(
                                        "Run Review or Conversation Evolution to inspect this project.",
                                    )
                                    .color(palette.muted),
                                );
                            } else {
                                ui.monospace(&self.report);
                            }
                        });
                });
        });
    }

    fn render_draft_inbox(&mut self, ui: &mut egui::Ui, project: &RegisteredProject) {
        let palette = ui_palette();
        card_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(DRAFT_INBOX_TITLE)
                        .size(16.0)
                        .strong()
                        .color(palette.text),
                );
                ui.label(
                    egui::RichText::new("Review before enabling generated Skilllets")
                        .color(palette.muted),
                );
            });

            let drafts = match draft::load_drafts(&PathBuf::from(&project.path)) {
                Ok(drafts) => drafts,
                Err(err) => {
                    ui.label(format!("Could not load drafts: {err}"));
                    return;
                }
            };

            if drafts.is_empty() {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("No pending drafts.").color(palette.muted));
                return;
            }

            let mut decision: Option<(String, bool)> = None;
            let mut edit_action: Option<EditAction> = None;
            let mut local_edit: Option<DraftEditState> = self.draft_edit.clone();
            egui::ScrollArea::vertical()
                .id_salt(DRAFT_SCROLL_ID)
                .max_height(260.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for draft in &drafts {
                        subtle_card_frame().show(ui, |ui| {
                            let is_editing =
                                local_edit.as_ref().is_some_and(|e| e.draft_id == draft.id);

                            if is_editing {
                                let edit = local_edit.as_mut().unwrap();
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("Editing")
                                            .size(12.0)
                                            .color(palette.accent),
                                    );
                                    ui.monospace(&draft.id);
                                });
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    ui.label("Title");
                                    ui.add(
                                        egui::TextEdit::singleline(&mut edit.title)
                                            .hint_text("Draft title")
                                            .desired_width(ui.available_width()),
                                    );
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Kind");
                                    ui.add(
                                        egui::TextEdit::singleline(&mut edit.kind)
                                            .hint_text("preference")
                                            .desired_width(120.0),
                                    );
                                    ui.label("Scope");
                                    ui.add(
                                        egui::TextEdit::singleline(&mut edit.scope)
                                            .hint_text("project")
                                            .desired_width(120.0),
                                    );
                                });
                                ui.label("Body");
                                ui.add(
                                    egui::TextEdit::multiline(&mut edit.body)
                                        .hint_text("Draft body")
                                        .desired_rows(2)
                                        .desired_width(ui.available_width()),
                                );
                                ui.label("Targets");
                                ui.add(
                                    egui::TextEdit::singleline(&mut edit.targets_text)
                                        .hint_text("codex, claude-code")
                                        .desired_width(ui.available_width()),
                                );
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    if ui.add(primary_button(SAVE_LABEL)).clicked() {
                                        edit_action =
                                            Some(EditAction::Save(local_edit.clone().unwrap()));
                                    }
                                    if ui.add(danger_button(CANCEL_LABEL)).clicked() {
                                        edit_action = Some(EditAction::Cancel);
                                    }
                                });
                            } else {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(&draft.title)
                                            .strong()
                                            .color(palette.text),
                                    );
                                    Self::pill(ui, &draft.kind, palette.accent_soft);
                                    Self::pill(ui, &draft.scope, palette.card);
                                });
                                ui.monospace(&draft.id);
                                ui.label(egui::RichText::new(&draft.body).color(palette.text));
                                if !draft.targets.is_empty() {
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "Targets: {}",
                                            draft.targets.join(", ")
                                        ))
                                        .size(12.0)
                                        .color(palette.muted),
                                    );
                                }
                                if let Some(confidence) = draft.confidence {
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{CONFIDENCE_LABEL}: {:.0}%",
                                            confidence * 100.0
                                        ))
                                        .size(12.0)
                                        .color(palette.success),
                                    );
                                }
                                if let Some(template) = draft.matched_template.as_deref() {
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{MATCHED_TEMPLATE_LABEL}: {template}"
                                        ))
                                        .size(12.0)
                                        .color(palette.muted),
                                    );
                                }
                                if let Some(reason) = draft.reason.as_deref() {
                                    ui.label(
                                        egui::RichText::new(format!("{REASON_LABEL}: {reason}"))
                                            .size(12.0)
                                            .color(palette.muted),
                                    );
                                }
                                ui.horizontal(|ui| {
                                    if ui.add(primary_button(APPROVE_LABEL)).clicked() {
                                        decision = Some((draft.id.clone(), true));
                                    }
                                    if ui.add(danger_button(REJECT_LABEL)).clicked() {
                                        decision = Some((draft.id.clone(), false));
                                    }
                                    if ui.add(secondary_button(EDIT_LABEL)).clicked() {
                                        edit_action = Some(EditAction::Start(DraftEditState {
                                            draft_id: draft.id.clone(),
                                            title: draft.title.clone(),
                                            body: draft.body.clone(),
                                            kind: draft.kind.clone(),
                                            scope: draft.scope.clone(),
                                            targets_text: draft.targets.join(", "),
                                        }));
                                    }
                                });
                            }
                        });
                    }
                });

            // Persist TextEdit mutations and apply the edit action.
            self.draft_edit = reduce_edit_state(local_edit, &edit_action);

            if let Some((draft_id, approve)) = decision {
                self.decide_draft_for_selected(&draft_id, approve);
            }
            if matches!(edit_action, Some(EditAction::Save(_))) {
                self.save_draft_edit_for_selected();
            }
        });
    }

    fn render_catalog_store(&mut self, ui: &mut egui::Ui, project: &RegisteredProject) {
        let palette = ui_palette();
        card_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(CATALOG_TITLE)
                        .size(16.0)
                        .strong()
                        .color(palette.text),
                );
                ui.label(egui::RichText::new("Local App Store").color(palette.muted));
            });

            let root = PathBuf::from(&project.path);
            let validation = match catalog::load_or_default_catalog(&root) {
                Ok(catalog) => catalog::validate_catalog(&catalog),
                Err(err) => {
                    ui.label(format!("Could not load catalog: {err}"));
                    return;
                }
            };
            ui.label(
                egui::RichText::new(format!(
                    "Catalog Health: {} errors, {} warnings",
                    validation.errors, validation.warnings
                ))
                .size(12.0)
                .color(palette.muted),
            );

            let status = match catalog::catalog_status(&root) {
                Ok(status) => status,
                Err(err) => {
                    ui.label(format!("Could not load catalog status: {err}"));
                    return;
                }
            };

            if status.items.is_empty() {
                ui.label(egui::RichText::new("No catalog packages found.").color(palette.muted));
                return;
            }

            let mut install: Option<(String, Vec<String>)> = None;
            egui::ScrollArea::vertical()
                .id_salt(CATALOG_SCROLL_ID)
                .max_height(300.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for item in &status.items {
                        subtle_card_frame().show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(&item.package.title)
                                        .strong()
                                        .color(palette.text),
                                );
                                let status_text = if item.installed {
                                    "installed"
                                } else {
                                    "available"
                                };
                                let status_color = if item.installed {
                                    palette.success
                                } else {
                                    palette.accent
                                };
                                ui.label(
                                    egui::RichText::new(status_text)
                                        .size(12.0)
                                        .color(status_color),
                                );
                                ui.label(
                                    egui::RichText::new(format!("@{}", item.package.version))
                                        .size(12.0)
                                        .color(palette.muted),
                                );
                            });
                            ui.monospace(&item.package.id);
                            ui.label(
                                egui::RichText::new(&item.package.description).color(palette.text),
                            );
                            ui.label(
                                egui::RichText::new(format!("Source: {}", item.package.source_url))
                                    .size(11.0)
                                    .color(palette.muted),
                            );
                            if !item.package.tags.is_empty() {
                                ui.horizontal_wrapped(|ui| {
                                    for tag in &item.package.tags {
                                        Self::pill(ui, tag, palette.card);
                                    }
                                });
                            }
                            ui.horizontal(|ui| {
                                if ui.add(secondary_button(INSTALL_CODEX_LABEL)).clicked() {
                                    install =
                                        Some((item.package.id.clone(), vec!["codex".to_string()]));
                                }
                                if ui.add(secondary_button(INSTALL_CLAUDE_LABEL)).clicked() {
                                    install = Some((
                                        item.package.id.clone(),
                                        vec!["claude-code".to_string()],
                                    ));
                                }
                            });
                        });
                    }
                });

            if let Some((package_id, targets)) = install {
                self.install_catalog_for_selected(&package_id, targets);
            }
        });
    }

    fn render_skilllet_matrix(&mut self, ui: &mut egui::Ui, project: &RegisteredProject) {
        let palette = ui_palette();
        card_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(SKILLLET_MATRIX_TITLE)
                        .size(16.0)
                        .strong()
                        .color(palette.text),
                );
                ui.label(egui::RichText::new("Assign Skilllets per Agent").color(palette.muted));
            });

            let matrix = match skilllet::skilllet_target_matrix(&PathBuf::from(&project.path)) {
                Ok(matrix) => matrix,
                Err(err) => {
                    ui.label(format!("Could not load skilllet matrix: {err}"));
                    return;
                }
            };

            if matrix.rows.is_empty() {
                ui.label(egui::RichText::new("No skilllets found.").color(palette.muted));
                return;
            }

            let mut toggle: Option<(String, String)> = None;
            egui::ScrollArea::vertical()
                .id_salt(MATRIX_SCROLL_ID)
                .max_height(280.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::Grid::new("native-skilllet-target-matrix")
                        .striped(false)
                        .spacing(egui::vec2(14.0, 8.0))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new("Skilllet")
                                    .strong()
                                    .color(palette.muted),
                            );
                            for agent in &matrix.agents {
                                ui.label(egui::RichText::new(agent).strong().color(palette.muted));
                            }
                            ui.end_row();

                            for row in &matrix.rows {
                                ui.vertical(|ui| {
                                    ui.label(
                                        egui::RichText::new(&row.title)
                                            .strong()
                                            .color(palette.text),
                                    );
                                    ui.label(
                                        egui::RichText::new(&row.skilllet_id)
                                            .size(11.0)
                                            .color(palette.muted),
                                    );
                                });
                                for agent in &matrix.agents {
                                    let assigned = row.targets.get(agent).copied().unwrap_or(false);
                                    let button = if assigned {
                                        primary_button("Assigned")
                                    } else {
                                        secondary_button("Off")
                                    };
                                    if ui.add(button).clicked() {
                                        toggle = Some((row.skilllet_id.clone(), agent.clone()));
                                    }
                                }
                                ui.end_row();
                            }
                        });
                });

            if let Some((skilllet_id, agent)) = toggle {
                self.toggle_skilllet_target_for_selected(&skilllet_id, &agent);
            }
        });
    }
}
