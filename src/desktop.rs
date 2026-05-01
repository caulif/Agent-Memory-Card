use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

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
const APP_SUBTITLE: &str = "Claude Code / Codex 本地 Skilllet 进化引擎";
const DRAFT_INBOX_TITLE: &str = "待审候选";
const CATALOG_TITLE: &str = "技能商店";
const SKILLLET_MATRIX_TITLE: &str = "Agent 分配矩阵";
const APPROVE_LABEL: &str = "批准";
const REJECT_LABEL: &str = "拒绝";
const EDIT_LABEL: &str = "编辑";
const SAVE_LABEL: &str = "保存";
const CANCEL_LABEL: &str = "取消";
const INSTALL_CODEX_LABEL: &str = "安装到 Codex";
const INSTALL_CLAUDE_LABEL: &str = "安装到 Claude";
const CONFIDENCE_LABEL: &str = "可信度";
const MATCHED_TEMPLATE_LABEL: &str = "匹配模板";
const REASON_LABEL: &str = "生成原因";
const SIDEBAR_SCROLL_ID: &str = "native-project-sidebar-scroll";
const DRAFT_SCROLL_ID: &str = "native-draft-inbox-scroll";
const CATALOG_SCROLL_ID: &str = "native-catalog-scroll";
const MATRIX_SCROLL_ID: &str = "native-skilllet-matrix-scroll";
const OUTPUT_SCROLL_ID: &str = "native-output-scroll";
const WORKSPACE_SCROLL_ID: &str = "native-project-workspace-scroll";
const MAX_HEADER_MARKERS: usize = 3;
const MERGE_SELECTED_LABEL: &str = "合并所选";
const MERGE_CONFIRM_LABEL: &str = "创建合并候选";
const MERGE_CANCEL_LABEL: &str = "清空选择";

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

/// Tracks in-progress merge form state when merging multiple drafts.
#[derive(Debug, Clone)]
struct MergeEditState {
    merged_id: String,
    merged_title: String,
    targets_text: String,
}

struct ProjectCache {
    project_path: Option<String>,
    drafts: Vec<draft::DraftRecord>,
    catalog_validation: Option<catalog::CatalogValidationReport>,
    catalog_status: Option<catalog::CatalogStatus>,
    skilllet_matrix: Option<skilllet::SkillletTargetMatrix>,
    error: Option<String>,
}

enum UiTaskResult {
    Scan {
        status: String,
        registry: ProjectRegistry,
        selected_path: Option<String>,
        cache: ProjectCache,
    },
    Report {
        report: String,
        cache: Option<ProjectCache>,
    },
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
        .map(|marker| format!("标记：{}", compact_marker(marker)))
        .collect::<Vec<_>>();
    if markers.len() > MAX_HEADER_MARKERS {
        visible.push(format!(
            "还有 {} 个标记",
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

fn empty_project_cache() -> ProjectCache {
    ProjectCache {
        project_path: None,
        drafts: Vec::new(),
        catalog_validation: None,
        catalog_status: None,
        skilllet_matrix: None,
        error: None,
    }
}

fn load_project_cache(project: Option<&RegisteredProject>) -> ProjectCache {
    let Some(project) = project else {
        return empty_project_cache();
    };
    let root = PathBuf::from(&project.path);
    let mut cache = ProjectCache {
        project_path: Some(project.path.clone()),
        drafts: Vec::new(),
        catalog_validation: None,
        catalog_status: None,
        skilllet_matrix: None,
        error: None,
    };

    match draft::load_drafts(&root) {
        Ok(drafts) => cache.drafts = drafts,
        Err(err) => cache.error = Some(format!("候选读取失败：{err}")),
    }

    match catalog::load_or_default_catalog(&root) {
        Ok(cat) => cache.catalog_validation = Some(catalog::validate_catalog(&cat)),
        Err(err) => {
            if cache.error.is_none() {
                cache.error = Some(format!("技能商店读取失败：{err}"));
            }
        }
    }

    match catalog::catalog_status(&root) {
        Ok(status) => cache.catalog_status = Some(status),
        Err(err) => {
            if cache.error.is_none() {
                cache.error = Some(format!("技能商店状态读取失败：{err}"));
            }
        }
    }

    match skilllet::skilllet_target_matrix(&root) {
        Ok(matrix) => cache.skilllet_matrix = Some(matrix),
        Err(err) => {
            if cache.error.is_none() {
                cache.error = Some(format!("Agent 分配矩阵读取失败：{err}"));
            }
        }
    }

    cache
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
        assert_eq!(DRAFT_INBOX_TITLE, "待审候选");
        assert_eq!(CATALOG_TITLE, "技能商店");
        assert_eq!(SKILLLET_MATRIX_TITLE, "Agent 分配矩阵");
        assert_eq!(APPROVE_LABEL, "批准");
        assert_eq!(REJECT_LABEL, "拒绝");
        assert_eq!(EDIT_LABEL, "编辑");
        assert_eq!(SAVE_LABEL, "保存");
        assert_eq!(CANCEL_LABEL, "取消");
        assert_eq!(INSTALL_CODEX_LABEL, "安装到 Codex");
        assert_eq!(INSTALL_CLAUDE_LABEL, "安装到 Claude");
        assert_eq!(CONFIDENCE_LABEL, "可信度");
        assert_eq!(MATCHED_TEMPLATE_LABEL, "匹配模板");
        assert_eq!(REASON_LABEL, "生成原因");
    }

    #[test]
    fn native_modern_ui_tokens_are_stable() {
        assert_eq!(APP_TITLE, "Agent-Kernel");
        assert_eq!(APP_SUBTITLE, "Claude Code / Codex 本地 Skilllet 进化引擎");
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
        assert_eq!(visible[3], "还有 2 个标记");
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

    #[test]
    fn native_merge_selects_drafts_and_creates_merged_draft() {
        let temp = tempfile::tempdir().expect("tempdir");
        draft::add_draft(
            temp.path(),
            draft::NewDraft {
                id: "project:use-axios".to_string(),
                title: "Use Axios".to_string(),
                body: "Use Axios for frontend HTTP requests.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "test".to_string(),
                confidence: Some(0.92),
                reason: None,
                matched_template: None,
            },
        )
        .expect("add axios");
        draft::add_draft(
            temp.path(),
            draft::NewDraft {
                id: "project:prefer-bun".to_string(),
                title: "Prefer Bun".to_string(),
                body: "Use Bun for package management and scripts.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["claude-code".to_string()],
                evidence: "test".to_string(),
                confidence: Some(0.84),
                reason: None,
                matched_template: None,
            },
        )
        .expect("add bun");

        let merged = draft::merge_drafts(
            temp.path(),
            "project:frontend-defaults",
            "Frontend Defaults",
            vec![
                "project:use-axios".to_string(),
                "project:prefer-bun".to_string(),
            ],
            vec!["codex".to_string(), "claude-code".to_string()],
        )
        .expect("merge");

        assert_eq!(merged.id, "project:frontend-defaults");
        assert_eq!(merged.title, "Frontend Defaults");
        assert_eq!(merged.kind, "procedure");
        assert_eq!(merged.scope, "project");
        assert_eq!(merged.status, "draft");
        assert!(merged.body.contains("Use Axios"));
        assert!(merged.body.contains("Prefer Bun"));
        assert!(merged.body.contains("## Use Axios"));
        assert_eq!(
            merged.evidence,
            "Merged Drafts: project:use-axios, project:prefer-bun"
        );

        // Source drafts must still exist (conservative merge).
        let all_drafts = draft::load_drafts(temp.path()).expect("drafts");
        assert_eq!(all_drafts.len(), 3);
        assert!(all_drafts.iter().any(|d| d.id == "project:use-axios"));
        assert!(all_drafts.iter().any(|d| d.id == "project:prefer-bun"));
        assert!(
            all_drafts
                .iter()
                .any(|d| d.id == "project:frontend-defaults")
        );
    }

    #[test]
    fn native_merge_rejects_empty_sources() {
        let temp = tempfile::tempdir().expect("tempdir");

        let err = draft::merge_drafts(
            temp.path(),
            "project:bad-merge",
            "Bad Merge",
            vec![],
            vec!["codex".to_string()],
        )
        .expect_err("empty sources should fail");

        assert!(err.to_string().contains("at least two source drafts"));
    }

    #[test]
    fn native_merge_rejects_single_source() {
        let temp = tempfile::tempdir().expect("tempdir");
        draft::add_draft(
            temp.path(),
            draft::NewDraft {
                id: "project:solo".to_string(),
                title: "Solo".to_string(),
                body: "Only one.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "test".to_string(),
                confidence: None,
                reason: None,
                matched_template: None,
            },
        )
        .expect("add solo");

        let err = draft::merge_drafts(
            temp.path(),
            "project:solo-merged",
            "Solo Merged",
            vec!["project:solo".to_string()],
            vec!["codex".to_string()],
        )
        .expect_err("single source should fail");

        assert!(err.to_string().contains("at least two source drafts"));
        let drafts = draft::load_drafts(temp.path()).expect("drafts");
        assert_eq!(drafts.len(), 1);
    }

    #[test]
    fn native_merge_rejects_missing_sources() {
        let temp = tempfile::tempdir().expect("tempdir");
        draft::add_draft(
            temp.path(),
            draft::NewDraft {
                id: "project:present".to_string(),
                title: "Present".to_string(),
                body: "This one exists.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "test".to_string(),
                confidence: None,
                reason: None,
                matched_template: None,
            },
        )
        .expect("add present");

        let err = draft::merge_drafts(
            temp.path(),
            "project:bad-merge",
            "Bad Merge",
            vec!["project:present".to_string(), "project:missing".to_string()],
            vec!["codex".to_string()],
        )
        .expect_err("missing source should fail");

        assert!(err.to_string().contains("missing source drafts"));
        assert!(err.to_string().contains("project:missing"));
    }

    #[test]
    fn native_merge_form_state_holds_editable_fields() {
        let state = MergeEditState {
            merged_id: "project:merged-2".to_string(),
            merged_title: "Merged Draft".to_string(),
            targets_text: "codex, claude-code".to_string(),
        };

        assert_eq!(state.merged_id, "project:merged-2");
        assert_eq!(state.merged_title, "Merged Draft");
        assert_eq!(state.targets_text, "codex, claude-code");
    }

    #[test]
    fn native_merge_labels_are_compact() {
        for label in &[
            MERGE_SELECTED_LABEL,
            MERGE_CONFIRM_LABEL,
            MERGE_CANCEL_LABEL,
        ] {
            assert!(
                label.len() <= 22,
                "merge label `{label}` is {len} chars; keep it short",
                len = label.len()
            );
        }
    }

    #[test]
    fn native_merge_preserves_source_explainability() {
        let temp = tempfile::tempdir().expect("tempdir");
        draft::add_draft(
            temp.path(),
            draft::NewDraft {
                id: "project:use-axios".to_string(),
                title: "Use Axios".to_string(),
                body: "Use Axios for frontend HTTP requests.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "observation:a".to_string(),
                confidence: Some(0.92),
                reason: Some("Matched HTTP client preference".to_string()),
                matched_template: Some("built-in:Use Axios".to_string()),
            },
        )
        .expect("add axios");
        draft::add_draft(
            temp.path(),
            draft::NewDraft {
                id: "project:prefer-bun".to_string(),
                title: "Prefer Bun".to_string(),
                body: "Use Bun for package management.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["claude-code".to_string()],
                evidence: "observation:b".to_string(),
                confidence: Some(0.78),
                reason: Some("Matched package manager preference".to_string()),
                matched_template: Some("built-in:Prefer Bun".to_string()),
            },
        )
        .expect("add bun");

        let merged = draft::merge_drafts(
            temp.path(),
            "project:frontend-defaults",
            "Frontend Defaults",
            vec![
                "project:use-axios".to_string(),
                "project:prefer-bun".to_string(),
            ],
            vec!["codex".to_string(), "claude-code".to_string()],
        )
        .expect("merge");

        // Merged draft carries the min confidence.
        assert_eq!(merged.confidence, Some(0.78));
        // Merged draft records the merge source.
        assert!(merged.evidence.contains("Merged Drafts"));
        assert!(merged.matched_template.as_deref() == Some("manual:draft-merge"));

        // Source drafts retain their original explainability fields.
        let drafts = draft::load_drafts(temp.path()).expect("drafts");
        let axios = drafts.iter().find(|d| d.id == "project:use-axios").unwrap();
        assert_eq!(axios.confidence, Some(0.92));
        assert!(axios.reason.as_deref().unwrap().contains("HTTP client"));
    }

    #[test]
    fn native_render_labels_are_chinese() {
        // 验证所有界面标签已转换为中文
        assert_eq!(DRAFT_INBOX_TITLE, "待审候选");
        assert_eq!(CATALOG_TITLE, "技能商店");
        assert_eq!(SKILLLET_MATRIX_TITLE, "Agent 分配矩阵");
        assert_eq!(APPROVE_LABEL, "批准");
        assert_eq!(REJECT_LABEL, "拒绝");
        assert_eq!(EDIT_LABEL, "编辑");
        assert_eq!(SAVE_LABEL, "保存");
        assert_eq!(CANCEL_LABEL, "取消");
        // 确认所有核心标签包含中文字符
        for label in &[
            DRAFT_INBOX_TITLE,
            CATALOG_TITLE,
            SKILLLET_MATRIX_TITLE,
            APPROVE_LABEL,
            REJECT_LABEL,
            EDIT_LABEL,
            SAVE_LABEL,
            CANCEL_LABEL,
            INSTALL_CODEX_LABEL,
            INSTALL_CLAUDE_LABEL,
            CONFIDENCE_LABEL,
            MATCHED_TEMPLATE_LABEL,
            REASON_LABEL,
            MERGE_SELECTED_LABEL,
            MERGE_CONFIRM_LABEL,
            MERGE_CANCEL_LABEL,
        ] {
            assert!(
                label.chars().any(|c| c as u32 > 127),
                "label `{label}` must contain Chinese characters"
            );
        }
    }

    #[test]
    fn native_cache_state_exists_after_init() {
        // 验证 AgentKernelApp 初始化后缓存字段存在且为空状态
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().to_path_buf();
        let app = AgentKernelApp {
            home: home.clone(),
            scan_roots: Vec::new(),
            max_depth: 2,
            registry: ProjectRegistry {
                version: 1,
                projects: Vec::new(),
            },
            selected_path: None,
            status: String::new(),
            report: String::new(),
            draft_edit: None,
            draft_selection: Vec::new(),
            merge_edit: None,
            cached_project_path: None,
            cached_drafts: Vec::new(),
            cached_catalog_validation: None,
            cached_catalog_status: None,
            cached_skilllet_matrix: None,
            cache_error: None,
            task_rx: None,
            busy_task: None,
        };
        assert!(app.cached_project_path.is_none());
        assert!(app.cached_drafts.is_empty());
        assert!(app.cached_catalog_validation.is_none());
        assert!(app.cached_catalog_status.is_none());
        assert!(app.cached_skilllet_matrix.is_none());
        assert!(app.cache_error.is_none());
    }

    #[test]
    fn native_refresh_project_cache_loads_drafts_from_temp_project() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project_root = temp.path().to_path_buf();

        // 注册项目到 registry
        let registered = project_registry::RegisteredProject {
            name: "test-project".to_string(),
            path: project_root.to_string_lossy().to_string(),
            agents: vec!["codex".to_string()],
            markers: Vec::new(),
            last_seen: String::new(),
        };

        // 添加 draft
        draft::add_draft(
            &project_root,
            draft::NewDraft {
                id: "project:cache-test".to_string(),
                title: "Cache Test Draft".to_string(),
                body: "Testing cache refresh.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "cache test".to_string(),
                confidence: Some(0.95),
                reason: None,
                matched_template: None,
            },
        )
        .expect("add draft");

        let mut app = AgentKernelApp {
            home: temp.path().to_path_buf(),
            scan_roots: Vec::new(),
            max_depth: 2,
            registry: ProjectRegistry {
                version: 1,
                projects: vec![registered],
            },
            selected_path: Some(project_root.to_string_lossy().to_string()),
            status: String::new(),
            report: String::new(),
            draft_edit: None,
            draft_selection: Vec::new(),
            merge_edit: None,
            cached_project_path: None,
            cached_drafts: Vec::new(),
            cached_catalog_validation: None,
            cached_catalog_status: None,
            cached_skilllet_matrix: None,
            cache_error: None,
            task_rx: None,
            busy_task: None,
        };

        app.refresh_project_cache_for_selected();

        assert_eq!(
            app.cached_project_path,
            Some(project_root.to_string_lossy().to_string())
        );
        assert_eq!(app.cached_drafts.len(), 1);
        assert_eq!(app.cached_drafts[0].id, "project:cache-test");
        assert_eq!(app.cached_drafts[0].title, "Cache Test Draft");
        assert!(app.cache_error.is_none());
    }

    #[test]
    fn native_background_task_reports_without_blocking_state() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut app = AgentKernelApp {
            home: temp.path().to_path_buf(),
            scan_roots: Vec::new(),
            max_depth: 2,
            registry: ProjectRegistry {
                version: 1,
                projects: Vec::new(),
            },
            selected_path: None,
            status: String::new(),
            report: String::new(),
            draft_edit: None,
            draft_selection: Vec::new(),
            merge_edit: None,
            cached_project_path: None,
            cached_drafts: Vec::new(),
            cached_catalog_validation: None,
            cached_catalog_status: None,
            cached_skilllet_matrix: None,
            cache_error: None,
            task_rx: None,
            busy_task: None,
        };

        app.start_task("测试任务", || UiTaskResult::Report {
            report: "后台任务完成".to_string(),
            cache: None,
        });

        assert!(app.is_busy());
        for _ in 0..20 {
            app.poll_task_result();
            if !app.is_busy() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        assert!(!app.is_busy());
        assert_eq!(app.report, "后台任务完成");
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
    draft_selection: Vec<String>,
    merge_edit: Option<MergeEditState>,
    cached_project_path: Option<String>,
    cached_drafts: Vec<draft::DraftRecord>,
    cached_catalog_validation: Option<catalog::CatalogValidationReport>,
    cached_catalog_status: Option<catalog::CatalogStatus>,
    cached_skilllet_matrix: Option<skilllet::SkillletTargetMatrix>,
    cache_error: Option<String>,
    task_rx: Option<Receiver<UiTaskResult>>,
    busy_task: Option<String>,
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
            draft_selection: Vec::new(),
            merge_edit: None,
            cached_project_path: None,
            cached_drafts: Vec::new(),
            cached_catalog_validation: None,
            cached_catalog_status: None,
            cached_skilllet_matrix: None,
            cache_error: None,
            task_rx: None,
            busy_task: None,
        };
        app.refresh_with_scan();
        app
    }

    fn refresh_with_scan(&mut self) {
        match project_registry::scan_and_register(&self.home, &self.scan_roots, self.max_depth) {
            Ok(report) => {
                self.status = format!(
                    "已扫描 {} 个项目，注册表共 {} 个项目。",
                    report.discovered.len(),
                    report.total
                );
                self.registry = project_registry::load_registry(&self.home).unwrap_or_default();
                if self.selected_path.is_none() {
                    self.selected_path = self.registry.projects.first().map(|p| p.path.clone());
                }
                self.refresh_project_cache_for_selected();
            }
            Err(err) => {
                self.status = format!("扫描失败：{err}");
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

    /// 为当前选中的项目刷新缓存：drafts、catalog、skilllet matrix 各加载一次，避免渲染循环中重复 IO。
    fn refresh_project_cache_for_selected(&mut self) {
        let project = self.selected_project();
        self.apply_project_cache(load_project_cache(project.as_ref()));
    }

    fn apply_project_cache(&mut self, cache: ProjectCache) {
        self.cached_project_path = cache.project_path;
        self.cached_drafts = cache.drafts;
        self.cached_catalog_validation = cache.catalog_validation;
        self.cached_catalog_status = cache.catalog_status;
        self.cached_skilllet_matrix = cache.skilllet_matrix;
        self.cache_error = cache.error;
    }

    fn is_busy(&self) -> bool {
        self.task_rx.is_some()
    }

    fn start_task<F>(&mut self, label: &'static str, job: F)
    where
        F: FnOnce() -> UiTaskResult + Send + 'static,
    {
        if self.is_busy() {
            self.report = format!(
                "正在处理“{}”，请稍候。",
                self.busy_task.as_deref().unwrap_or("任务")
            );
            return;
        }

        let (tx, rx) = mpsc::channel();
        self.task_rx = Some(rx);
        self.busy_task = Some(label.to_string());
        self.report = format!("正在{}...", label);
        thread::spawn(move || {
            let _ = tx.send(job());
        });
    }

    fn poll_task_result(&mut self) {
        let Some(rx) = self.task_rx.take() else {
            return;
        };

        match rx.try_recv() {
            Ok(result) => {
                self.busy_task = None;
                self.apply_task_result(result);
            }
            Err(TryRecvError::Empty) => {
                self.task_rx = Some(rx);
            }
            Err(TryRecvError::Disconnected) => {
                self.busy_task = None;
                self.report = "后台任务意外结束，没有返回结果。".to_string();
            }
        }
    }

    fn apply_task_result(&mut self, result: UiTaskResult) {
        match result {
            UiTaskResult::Scan {
                status,
                registry,
                selected_path,
                cache,
            } => {
                self.status = status;
                self.registry = registry;
                self.selected_path = selected_path;
                self.apply_project_cache(cache);
            }
            UiTaskResult::Report { report, cache } => {
                self.report = report;
                if let Some(cache) = cache {
                    self.apply_project_cache(cache);
                }
            }
        }
    }

    fn start_scan(&mut self) {
        let home = self.home.clone();
        let scan_roots = self.scan_roots.clone();
        let max_depth = self.max_depth;
        let selected_path = self.selected_path.clone();
        self.start_task(
            "扫描项目",
            move || match project_registry::scan_and_register(&home, &scan_roots, max_depth) {
                Ok(report) => {
                    let registry = project_registry::load_registry(&home).unwrap_or_default();
                    let selected_path = selected_path
                        .filter(|path| {
                            registry
                                .projects
                                .iter()
                                .any(|project| &project.path == path)
                        })
                        .or_else(|| {
                            registry
                                .projects
                                .first()
                                .map(|project| project.path.clone())
                        });
                    let selected_project = selected_path.as_ref().and_then(|path| {
                        registry
                            .projects
                            .iter()
                            .find(|project| &project.path == path)
                            .cloned()
                    });
                    let cache = load_project_cache(selected_project.as_ref());
                    UiTaskResult::Scan {
                        status: format!(
                            "已扫描 {} 个项目，注册表共 {} 个项目。",
                            report.discovered.len(),
                            report.total
                        ),
                        registry,
                        selected_path,
                        cache,
                    }
                }
                Err(err) => {
                    let registry = project_registry::load_registry(&home).unwrap_or_default();
                    UiTaskResult::Scan {
                        status: format!("扫描失败：{err}"),
                        registry,
                        selected_path: None,
                        cache: empty_project_cache(),
                    }
                }
            },
        );
    }

    fn review_selected(&mut self) {
        let Some(project) = self.selected_project() else {
            self.report = "请先选择一个项目。".to_string();
            return;
        };
        let project_name = project.name.clone();
        let root = PathBuf::from(&project.path);
        self.start_task("项目体检", move || {
            let report = match review::review_project(&root) {
                Ok(report) => format!(
                    "项目体检：{}\n\n待审候选：{}\n规则测试失败：{}\n产物漂移：{}",
                    project.name,
                    report.summary.drafts_pending,
                    report.summary.rule_tests_failed,
                    report.summary.artifact_drifts
                ),
                Err(err) => format!("项目体检失败：{project_name}\n{err}"),
            };
            UiTaskResult::Report {
                report,
                cache: None,
            }
        });
    }

    fn evolve_selected(&mut self, dry_run: bool) {
        let Some(project) = self.selected_project() else {
            self.report = "请先选择一个项目。".to_string();
            return;
        };
        let targets = vec!["codex".to_string(), "claude-code".to_string()];
        let home = self.home.clone();
        let root = PathBuf::from(&project.path);
        let task_project = project.clone();
        let label = if dry_run {
            "预览对话进化"
        } else {
            "生成候选"
        };
        self.start_task(
            label,
            move || match observation::evolve_local_conversations(&root, &home, targets, dry_run) {
                Ok(report) => UiTaskResult::Report {
                    report: report.render(),
                    cache: (!dry_run).then(|| load_project_cache(Some(&task_project))),
                },
                Err(err) => UiTaskResult::Report {
                    report: format!("对话进化失败：{}\n{err}", task_project.name),
                    cache: None,
                },
            },
        );
    }

    fn decide_draft_for_selected(&mut self, draft_id: &str, approve: bool) {
        let Some(project) = self.selected_project() else {
            self.report = "请先选择一个项目。".to_string();
            return;
        };
        let root = PathBuf::from(&project.path);
        let task_project = project.clone();
        let draft_id = draft_id.to_string();
        let label = if approve {
            "批准候选"
        } else {
            "拒绝候选"
        };
        self.start_task(label, move || {
            match apply_native_draft_decision(&root, &draft_id, approve) {
                Ok(report) => {
                    let action = if approve { "已批准" } else { "已拒绝" };
                    UiTaskResult::Report {
                        report: format!(
                            "{action} `{draft_id}` -> {}\n\n待审候选：{}\n规则测试失败：{}\n产物漂移：{}",
                            task_project.name,
                            report.summary.drafts_pending,
                            report.summary.rule_tests_failed,
                            report.summary.artifact_drifts
                        ),
                        cache: Some(load_project_cache(Some(&task_project))),
                    }
                }
                Err(err) => UiTaskResult::Report {
                    report: format!("候选处理失败：`{draft_id}`\n{err}"),
                    cache: None,
                },
            }
        });
    }

    fn save_draft_edit_for_selected(&mut self) {
        let Some(project) = self.selected_project() else {
            self.report = "请先选择一个项目。".to_string();
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
        let root = PathBuf::from(&project.path);
        let task_project = project.clone();
        self.draft_edit = None;
        self.start_task("保存候选", move || {
            match draft::update_draft(&root, &draft_id, update) {
                Ok(updated) => UiTaskResult::Report {
                    report: format!(
                        "候选已保存：`{}`\n项目：{}\n标题：{}",
                        updated.id, task_project.name, updated.title
                    ),
                    cache: Some(load_project_cache(Some(&task_project))),
                },
                Err(err) => UiTaskResult::Report {
                    report: format!("候选保存失败：`{draft_id}`\n{err}"),
                    cache: None,
                },
            }
        });
    }

    fn merge_drafts_for_selected(&mut self) {
        let Some(project) = self.selected_project() else {
            self.report = "请先选择一个项目。".to_string();
            self.draft_selection.clear();
            self.merge_edit = None;
            return;
        };
        let Some(ref edit) = self.merge_edit else {
            return;
        };
        let merged_id = edit.merged_id.trim().to_string();
        let merged_title = edit.merged_title.trim().to_string();
        if merged_id.is_empty() {
            self.report = "请填写合并候选 ID。".to_string();
            return;
        }
        if merged_title.is_empty() {
            self.report = "请填写合并候选标题。".to_string();
            return;
        }
        if self.draft_selection.len() < 2 {
            self.report = "至少选择两个候选才能合并。".to_string();
            return;
        }
        let targets: Vec<String> = if edit.targets_text.trim().is_empty() {
            Vec::new()
        } else {
            edit.targets_text
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        };
        let source_ids = self.draft_selection.clone();
        let root = PathBuf::from(&project.path);
        let task_project = project.clone();
        self.draft_selection.clear();
        self.merge_edit = None;
        self.start_task("合并候选", move || {
            match draft::merge_drafts(&root, &merged_id, &merged_title, source_ids, targets) {
                Ok(merged) => UiTaskResult::Report {
                    report: format!(
                        "合并候选已创建：`{}`\n项目：{}\n标题：{}",
                        merged.id, task_project.name, merged.title
                    ),
                    cache: Some(load_project_cache(Some(&task_project))),
                },
                Err(err) => UiTaskResult::Report {
                    report: format!("候选合并失败：{err}"),
                    cache: None,
                },
            }
        });
    }

    fn install_catalog_for_selected(&mut self, package_id: &str, targets: Vec<String>) {
        let Some(project) = self.selected_project() else {
            self.report = "请先选择一个项目。".to_string();
            return;
        };
        let target_summary = if targets.is_empty() {
            "全部已启用 Agent".to_string()
        } else {
            targets.join(", ")
        };
        let package_id = package_id.to_string();
        let root = PathBuf::from(&project.path);
        let task_project = project.clone();
        self.start_task("安装 Skilllet", move || {
            match install_native_catalog_package(&root, &package_id, targets) {
                Ok(status) => {
                    let installed = status.items.iter().filter(|item| item.installed).count();
                    UiTaskResult::Report {
                        report: format!(
                            "已安装 `{package_id}` 到 {target_summary}\n项目：{}\n\n已安装包：{installed}/{}",
                            task_project.name,
                            status.items.len()
                        ),
                        cache: Some(load_project_cache(Some(&task_project))),
                    }
                }
                Err(err) => UiTaskResult::Report {
                    report: format!("技能包安装失败：`{package_id}`\n{err}"),
                    cache: None,
                },
            }
        });
    }

    fn toggle_skilllet_target_for_selected(&mut self, skilllet_id: &str, agent: &str) {
        let Some(project) = self.selected_project() else {
            self.report = "请先选择一个项目。".to_string();
            return;
        };
        let skilllet_id = skilllet_id.to_string();
        let agent = agent.to_string();
        let root = PathBuf::from(&project.path);
        let task_project = project.clone();
        self.start_task(
            "更新 Agent 分配",
            move || match toggle_native_skilllet_target(&root, &skilllet_id, &agent) {
                Ok(matrix) => UiTaskResult::Report {
                    report: format!(
                        "已更新 `{skilllet_id}` -> `{agent}`\n项目：{}\n\n{}",
                        task_project.name,
                        matrix.render()
                    ),
                    cache: Some(load_project_cache(Some(&task_project))),
                },
                Err(err) => UiTaskResult::Report {
                    report: format!("Agent 分配更新失败：`{skilllet_id}`\n{err}"),
                    cache: None,
                },
            },
        );
    }
}

impl eframe::App for AgentKernelApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_task_result();
        if self.is_busy() {
            ui.ctx().request_repaint();
        }
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
                if let Some(task) = self.busy_task.as_deref() {
                    ui.label(
                        egui::RichText::new(format!("正在{}...", task))
                            .size(12.0)
                            .color(palette.accent),
                    );
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(!self.is_busy(), secondary_button("扫描"))
                    .clicked()
                {
                    self.start_scan();
                }
            });
        });
        ui.add_space(14.0);
        ui.label(
            egui::RichText::new(format!(
                "项目索引\n{}",
                project_registry::registry_path(&self.home).display()
            ))
            .size(11.0)
            .color(palette.muted),
        );
        ui.add_space(14.0);

        if self.registry.projects.is_empty() {
            subtle_card_frame().show(ui, |ui| {
                ui.label(
                    egui::RichText::new("还没有发现项目。点击“扫描”开始建立索引。")
                        .color(palette.muted),
                );
            });
            return;
        }

        let mut project_path_selected: Option<String> = None;
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
                                project_path_selected = Some(project.path.clone());
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
        if let Some(path) = project_path_selected {
            self.selected_path = Some(path);
            self.refresh_project_cache_for_selected();
        }
    }

    fn render_project_workspace(&mut self, ui: &mut egui::Ui) {
        let palette = ui_palette();

        let Some(project) = self.selected_project() else {
            card_frame().show(ui, |ui| {
                ui.heading("选择一个项目");
                ui.label("从左侧项目列表选择，或点击扫描刷新本地项目索引。");
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
                        let busy = self.is_busy();
                        if ui.add(secondary_button("打开目录")).clicked() {
                            let _ = open::that(&project.path);
                        }
                        if ui.add_enabled(!busy, secondary_button("体检")).clicked() {
                            self.review_selected();
                        }
                        if ui
                            .add_enabled(!busy, secondary_button("预览进化"))
                            .clicked()
                        {
                            self.evolve_selected(true);
                        }
                        if ui.add_enabled(!busy, primary_button("生成候选")).clicked() {
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
                    egui::RichText::new("操作输出")
                        .size(16.0)
                        .strong()
                        .color(palette.text),
                );
                ui.label(egui::RichText::new("最近一次操作结果").color(palette.muted));
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
                                        "点击“体检”或“生成候选”，这里会显示处理结果。",
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

    fn render_draft_inbox(&mut self, ui: &mut egui::Ui, _project: &RegisteredProject) {
        let palette = ui_palette();
        card_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(DRAFT_INBOX_TITLE)
                        .size(16.0)
                        .strong()
                        .color(palette.text),
                );
                ui.label(egui::RichText::new("先审查，再固化为项目记忆").color(palette.muted));
            });

            let drafts = if let Some(ref err) = self.cache_error {
                ui.label(format!("无法读取待审候选：{err}"));
                return;
            } else {
                &self.cached_drafts
            };

            if drafts.is_empty() {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("当前没有待审候选。").color(palette.muted));
                return;
            }

            let mut decision: Option<(String, bool)> = None;
            let mut edit_action: Option<EditAction> = None;
            let mut local_edit: Option<DraftEditState> = self.draft_edit.clone();
            let mut toggle_selection: Option<String> = None;
            let mut start_merge = false;
            let mut merge_confirm = false;
            let mut merge_cancel = false;
            let mut local_merge: Option<MergeEditState> = self.merge_edit.clone();
            egui::ScrollArea::vertical()
                .id_salt(DRAFT_SCROLL_ID)
                .max_height(260.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for draft in drafts.iter() {
                        subtle_card_frame().show(ui, |ui| {
                            let is_editing =
                                local_edit.as_ref().is_some_and(|e| e.draft_id == draft.id);

                            if is_editing {
                                let edit = local_edit.as_mut().unwrap();
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("正在编辑")
                                            .size(12.0)
                                            .color(palette.accent),
                                    );
                                    ui.monospace(&draft.id);
                                });
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    ui.label("标题");
                                    ui.add(
                                        egui::TextEdit::singleline(&mut edit.title)
                                            .hint_text("候选标题")
                                            .desired_width(ui.available_width()),
                                    );
                                });
                                ui.horizontal(|ui| {
                                    ui.label("类型");
                                    ui.add(
                                        egui::TextEdit::singleline(&mut edit.kind)
                                            .hint_text("preference")
                                            .desired_width(120.0),
                                    );
                                    ui.label("范围");
                                    ui.add(
                                        egui::TextEdit::singleline(&mut edit.scope)
                                            .hint_text("project")
                                            .desired_width(120.0),
                                    );
                                });
                                ui.label("内容");
                                ui.add(
                                    egui::TextEdit::multiline(&mut edit.body)
                                        .hint_text("候选内容")
                                        .desired_rows(2)
                                        .desired_width(ui.available_width()),
                                );
                                ui.label("目标 Agent");
                                ui.add(
                                    egui::TextEdit::singleline(&mut edit.targets_text)
                                        .hint_text("codex, claude-code")
                                        .desired_width(ui.available_width()),
                                );
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    if ui
                                        .add_enabled(!self.is_busy(), primary_button(SAVE_LABEL))
                                        .clicked()
                                    {
                                        edit_action =
                                            Some(EditAction::Save(local_edit.clone().unwrap()));
                                    }
                                    if ui.add(danger_button(CANCEL_LABEL)).clicked() {
                                        edit_action = Some(EditAction::Cancel);
                                    }
                                });
                            } else {
                                let is_selected =
                                    self.draft_selection.iter().any(|s| s == &draft.id);
                                ui.horizontal(|ui| {
                                    let mut checked = is_selected;
                                    if ui.checkbox(&mut checked, "").changed() {
                                        toggle_selection = Some(draft.id.clone());
                                    }
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
                                            "目标 Agent：{}",
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
                                    let busy = self.is_busy();
                                    if ui
                                        .add_enabled(!busy, primary_button(APPROVE_LABEL))
                                        .clicked()
                                    {
                                        decision = Some((draft.id.clone(), true));
                                    }
                                    if ui.add_enabled(!busy, danger_button(REJECT_LABEL)).clicked()
                                    {
                                        decision = Some((draft.id.clone(), false));
                                    }
                                    if ui
                                        .add_enabled(!busy, secondary_button(EDIT_LABEL))
                                        .clicked()
                                    {
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

            // Handle draft selection toggle.
            if let Some(draft_id) = toggle_selection {
                if let Some(pos) = self.draft_selection.iter().position(|s| s == &draft_id) {
                    self.draft_selection.remove(pos);
                } else {
                    self.draft_selection.push(draft_id);
                }
                self.draft_selection.sort();
                // Clear merge form when selection changes.
                self.merge_edit = None;
            }

            // Render merge bar when 2+ drafts selected.
            let selection_count = self.draft_selection.len();
            if selection_count >= 2 {
                ui.add_space(8.0);
                subtle_card_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("已选择 {} 个候选", selection_count))
                                .color(palette.accent),
                        );
                        if ui
                            .add_enabled(!self.is_busy(), primary_button(MERGE_SELECTED_LABEL))
                            .clicked()
                        {
                            start_merge = true;
                        }
                        if ui.add(danger_button(MERGE_CANCEL_LABEL)).clicked() {
                            merge_cancel = true;
                        }
                    });
                });
            }

            // Render merge form when active.
            if self.merge_edit.is_some() {
                let merge = local_merge.as_mut().unwrap();
                ui.add_space(6.0);
                card_frame().show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("创建合并候选")
                            .size(14.0)
                            .strong()
                            .color(palette.text),
                    );
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("ID");
                        ui.add(
                            egui::TextEdit::singleline(&mut merge.merged_id)
                                .hint_text("project:merged-draft")
                                .desired_width(ui.available_width()),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("标题");
                        ui.add(
                            egui::TextEdit::singleline(&mut merge.merged_title)
                                .hint_text("合并候选标题")
                                .desired_width(ui.available_width()),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("目标 Agent");
                        ui.add(
                            egui::TextEdit::singleline(&mut merge.targets_text)
                                .hint_text("codex, claude-code")
                                .desired_width(ui.available_width()),
                        );
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(!self.is_busy(), primary_button(MERGE_CONFIRM_LABEL))
                            .clicked()
                        {
                            merge_confirm = true;
                        }
                        if ui.add(danger_button(MERGE_CANCEL_LABEL)).clicked() {
                            merge_cancel = true;
                        }
                    });
                });
            }

            // Apply merge form persistence.
            if start_merge {
                // Pre-fill merge form with defaults from selected drafts.
                let default_targets = {
                    let mut union_targets: Vec<String> = Vec::new();
                    for d in &self.cached_drafts {
                        if self.draft_selection.contains(&d.id) {
                            union_targets.extend(d.targets.clone());
                        }
                    }
                    union_targets.sort();
                    union_targets.dedup();
                    union_targets.join(", ")
                };
                self.merge_edit = Some(MergeEditState {
                    merged_id: format!("project:merged-{}", selection_count),
                    merged_title: String::new(),
                    targets_text: default_targets,
                });
            } else if merge_cancel {
                self.draft_selection.clear();
                self.merge_edit = None;
            } else {
                // Persist merge form edits between frames.
                self.merge_edit = local_merge;
            }

            if merge_confirm {
                self.merge_drafts_for_selected();
            }

            if let Some((draft_id, approve)) = decision {
                self.decide_draft_for_selected(&draft_id, approve);
            }
            if matches!(edit_action, Some(EditAction::Save(_))) {
                self.save_draft_edit_for_selected();
            }
        });
    }

    fn render_catalog_store(&mut self, ui: &mut egui::Ui, _project: &RegisteredProject) {
        let palette = ui_palette();
        card_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(CATALOG_TITLE)
                        .size(16.0)
                        .strong()
                        .color(palette.text),
                );
                ui.label(egui::RichText::new("本地可安装规则包").color(palette.muted));
            });

            let validation = match &self.cached_catalog_validation {
                Some(v) => v,
                None => {
                    if let Some(ref err) = self.cache_error {
                        ui.label(format!("无法读取技能商店：{err}"));
                    } else {
                        ui.label("无法读取技能商店校验信息。");
                    }
                    return;
                }
            };
            ui.label(
                egui::RichText::new(format!(
                    "商店健康度：{} 个错误，{} 个警告",
                    validation.errors, validation.warnings
                ))
                .size(12.0)
                .color(palette.muted),
            );

            let status = match &self.cached_catalog_status {
                Some(s) => s,
                None => {
                    if let Some(ref err) = self.cache_error {
                        ui.label(format!("无法读取技能商店状态：{err}"));
                    } else {
                        ui.label("无法读取技能商店状态。");
                    }
                    return;
                }
            };

            if status.items.is_empty() {
                ui.label(egui::RichText::new("当前没有可安装的技能包。").color(palette.muted));
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
                                    "已安装"
                                } else {
                                    "可安装"
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
                                egui::RichText::new(format!("来源：{}", item.package.source_url))
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
                                let busy = self.is_busy();
                                if ui
                                    .add_enabled(!busy, secondary_button(INSTALL_CODEX_LABEL))
                                    .clicked()
                                {
                                    install =
                                        Some((item.package.id.clone(), vec!["codex".to_string()]));
                                }
                                if ui
                                    .add_enabled(!busy, secondary_button(INSTALL_CLAUDE_LABEL))
                                    .clicked()
                                {
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

    fn render_skilllet_matrix(&mut self, ui: &mut egui::Ui, _project: &RegisteredProject) {
        let palette = ui_palette();
        card_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(SKILLLET_MATRIX_TITLE)
                        .size(16.0)
                        .strong()
                        .color(palette.text),
                );
                ui.label(egui::RichText::new("为每个 Agent 分配 Skilllet").color(palette.muted));
            });

            let matrix = match &self.cached_skilllet_matrix {
                Some(m) => m,
                None => {
                    if let Some(ref err) = self.cache_error {
                        ui.label(format!("无法读取 Agent 分配矩阵：{err}"));
                    } else {
                        ui.label("无法读取 Agent 分配矩阵。");
                    }
                    return;
                }
            };

            if matrix.rows.is_empty() {
                ui.label(egui::RichText::new("当前项目还没有 Skilllet。").color(palette.muted));
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
                                        primary_button("已分配")
                                    } else {
                                        secondary_button("未启用")
                                    };
                                    if ui.add_enabled(!self.is_busy(), button).clicked() {
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
