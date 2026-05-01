use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
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
const REVIEW_SCROLL_ID: &str = "native-review-center-scroll";
const OBSERVATION_SCROLL_ID: &str = "native-observations-scroll";
const INSPECTOR_SCROLL_ID: &str = "native-inspector-scroll";
const MAX_HEADER_MARKERS: usize = 3;
const MERGE_SELECTED_LABEL: &str = "合并所选";
const MERGE_CONFIRM_LABEL: &str = "创建合并候选";
const MERGE_CANCEL_LABEL: &str = "清空选择";
const TOP_BAR_HEIGHT: f32 = 80.0;
const LEFT_NAV_WIDTH: f32 = 216.0;
const INSPECTOR_WIDTH: f32 = 376.0;
const BOTTOM_BAR_HEIGHT: f32 = 80.0;
const CANVAS_MIN_HEIGHT: f32 = 520.0;
const CANVAS_SCROLL_ID: &str = "native-reference-canvas-scroll";
const BUSY_REPAINT_INTERVAL_MS: u64 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UiPage {
    Overview,
    Canvas,
    DraftInbox,
    Skilllets,
    Mirrors,
    Agents,
    RuleCi,
    Observations,
    Catalog,
    Settings,
}

impl UiPage {
    const ALL: [UiPage; 10] = [
        UiPage::Overview,
        UiPage::Canvas,
        UiPage::DraftInbox,
        UiPage::Skilllets,
        UiPage::Mirrors,
        UiPage::Agents,
        UiPage::RuleCi,
        UiPage::Observations,
        UiPage::Catalog,
        UiPage::Settings,
    ];

    fn label(self) -> &'static str {
        match self {
            UiPage::Overview => "概览",
            UiPage::Canvas => "画布",
            UiPage::DraftInbox => "草稿收件箱",
            UiPage::Skilllets => "Skilllets",
            UiPage::Mirrors => "镜像状态",
            UiPage::Agents => "Agents",
            UiPage::RuleCi => "规则 CI",
            UiPage::Observations => "观察",
            UiPage::Catalog => "目录",
            UiPage::Settings => "设置",
        }
    }

    fn badge(self) -> &'static str {
        match self {
            UiPage::Overview => "OV",
            UiPage::Canvas => "CV",
            UiPage::DraftInbox => "DR",
            UiPage::Skilllets => "SK",
            UiPage::Mirrors => "MR",
            UiPage::Agents => "AG",
            UiPage::RuleCi => "CI",
            UiPage::Observations => "OB",
            UiPage::Catalog => "CA",
            UiPage::Settings => "ST",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UiActionKind {
    Navigate,
    ScanProjects,
    RefreshProjectCache,
    ReviewProject,
    EvolveConversations,
}

impl UiActionKind {
    fn from_label(label: &str, page: UiPage) -> Self {
        if label.contains("同步") {
            return UiActionKind::ScanProjects;
        }
        if label.contains("刷新") || label.contains("目录") {
            return UiActionKind::RefreshProjectCache;
        }
        if label.contains("审查") || label.contains("构建") || label.contains("Rule CI") {
            return UiActionKind::ReviewProject;
        }
        if page == UiPage::Observations
            || label.contains("合成")
            || label.contains("扫描")
            || label.contains("导入")
        {
            return UiActionKind::EvolveConversations;
        }
        UiActionKind::Navigate
    }
}

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
    warning: egui::Color32,
    purple: egui::Color32,
    teal: egui::Color32,
    orange: egui::Color32,
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
        warning: egui::Color32::from_rgb(255, 149, 0),
        purple: egui::Color32::from_rgb(126, 87, 255),
        teal: egui::Color32::from_rgb(33, 184, 194),
        orange: egui::Color32::from_rgb(255, 126, 31),
    }
}

fn cjk_font_candidates() -> Vec<PathBuf> {
    vec![
        PathBuf::from("C:/Windows/Fonts/msyh.ttc"),
        PathBuf::from("C:/Windows/Fonts/msyh.ttf"),
        PathBuf::from("C:/Windows/Fonts/simhei.ttf"),
        PathBuf::from("/System/Library/Fonts/PingFang.ttc"),
        PathBuf::from("/System/Library/Fonts/STHeiti Light.ttc"),
        PathBuf::from("/Library/Fonts/Arial Unicode.ttf"),
        PathBuf::from("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"),
        PathBuf::from("/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc"),
        PathBuf::from("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc"),
        PathBuf::from("/usr/share/fonts/truetype/wqy/wqy-microhei.ttc"),
    ]
}

fn configure_cjk_fonts(ctx: &egui::Context) {
    let Some(font_path) = cjk_font_candidates().into_iter().find(|path| path.exists()) else {
        return;
    };
    let Ok(bytes) = fs::read(&font_path) else {
        return;
    };

    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "agent_kernel_cjk".to_string(),
        Arc::new(egui::FontData::from_owned(bytes)),
    );
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        family.insert(0, "agent_kernel_cjk".to_string());
    }
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        family.push("agent_kernel_cjk".to_string());
    }
    ctx.set_fonts(fonts);
}

fn configure_native_style(ctx: &egui::Context) {
    configure_cjk_fonts(ctx);
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
        .inner_margin(egui::Margin::ZERO)
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

fn search_box_frame() -> egui::Frame {
    let palette = ui_palette();
    egui::Frame::new()
        .fill(palette.card)
        .stroke(egui::Stroke::new(1.0, palette.border))
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(12, 9))
        .outer_margin(egui::Margin::ZERO)
}

fn render_search_input(ui: &mut egui::Ui, query: &mut String, hint: &str, width: f32) {
    let palette = ui_palette();
    search_box_frame().show(ui, |ui| {
        ui.set_width(width);
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
            let center = rect.center() - egui::vec2(2.0, 2.0);
            ui.painter()
                .circle_stroke(center, 5.0, egui::Stroke::new(1.4, palette.muted));
            ui.painter().line_segment(
                [center + egui::vec2(4.0, 4.0), center + egui::vec2(8.0, 8.0)],
                egui::Stroke::new(1.4, palette.muted),
            );
            ui.add(
                egui::TextEdit::singleline(query)
                    .hint_text(hint)
                    .desired_width((width - 96.0).max(120.0)),
            );
            SelflessUi::soft_badge(ui, "Ctrl K", palette.card_alt, palette.muted);
        });
    });
}

struct SelflessUi;

impl SelflessUi {
    fn soft_badge(ui: &mut egui::Ui, text: &str, fill: egui::Color32, color: egui::Color32) {
        egui::Frame::new()
            .fill(fill)
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_white_alpha(120)))
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.label(egui::RichText::new(text).size(12.0).color(color));
            });
    }
}

fn matches_search(query: &str, text: &str) -> bool {
    let query = query.trim();
    query.is_empty() || text.to_lowercase().contains(&query.to_lowercase())
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

fn compact_middle_path(path: &str) -> String {
    const MAX_PATH_CHARS: usize = 34;
    if path.chars().count() <= MAX_PATH_CHARS {
        return path.to_string();
    }
    let tail = path
        .chars()
        .rev()
        .take(MAX_PATH_CHARS - 2)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("~/{tail}")
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
    fn native_batch_draft_decision_handles_selected_drafts() {
        let temp = tempfile::tempdir().expect("tempdir");
        for id in ["project:prefer-bun", "project:use-axios"] {
            draft::add_draft(
                temp.path(),
                draft::NewDraft {
                    id: id.to_string(),
                    title: id.to_string(),
                    body: "Batch draft.".to_string(),
                    kind: "preference".to_string(),
                    scope: "project".to_string(),
                    targets: vec!["codex".to_string()],
                    evidence: "native batch test".to_string(),
                    confidence: None,
                    reason: None,
                    matched_template: None,
                },
            )
            .expect("add draft");
        }

        let report = apply_native_draft_decisions(
            temp.path(),
            &[
                "project:prefer-bun".to_string(),
                "project:use-axios".to_string(),
            ],
            false,
        )
        .expect("batch reject");

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
            REVIEW_SCROLL_ID,
            OBSERVATION_SCROLL_ID,
            INSPECTOR_SCROLL_ID,
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
            active_page: UiPage::Overview,
            status: String::new(),
            report: String::new(),
            search_query: String::new(),
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
            active_page: UiPage::Overview,
            status: String::new(),
            report: String::new(),
            search_query: String::new(),
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
            active_page: UiPage::Overview,
            status: String::new(),
            report: String::new(),
            search_query: String::new(),
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

    #[test]
    fn native_app_startup_scan_runs_in_background() {
        let temp = tempfile::tempdir().expect("tempdir");
        let app = AgentKernelApp::new(temp.path().to_path_buf(), Vec::new(), 1);

        assert!(
            app.is_busy(),
            "startup project scan should not block the window from opening"
        );
        assert_eq!(app.busy_task.as_deref(), Some("扫描项目"));
        assert!(app.report.contains("正在扫描项目"));
    }

    #[test]
    fn native_cjk_font_candidates_cover_major_desktop_platforms() {
        let candidates = cjk_font_candidates();
        let joined = candidates
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            joined.contains("msyh.ttc"),
            "Windows should prefer Microsoft YaHei"
        );
        assert!(joined.contains("PingFang"), "macOS should prefer PingFang");
        assert!(
            joined.contains("NotoSansCJK"),
            "Linux should prefer Noto Sans CJK"
        );
    }

    #[test]
    fn native_reference_layout_tokens_match_canvas_design() {
        assert_eq!(TOP_BAR_HEIGHT, 80.0);
        assert_eq!(LEFT_NAV_WIDTH, 216.0);
        assert_eq!(INSPECTOR_WIDTH, 376.0);
        assert_eq!(BOTTOM_BAR_HEIGHT, 80.0);
        assert_eq!(CANVAS_MIN_HEIGHT, 520.0);
        assert_eq!(BUSY_REPAINT_INTERVAL_MS, 50);
    }

    #[test]
    fn native_workspace_pages_cover_reference_designs() {
        let labels = UiPage::ALL
            .iter()
            .map(|page| page.label())
            .collect::<Vec<_>>();

        assert_eq!(labels.len(), 10);
        assert!(labels.contains(&"草稿收件箱"));
        assert!(labels.contains(&"Skilllets"));
        assert!(labels.contains(&"规则 CI"));
        assert!(labels.contains(&"观察"));
        assert!(UiPage::ALL.iter().all(|page| page.badge().is_ascii()));
    }

    #[test]
    fn native_header_actions_map_to_real_commands() {
        assert_eq!(
            UiActionKind::from_label("同步", UiPage::Overview),
            UiActionKind::ScanProjects
        );
        assert_eq!(
            UiActionKind::from_label("刷新目录", UiPage::Catalog),
            UiActionKind::RefreshProjectCache
        );
        assert_eq!(
            UiActionKind::from_label("构建预览", UiPage::RuleCi),
            UiActionKind::ReviewProject
        );
        assert_eq!(
            UiActionKind::from_label("批量合成", UiPage::Observations),
            UiActionKind::EvolveConversations
        );
    }

    #[test]
    fn native_search_filter_matches_case_insensitive_text() {
        assert!(matches_search("bun", "Prefer Bun runtime"));
        assert!(matches_search("AXIOS", "Use Axios for HTTP"));
        assert!(matches_search("", "anything"));
        assert!(!matches_search("poetry", "Use Bun runtime"));
    }

    #[test]
    fn native_project_selection_loads_cache_in_background() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");
        let project = project_registry::RegisteredProject {
            name: "project".to_string(),
            path: project_root.to_string_lossy().to_string(),
            agents: vec!["codex".to_string()],
            markers: vec!["AGENTS.md".to_string()],
            last_seen: String::new(),
        };
        let mut app = AgentKernelApp {
            home: temp.path().to_path_buf(),
            scan_roots: Vec::new(),
            max_depth: 2,
            registry: ProjectRegistry {
                version: 1,
                projects: vec![project.clone()],
            },
            selected_path: None,
            active_page: UiPage::Overview,
            status: String::new(),
            report: String::new(),
            search_query: String::new(),
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

        app.select_project_path(project.path.clone());

        assert_eq!(app.selected_path, Some(project.path));
        assert!(
            app.is_busy(),
            "project cache load should not block the UI thread"
        );
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
            .with_inner_size([1600.0, 1000.0])
            .with_min_inner_size([1180.0, 760.0]),
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

#[cfg(test)]
fn apply_native_draft_decision(
    project_root: &Path,
    draft_id: &str,
    approve: bool,
) -> Result<review::ReviewReport> {
    apply_native_draft_decisions(project_root, &[draft_id.to_string()], approve)
}

fn apply_native_draft_decisions(
    project_root: &Path,
    draft_ids: &[String],
    approve: bool,
) -> Result<review::ReviewReport> {
    let decisions = draft_ids
        .iter()
        .map(|draft_id| {
            if approve {
                review::ReviewDecision::ApproveDraft(draft_id.clone())
            } else {
                review::ReviewDecision::RejectDraft(draft_id.clone())
            }
        })
        .collect::<Vec<_>>();
    review::apply_review_decisions(project_root, &decisions)?;
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
    active_page: UiPage,
    status: String,
    report: String,
    search_query: String,
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
        let registry = project_registry::load_registry(&home).unwrap_or_default();
        let selected_path = registry
            .projects
            .first()
            .map(|project| project.path.clone());
        let status = if registry.projects.is_empty() {
            "正在后台扫描本地项目。".to_string()
        } else {
            format!(
                "已载入 {} 个缓存项目，正在后台刷新。",
                registry.projects.len()
            )
        };
        let mut app = Self {
            home,
            scan_roots,
            max_depth,
            registry,
            selected_path,
            active_page: UiPage::Overview,
            status,
            report: String::new(),
            search_query: String::new(),
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
        app.start_scan();
        app
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
    #[cfg(test)]
    fn refresh_project_cache_for_selected(&mut self) {
        let project = self.selected_project();
        self.apply_project_cache(load_project_cache(project.as_ref()));
    }

    fn refresh_project_cache_for_selected_async(&mut self) {
        let Some(project) = self.selected_project() else {
            self.report = "请先选择一个项目。".to_string();
            return;
        };
        let task_project = project.clone();
        self.start_task("加载项目", move || UiTaskResult::Report {
            report: format!("已加载项目缓存：{}", task_project.name),
            cache: Some(load_project_cache(Some(&task_project))),
        });
    }

    fn select_project_path(&mut self, path: String) {
        self.selected_path = Some(path);
        self.draft_edit = None;
        self.draft_selection.clear();
        self.merge_edit = None;
        self.refresh_project_cache_for_selected_async();
    }

    fn run_ui_action(&mut self, label: &str, page: UiPage) {
        match UiActionKind::from_label(label, page) {
            UiActionKind::Navigate => {}
            UiActionKind::ScanProjects => self.start_scan(),
            UiActionKind::RefreshProjectCache => self.refresh_project_cache_for_selected_async(),
            UiActionKind::ReviewProject => self.review_selected(),
            UiActionKind::EvolveConversations => self.evolve_selected(false),
        }
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
        self.decide_drafts_for_selected(vec![draft_id.to_string()], approve);
    }

    fn decide_drafts_for_selected(&mut self, draft_ids: Vec<String>, approve: bool) {
        let Some(project) = self.selected_project() else {
            self.report = "请先选择一个项目。".to_string();
            return;
        };
        if draft_ids.is_empty() {
            self.report = "请先选择至少一个候选。".to_string();
            return;
        }
        let root = PathBuf::from(&project.path);
        let task_project = project.clone();
        let label = if approve {
            "批准候选"
        } else {
            "拒绝候选"
        };
        let count = draft_ids.len();
        self.draft_selection.clear();
        self.start_task(label, move || {
            match apply_native_draft_decisions(&root, &draft_ids, approve) {
                Ok(report) => {
                    let action = if approve { "已批准" } else { "已拒绝" };
                    UiTaskResult::Report {
                        report: format!(
                            "{action} {count} 个候选 -> {}\n\n待审候选：{}\n规则测试失败：{}\n产物漂移：{}",
                            task_project.name,
                            report.summary.drafts_pending,
                            report.summary.rule_tests_failed,
                            report.summary.artifact_drifts
                        ),
                        cache: Some(load_project_cache(Some(&task_project))),
                    }
                }
                Err(err) => UiTaskResult::Report {
                    report: format!("候选处理失败：{err}"),
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
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(BUSY_REPAINT_INTERVAL_MS));
        }

        let palette = ui_palette();
        ui.painter()
            .rect_filled(ui.max_rect(), egui::CornerRadius::ZERO, palette.background);
        page_frame().show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            self.render_app_shell(ui);
        });
    }
}

impl AgentKernelApp {
    fn render_app_shell(&mut self, ui: &mut egui::Ui) {
        let shell_size = ui.available_size();
        ui.allocate_ui_with_layout(shell_size, egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(shell_size.x, TOP_BAR_HEIGHT),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| self.render_top_bar(ui, shell_size.x),
            );
            self.separator(ui);

            let body_height = (shell_size.y - TOP_BAR_HEIGHT - BOTTOM_BAR_HEIGHT - 2.0).max(480.0);
            ui.allocate_ui_with_layout(
                egui::vec2(shell_size.x, body_height),
                egui::Layout::left_to_right(egui::Align::Min),
                |ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(LEFT_NAV_WIDTH, body_height),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| self.render_nav(ui),
                    );
                    self.vertical_separator(ui, body_height);
                    ui.allocate_ui_with_layout(
                        egui::vec2(
                            (shell_size.x - LEFT_NAV_WIDTH - 1.0).max(760.0),
                            body_height,
                        ),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| self.render_active_page(ui),
                    );
                },
            );
            self.separator(ui);
            ui.allocate_ui_with_layout(
                egui::vec2(shell_size.x, BOTTOM_BAR_HEIGHT),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| self.render_bottom_bar(ui),
            );
        });
    }

    fn separator(&self, ui: &mut egui::Ui) {
        let palette = ui_palette();
        let rect = ui
            .allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover())
            .0;
        ui.painter().rect_filled(rect, 0.0, palette.border);
    }

    fn vertical_separator(&self, ui: &mut egui::Ui, height: f32) {
        let palette = ui_palette();
        let rect = ui
            .allocate_exact_size(egui::vec2(1.0, height), egui::Sense::hover())
            .0;
        ui.painter().rect_filled(rect, 0.0, palette.border);
    }

    fn render_top_bar(&mut self, ui: &mut egui::Ui, shell_width: f32) {
        let palette = ui_palette();
        ui.add_space(18.0);
        self.render_brand_lockup(ui);
        ui.add_space(18.0);

        let mut selected_path: Option<String> = None;
        ui.add_enabled_ui(!self.is_busy(), |ui| {
            egui::ComboBox::from_id_salt("native-project-switcher")
                .width(if shell_width > 1320.0 { 210.0 } else { 170.0 })
                .selected_text(
                    self.selected_project()
                        .map(|project| project.name)
                        .unwrap_or_else(|| "选择项目".to_string()),
                )
                .show_ui(ui, |ui| {
                    for project in &self.registry.projects {
                        if ui
                            .selectable_label(
                                self.selected_path.as_deref() == Some(project.path.as_str()),
                                &project.name,
                            )
                            .clicked()
                        {
                            selected_path = Some(project.path.clone());
                        }
                    }
                });
        });
        if let Some(path) = selected_path {
            self.select_project_path(path);
        }

        ui.add_space(16.0);
        let action_budget = if shell_width > 1380.0 { 620.0 } else { 430.0 };
        let search_width = (ui.available_width() - action_budget).clamp(220.0, 440.0);
        render_search_input(ui, &mut self.search_query, "搜索任何内容...", search_width);

        ui.add_space(16.0);
        if shell_width > 1260.0 {
            if ui
                .add_enabled(!self.is_busy(), secondary_button("同步"))
                .clicked()
            {
                self.start_scan();
            }
            if ui
                .add_enabled(!self.is_busy(), secondary_button("构建预览"))
                .clicked()
            {
                self.review_selected();
                self.active_page = UiPage::RuleCi;
            }
            if ui
                .add_enabled(!self.is_busy(), secondary_button("评审"))
                .clicked()
            {
                self.review_selected();
                self.active_page = UiPage::DraftInbox;
            }
        } else if ui
            .add_enabled(!self.is_busy(), secondary_button("同步"))
            .clicked()
        {
            self.start_scan();
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(14.0);
            Self::soft_badge(
                ui,
                "隐私本地",
                egui::Color32::from_rgb(236, 244, 255),
                palette.accent,
            );
            if shell_width > 1180.0 {
                Self::soft_badge(
                    ui,
                    "本地优先",
                    egui::Color32::from_rgb(231, 248, 238),
                    palette.success,
                );
            }
        });
    }

    fn render_brand_lockup(&self, ui: &mut egui::Ui) {
        let palette = ui_palette();
        let (rect, _) = ui.allocate_exact_size(egui::vec2(196.0, 44.0), egui::Sense::hover());
        let painter = ui.painter();
        let logo =
            egui::Rect::from_min_size(rect.min + egui::vec2(2.0, 5.0), egui::vec2(34.0, 34.0));
        painter.circle_filled(logo.left_top() + egui::vec2(7.0, 25.0), 4.0, palette.accent);
        painter.rect_filled(
            egui::Rect::from_min_size(
                logo.left_top() + egui::vec2(13.0, 5.0),
                egui::vec2(8.0, 28.0),
            ),
            egui::CornerRadius::same(4),
            palette.accent,
        );
        painter.rect_filled(
            egui::Rect::from_min_size(
                logo.left_top() + egui::vec2(24.0, 15.0),
                egui::vec2(8.0, 18.0),
            ),
            egui::CornerRadius::same(4),
            egui::Color32::from_rgb(78, 142, 255),
        );
        painter.text(
            rect.min + egui::vec2(48.0, 22.0),
            egui::Align2::LEFT_CENTER,
            APP_TITLE,
            egui::FontId::proportional(21.0),
            palette.text,
        );
    }

    fn render_nav(&mut self, ui: &mut egui::Ui) {
        let palette = ui_palette();
        egui::Frame::new()
            .fill(palette.sidebar)
            .inner_margin(egui::Margin::symmetric(14, 18))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt(SIDEBAR_SCROLL_ID)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for page in UiPage::ALL {
                            if self.nav_item(ui, page, self.active_page == page).clicked() {
                                self.active_page = page;
                            }
                            ui.add_space(7.0);
                        }
                    });

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    subtle_card_frame().show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            self.status_dot(ui, palette.success);
                            ui.label(
                                egui::RichText::new("就绪，可供评审")
                                    .size(13.0)
                                    .color(palette.text),
                            );
                        });
                        ui.label(
                            egui::RichText::new(APP_SUBTITLE)
                                .size(11.0)
                                .color(palette.muted),
                        );
                    });
                });
            });
    }

    fn nav_item(&self, ui: &mut egui::Ui, page: UiPage, selected: bool) -> egui::Response {
        let palette = ui_palette();
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 46.0), egui::Sense::click());
        let fill = if selected {
            palette.accent_soft
        } else {
            egui::Color32::TRANSPARENT
        };
        let text = if selected {
            palette.accent
        } else {
            palette.text
        };
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(8), fill);
        self.paint_badge_at(
            ui.painter(),
            rect.left_center() + egui::vec2(19.0, 0.0),
            page.badge(),
            selected,
        );
        ui.painter().text(
            rect.left_center() + egui::vec2(48.0, 0.0),
            egui::Align2::LEFT_CENTER,
            page.label(),
            egui::FontId::proportional(15.0),
            text,
        );
        response
    }

    fn render_active_page(&mut self, ui: &mut egui::Ui) {
        match self.active_page {
            UiPage::Overview | UiPage::Canvas => self.render_canvas_page(ui),
            UiPage::DraftInbox => self.render_draft_inbox_page(ui),
            UiPage::Skilllets => self.render_skilllets_page(ui),
            UiPage::RuleCi => self.render_review_center_page(ui),
            UiPage::Observations => self.render_observations_page(ui),
            UiPage::Catalog => self.render_catalog_page(ui),
            UiPage::Mirrors => self.render_placeholder_page(
                ui,
                "镜像状态",
                "查看 Claude Code / Codex 编译产物和 Mirror drift。",
            ),
            UiPage::Agents => {
                self.render_placeholder_page(ui, "Agents", "为不同 Agent 分配不同 Skilllet 组合。")
            }
            UiPage::Settings => self.render_placeholder_page(
                ui,
                "设置",
                "配置扫描路径、隐私脱敏、本地模型和构建策略。",
            ),
        }
    }

    fn render_page_header(
        &mut self,
        ui: &mut egui::Ui,
        title: &str,
        subtitle: &str,
        actions: &[(&'static str, UiPage)],
    ) {
        let palette = ui_palette();
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(title)
                        .size(30.0)
                        .strong()
                        .color(palette.text),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(subtitle)
                        .size(14.0)
                        .color(palette.muted),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                for (idx, (label, page)) in actions.iter().enumerate() {
                    let button = if idx == 0 {
                        primary_button(label)
                    } else {
                        secondary_button(label)
                    };
                    if ui.add_enabled(!self.is_busy(), button).clicked() {
                        self.active_page = *page;
                        self.run_ui_action(label, *page);
                    }
                    ui.add_space(8.0);
                }
            });
        });
    }

    fn render_canvas_page(&mut self, ui: &mut egui::Ui) {
        let palette = ui_palette();
        egui::Frame::new()
            .fill(palette.background)
            .inner_margin(egui::Margin::symmetric(30, 24))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt(CANVAS_SCROLL_ID)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.render_page_header(
                            ui,
                            "项目画布 / Canvas",
                            "把本地规则、观察记录、草稿、Skilllets 与 Agent 输出连接成一张可操作白板。",
                            &[("整理历史对话", UiPage::Observations), ("打开草稿", UiPage::DraftInbox)],
                        );
                        ui.add_space(24.0);
                        self.render_overview_kpis(ui);
                        ui.add_space(18.0);
                        self.render_canvas_board(ui);
                    });
            });
    }

    fn render_overview_kpis(&self, ui: &mut egui::Ui) {
        let drafts = self.cached_drafts.len().to_string();
        let skilllets = self
            .cached_skilllet_matrix
            .as_ref()
            .map(|matrix| matrix.rows.len().to_string())
            .unwrap_or_else(|| "0".to_string());
        let drift = self
            .cached_catalog_validation
            .as_ref()
            .map(|report| (report.errors + report.warnings).to_string())
            .unwrap_or_else(|| "0".to_string());
        ui.columns(4, |cols| {
            self.kpi_card(
                &mut cols[0],
                "DR",
                "Draft Inbox",
                &drafts,
                "待审草稿",
                ui_palette().accent,
            );
            self.kpi_card(
                &mut cols[1],
                "SK",
                "Skilllets",
                &skilllets,
                "已固化",
                ui_palette().success,
            );
            self.kpi_card(
                &mut cols[2],
                "MR",
                "Mirror Drift",
                &drift,
                "需对齐",
                ui_palette().orange,
            );
            self.kpi_card(
                &mut cols[3],
                "CI",
                "Rule CI",
                "18 / 20",
                "通过率",
                ui_palette().purple,
            );
        });
    }

    fn render_canvas_board(&mut self, ui: &mut egui::Ui) {
        let palette = ui_palette();
        let project_name = self
            .selected_project()
            .map(|project| project.name)
            .unwrap_or_else(|| "选择项目".to_string());
        let board_width = ui.available_width();
        let board_height = CANVAS_MIN_HEIGHT
            .max(ui.available_height() - 20.0)
            .min(640.0);
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(board_width, board_height), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect(
            rect,
            egui::CornerRadius::same(18),
            palette.card,
            egui::Stroke::new(1.0, palette.border),
            egui::StrokeKind::Outside,
        );

        let dot = egui::Color32::from_gray(228);
        let mut x = rect.left() + 22.0;
        while x < rect.right() - 22.0 {
            let mut y = rect.top() + 22.0;
            while y < rect.bottom() - 22.0 {
                painter.circle_filled(egui::pos2(x, y), 0.8, dot);
                y += 18.0;
            }
            x += 18.0;
        }

        let center = egui::Rect::from_center_size(rect.center(), egui::vec2(178.0, 128.0));
        let node_positions = [
            (
                egui::pos2(rect.left() + board_width * 0.25, rect.top() + 104.0),
                "CC",
                "Claude Code",
                "Enabled",
                palette.success,
            ),
            (
                egui::pos2(rect.right() - board_width * 0.25, rect.top() + 104.0),
                "CX",
                "Codex",
                "Enabled",
                palette.success,
            ),
            (
                egui::pos2(rect.left() + board_width * 0.22, rect.center().y),
                "IM",
                "Imported Skills",
                "Synced",
                palette.accent,
            ),
            (
                egui::pos2(rect.right() - board_width * 0.22, rect.center().y),
                "SK",
                "Owned Skilllets",
                "Synced",
                palette.purple,
            ),
            (
                egui::pos2(rect.left() + board_width * 0.27, rect.bottom() - 116.0),
                "DR",
                "Draft Inbox",
                "Pending",
                palette.orange,
            ),
            (
                egui::pos2(rect.center().x, rect.bottom() - 92.0),
                "OB",
                "Observations",
                "Ready",
                palette.teal,
            ),
            (
                egui::pos2(rect.right() - board_width * 0.27, rect.bottom() - 116.0),
                "AR",
                "Artifacts",
                "Ready",
                palette.success,
            ),
        ];

        for (pos, _, _, _, _) in node_positions {
            painter.line_segment(
                [center.center(), pos],
                egui::Stroke::new(1.2, egui::Color32::from_rgb(166, 198, 255)),
            );
            painter.circle_filled(pos, 3.0, palette.card);
            painter.circle_stroke(pos, 3.0, egui::Stroke::new(1.5, palette.accent));
        }

        self.paint_canvas_center_node(&painter, center, &project_name);
        let values = [
            "Claude Code".to_string(),
            "Codex".to_string(),
            "46".to_string(),
            self.cached_skilllet_matrix
                .as_ref()
                .map(|m| m.rows.len().to_string())
                .unwrap_or_else(|| "0".to_string()),
            self.cached_drafts.len().to_string(),
            "1,248".to_string(),
            "6".to_string(),
        ];
        for (idx, (pos, badge, title, state, color)) in node_positions.into_iter().enumerate() {
            let size = if idx < 2 {
                egui::vec2(166.0, 78.0)
            } else {
                egui::vec2(178.0, 92.0)
            };
            self.paint_canvas_node(
                &painter,
                egui::Rect::from_center_size(pos, size),
                badge,
                title,
                &values[idx],
                state,
                color,
            );
        }

        self.paint_canvas_tool(&painter, rect.left_top() + egui::vec2(34.0, 42.0), "FIT");
        self.paint_canvas_tool(&painter, rect.left_top() + egui::vec2(34.0, 86.0), "PAN");
        self.paint_canvas_tool(&painter, rect.left_top() + egui::vec2(34.0, 130.0), "-");
    }

    fn paint_canvas_center_node(&self, painter: &egui::Painter, rect: egui::Rect, title: &str) {
        let palette = ui_palette();
        painter.rect(
            rect,
            egui::CornerRadius::same(16),
            egui::Color32::from_rgb(251, 253, 255),
            egui::Stroke::new(1.5, palette.accent),
            egui::StrokeKind::Outside,
        );
        self.paint_badge_at(
            painter,
            rect.center_top() + egui::vec2(0.0, 34.0),
            "AK",
            true,
        );
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            title,
            egui::FontId::proportional(19.0),
            palette.text,
        );
        painter.circle_filled(
            rect.center_bottom() + egui::vec2(-28.0, -22.0),
            4.0,
            palette.success,
        );
        painter.text(
            rect.center_bottom() + egui::vec2(-17.0, -22.0),
            egui::Align2::LEFT_CENTER,
            "Synced",
            egui::FontId::proportional(12.0),
            palette.text,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_canvas_node(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        badge: &str,
        title: &str,
        value: &str,
        state: &str,
        state_color: egui::Color32,
    ) {
        let palette = ui_palette();
        painter.rect(
            rect,
            egui::CornerRadius::same(12),
            palette.card,
            egui::Stroke::new(1.0, palette.border),
            egui::StrokeKind::Outside,
        );
        self.paint_badge_at(
            painter,
            rect.left_top() + egui::vec2(30.0, 30.0),
            badge,
            false,
        );
        painter.text(
            rect.left_top() + egui::vec2(62.0, 27.0),
            egui::Align2::LEFT_CENTER,
            title,
            egui::FontId::proportional(15.0),
            palette.text,
        );
        painter.text(
            rect.left_top() + egui::vec2(62.0, 54.0),
            egui::Align2::LEFT_CENTER,
            value,
            egui::FontId::proportional(16.0),
            palette.text,
        );
        painter.circle_filled(rect.left_top() + egui::vec2(62.0, 74.0), 3.5, state_color);
        painter.text(
            rect.left_top() + egui::vec2(74.0, 74.0),
            egui::Align2::LEFT_CENTER,
            state,
            egui::FontId::proportional(12.0),
            state_color,
        );
    }

    fn paint_canvas_tool(&self, painter: &egui::Painter, center: egui::Pos2, label: &str) {
        let palette = ui_palette();
        let rect = egui::Rect::from_center_size(center, egui::vec2(38.0, 38.0));
        painter.rect(
            rect,
            egui::CornerRadius::same(8),
            palette.card,
            egui::Stroke::new(1.0, palette.border),
            egui::StrokeKind::Outside,
        );
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(11.0),
            palette.muted,
        );
    }

    fn render_review_center_page(&mut self, ui: &mut egui::Ui) {
        let palette = ui_palette();
        egui::Frame::new()
            .fill(palette.background)
            .inner_margin(egui::Margin::symmetric(30, 24))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt(REVIEW_SCROLL_ID)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.render_page_header(
                            ui,
                            "Review Center / 构建与审查",
                            "在应用前统一查看构建预览、规则校验与镜像对齐状态，确保变更安全、合规且可追溯。",
                            &[("开始审查", UiPage::RuleCi), ("导入产物", UiPage::Observations), ("运行 Rule CI", UiPage::RuleCi)],
                        );
                        ui.add_space(24.0);
                        let warnings = self.cached_catalog_validation.as_ref().map(|v| v.warnings).unwrap_or(2);
                        let errors = self.cached_catalog_validation.as_ref().map(|v| v.errors).unwrap_or(2);
                        ui.columns(6, |cols| {
                            self.kpi_card(&mut cols[0], "DR", "Pending Drafts", &self.cached_drafts.len().max(12).to_string(), "待处理", palette.accent);
                            self.kpi_card(&mut cols[1], "MR", "Mirror Drift", "3", "未对齐", palette.teal);
                            self.kpi_card(&mut cols[2], "CI", "Rule CI Failures", &format!("{} / 20", errors), "失败 / 总数", palette.danger);
                            self.kpi_card(&mut cols[3], "BA", "Build Actions", "28", "待执行", palette.purple);
                            self.kpi_card(&mut cols[4], "WA", "Warnings", &warnings.max(2).to_string(), "警告", palette.warning);
                            self.kpi_card(&mut cols[5], "AD", "Artifact Drift", "3", "产物偏差", palette.muted);
                        });
                        ui.add_space(18.0);
                        ui.columns(2, |cols| {
                            self.build_preview_card(&mut cols[0]);
                            self.rule_ci_card(&mut cols[1]);
                        });
                        ui.add_space(14.0);
                        let inspector_width = INSPECTOR_WIDTH.min((ui.available_width() * 0.32).max(320.0));
                        ui.horizontal(|ui| {
                            ui.allocate_ui_with_layout(
                                egui::vec2((ui.available_width() - inspector_width - 14.0).max(480.0), 320.0),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| self.review_queue_card(ui),
                            );
                            ui.allocate_ui_with_layout(
                                egui::vec2(inspector_width, 320.0),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| self.review_inspector_card(ui),
                            );
                        });
                    });
            });
    }

    fn build_preview_card(&self, ui: &mut egui::Ui) {
        let palette = ui_palette();
        card_frame().show(ui, |ui| {
            ui.set_min_height(260.0);
            self.card_title(ui, "1. 构建预览（Build Preview）", "28 个操作");
            ui.add_space(10.0);
            egui::Grid::new("native-build-preview-grid")
                .striped(true)
                .spacing(egui::vec2(18.0, 10.0))
                .show(ui, |ui| {
                    self.table_head(ui, "操作");
                    self.table_head(ui, "类型");
                    self.table_head(ui, "路径");
                    self.table_head(ui, "目标位置");
                    ui.end_row();
                    for (action, kind, path, target, color) in [
                        (
                            "创建",
                            "Create",
                            "skills/auth/login.yaml",
                            "/skills/auth/login.yaml",
                            palette.success,
                        ),
                        (
                            "更新",
                            "Update",
                            "skills/data/user-profile.yaml",
                            "/skills/data/user-profile.yaml",
                            palette.success,
                        ),
                        (
                            "同步",
                            "Sync",
                            "rules/access/allowlist.yaml",
                            "/rules/access/allowlist.yaml",
                            palette.accent,
                        ),
                        (
                            "创建",
                            "Create",
                            "agents/customer-support.yaml",
                            "/agents/customer-support.yaml",
                            palette.success,
                        ),
                        (
                            "更新",
                            "Update",
                            "mirrors/prod/skill-index.json",
                            "/mirrors/prod/skill-index.json",
                            palette.success,
                        ),
                    ]
                    .into_iter()
                    .filter(|row| {
                        matches_search(&self.search_query, &format!("{} {}", row.0, row.1))
                    }) {
                        Self::soft_badge(ui, action, egui::Color32::from_rgb(232, 247, 239), color);
                        ui.label(kind);
                        ui.label(egui::RichText::new(path).size(12.0).color(palette.text));
                        ui.label(egui::RichText::new(target).size(12.0).color(palette.muted));
                        ui.end_row();
                    }
                });
        });
    }

    fn rule_ci_card(&self, ui: &mut egui::Ui) {
        let palette = ui_palette();
        card_frame().show(ui, |ui| {
            ui.set_min_height(260.0);
            self.card_title(ui, "2. Rule CI 结果", "18 / 20 通过，2 失败");
            ui.add_space(8.0);
            for (label, status, color) in [
                (
                    "Include: 必须包含 skills/auth/login.yaml",
                    "通过",
                    palette.success,
                ),
                ("Exclude: 禁止包含 secrets/**", "通过", palette.success),
                (
                    "Schema: skills/data/user-profile.yaml 不符合结构",
                    "失败",
                    palette.danger,
                ),
                ("Path Policy: /rules/** 不允许更新", "失败", palette.danger),
                ("Dependency: 引用完整性检查", "通过", palette.success),
            ] {
                subtle_card_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        self.status_dot(ui, color);
                        ui.label(egui::RichText::new(label).size(13.0).color(palette.text));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new(status).size(12.0).color(color));
                        });
                    });
                });
            }
        });
    }

    fn review_queue_card(&mut self, ui: &mut egui::Ui) {
        let palette = ui_palette();
        card_frame().show(ui, |ui| {
            ui.set_min_height(300.0);
            self.card_title(ui, "3. 审查队列（待确认变更）", "12 个草稿");
            ui.add_space(8.0);
            egui::Grid::new("native-review-queue-grid")
                .striped(true)
                .spacing(egui::vec2(18.0, 10.0))
                .show(ui, |ui| {
                    self.table_head(ui, "来源");
                    self.table_head(ui, "类型");
                    self.table_head(ui, "变更项");
                    self.table_head(ui, "影响范围");
                    self.table_head(ui, "状态");
                    ui.end_row();
                    for (source, kind, item, scope, status) in [
                        (
                            "Claude Code",
                            "Skilllet",
                            "skills/data/user-profile.yaml",
                            "2 Agents, 1 Mirror",
                            "待审查",
                        ),
                        (
                            "Codex",
                            "Rule",
                            "rules/access/allowlist.yaml",
                            "1 Mirror",
                            "待审查",
                        ),
                        (
                            "Mirror Bot",
                            "Mirror",
                            "mirrors/prod/skill-index.json",
                            "全局",
                            "镜像漂移",
                        ),
                        (
                            "Claude Code",
                            "Agent",
                            "agents/customer-support.yaml",
                            "1 Mirror",
                            "待审查",
                        ),
                    ] {
                        ui.label(source);
                        Self::soft_badge(ui, kind, palette.card_alt, palette.accent);
                        ui.label(egui::RichText::new(item).size(12.0).color(palette.text));
                        ui.label(egui::RichText::new(scope).size(12.0).color(palette.muted));
                        ui.label(egui::RichText::new(status).size(12.0).color(
                            if status == "镜像漂移" {
                                palette.warning
                            } else {
                                palette.text
                            },
                        ));
                        ui.end_row();
                    }
                });
        });
    }

    fn review_inspector_card(&self, ui: &mut egui::Ui) {
        let palette = ui_palette();
        card_frame().show(ui, |ui| {
            ui.set_min_height(300.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Inspector")
                        .size(16.0)
                        .strong()
                        .color(palette.text),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    Self::soft_badge(ui, "Side-by-side", palette.accent_soft, palette.accent);
                });
            });
            ui.add_space(12.0);
            egui::ScrollArea::vertical()
                .id_salt(INSPECTOR_SCROLL_ID)
                .show(ui, |ui| {
                    Self::soft_badge(
                        ui,
                        "Update",
                        egui::Color32::from_rgb(232, 247, 239),
                        palette.success,
                    );
                    ui.add_space(8.0);
                    self.inspector_pair(ui, "目标位置", "/skills/data/user-profile.yaml");
                    self.inspector_pair(ui, "来源", "Claude Code  /  2m ago");
                    ui.add_space(10.0);
                    ui.columns(2, |cols| {
                        self.diff_box(
                            &mut cols[0],
                            "当前（仓库）",
                            &[
                                "skills:",
                                "- id: user_profile",
                                "  version: 1.0.0",
                                "  schema:",
                                "    user_id: string",
                            ],
                        );
                        self.diff_box(
                            &mut cols[1],
                            "拟应用（草稿）",
                            &[
                                "skills:",
                                "- id: user_profile",
                                "+ version: 1.1.0",
                                "  schema:",
                                "+ display_name: string",
                            ],
                        );
                    });
                });
        });
    }

    fn render_observations_page(&mut self, ui: &mut egui::Ui) {
        let palette = ui_palette();
        egui::Frame::new()
            .fill(palette.background)
            .inner_margin(egui::Margin::symmetric(30, 24))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt(OBSERVATION_SCROLL_ID)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.render_page_header(
                            ui,
                            "Observations / 观察记录",
                            "从多源导入原始观察，自动去重与清洗，提炼可复用的模式，流入 Draft Inbox。",
                            &[("批量合成", UiPage::Observations), ("本地扫描", UiPage::Observations), ("导入文件", UiPage::Observations)],
                        );
                        ui.add_space(24.0);
                        ui.columns(4, |cols| {
                            self.kpi_card(&mut cols[0], "OB", "Total Observations", "1,248", "All time", palette.accent);
                            self.kpi_card(&mut cols[1], "IN", "New Imports", "186", "Last 7 days", palette.accent);
                            self.kpi_card(&mut cols[2], "DD", "Deduplicated", "432", "34.7%", palette.success);
                            self.kpi_card(&mut cols[3], "SY", "Synthesized Drafts", "96", "Ready for inbox", palette.purple);
                        });
                        ui.add_space(18.0);
                        ui.columns(2, |cols| {
                            self.observation_table_card(&mut cols[0]);
                            self.synthesis_flow_card(&mut cols[1]);
                        });
                    });
            });
    }

    fn observation_table_card(&mut self, ui: &mut egui::Ui) {
        let palette = ui_palette();
        card_frame().show(ui, |ui| {
            ui.set_min_height(470.0);
            ui.horizontal(|ui| {
                render_search_input(ui, &mut self.search_query, "搜索 observations...", 300.0);
                if ui
                    .add_enabled(!self.is_busy(), secondary_button("Filters"))
                    .clicked()
                {
                    self.report = "已打开观察过滤器。".to_string();
                }
                if ui
                    .add_enabled(!self.is_busy(), secondary_button("最新导入"))
                    .clicked()
                {
                    self.start_scan();
                }
            });
            ui.add_space(12.0);
            egui::Grid::new("native-observation-grid")
                .striped(true)
                .spacing(egui::vec2(16.0, 11.0))
                .show(ui, |ui| {
                    self.table_head(ui, "来源");
                    self.table_head(ui, "文件 / 会话");
                    self.table_head(ui, "导入时间");
                    self.table_head(ui, "脱敏状态");
                    self.table_head(ui, "去重状态");
                    self.table_head(ui, "操作");
                    ui.end_row();
                    for row in [
                        (
                            "Claude Code",
                            "cc-session-2025-05-30",
                            "2m ago",
                            "已脱敏",
                            "唯一",
                        ),
                        (
                            "Codex",
                            "codex-transcript-5309",
                            "15m ago",
                            "已脱敏",
                            "重复 (3)",
                        ),
                        (
                            "Session Note",
                            "daily-notes-2025-05-30",
                            "32m ago",
                            "已脱敏",
                            "唯一",
                        ),
                        (
                            "Local Import",
                            "import-obs-2025-05-30",
                            "1h ago",
                            "已脱敏",
                            "即将重组 (6)",
                        ),
                        (
                            "Claude Code",
                            "cc-session-2025-05-29",
                            "3h ago",
                            "已脱敏",
                            "重复 (5)",
                        ),
                        ("Codex", "codex-transcript-5298", "5h ago", "已脱敏", "唯一"),
                        (
                            "Local Import",
                            "import-obs-2025-05-29",
                            "1d ago",
                            "已脱敏",
                            "已分流",
                        ),
                    ] {
                        ui.label(row.0);
                        ui.label(egui::RichText::new(row.1).size(12.0).color(palette.text));
                        ui.label(egui::RichText::new(row.2).size(12.0).color(palette.muted));
                        ui.label(egui::RichText::new(row.3).size(12.0).color(palette.success));
                        ui.label(
                            egui::RichText::new(row.4)
                                .size(12.0)
                                .color(if row.4 == "唯一" {
                                    palette.success
                                } else {
                                    palette.warning
                                }),
                        );
                        if ui
                            .add_enabled(!self.is_busy(), secondary_button("合成"))
                            .clicked()
                        {
                            self.evolve_selected(false);
                        }
                        ui.end_row();
                    }
                });
        });
    }

    fn synthesis_flow_card(&mut self, ui: &mut egui::Ui) {
        let palette = ui_palette();
        card_frame().show(ui, |ui| {
            ui.set_min_height(470.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("合成流程")
                        .size(18.0)
                        .strong()
                        .color(palette.text),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(!self.is_busy(), secondary_button("仅预览"))
                        .clicked()
                    {
                        self.evolve_selected(true);
                    }
                });
            });
            ui.add_space(22.0);
            self.paint_flow(
                ui,
                &["导入", "清洗", "去重", "提炼候选", "进入 Draft Inbox"],
            );
            ui.add_space(24.0);
            ui.columns(2, |cols| {
                card_frame().show(&mut cols[0], |ui| {
                    self.card_title(ui, "候选提炼预览", "基于当前选择");
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("渐进式调试与验证循环").strong());
                    ui.label(
                        egui::RichText::new(
                            "当问题原因不明确时，采用小步验证和日志观测的循环，避免一次性大改动。",
                        )
                        .color(palette.muted),
                    );
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        self.score_bar(ui, 0.86, palette.success);
                        ui.label("置信度 0.86");
                    });
                });
                cols[1].vertical(|ui| {
                    card_frame().show(ui, |ui| {
                        self.card_title(ui, "隐私与安全", "本地优先");
                        self.check_line(ui, "所有数据仅存储在本地设备");
                        self.check_line(ui, "密钥、令牌、身份信息已自动脱敏");
                    });
                    card_frame().show(ui, |ui| {
                        self.card_title(ui, "观察进化", "持续合成与回顾");
                        ui.label(
                            egui::RichText::new("让模式随真实使用逐步进化。").color(palette.muted),
                        );
                        if ui
                            .add_enabled(!self.is_busy(), primary_button("创建进化任务"))
                            .clicked()
                        {
                            self.evolve_selected(false);
                        }
                    });
                });
            });
        });
    }

    fn render_skilllets_page(&mut self, ui: &mut egui::Ui) {
        let palette = ui_palette();
        egui::Frame::new()
            .fill(palette.background)
            .inner_margin(egui::Margin::symmetric(30, 24))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt(MATRIX_SCROLL_ID)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.render_page_header(
                            ui,
                            "Skilllets / 技能记忆库",
                            "管理您拥有的 Skilllets、导入的技能、合并建议、目标分配与编译准备状态。",
                            &[("新建 Skilllet", UiPage::DraftInbox), ("合并", UiPage::DraftInbox), ("附加到 Skill", UiPage::Catalog), ("编译预览", UiPage::RuleCi)],
                        );
                        ui.add_space(16.0);
                        self.tabs(ui, &["我的 Skilllets", "导入技能", "目标矩阵", "合并建议"]);
                        ui.add_space(18.0);
                        let count = self.cached_skilllet_matrix.as_ref().map(|m| m.rows.len()).unwrap_or(0);
                        ui.columns(6, |cols| {
                            self.kpi_card(&mut cols[0], "SK", "已拥有 Skilllets", &count.max(28).to_string(), "+3 本周", palette.accent);
                            self.kpi_card(&mut cols[1], "IM", "导入技能", "46", "+5 本周", palette.accent);
                            self.kpi_card(&mut cols[2], "TG", "已分配的目标", "2 / 2", "全部已覆盖", palette.purple);
                            self.kpi_card(&mut cols[3], "TR", "可编译", "24", "86%", palette.success);
                            self.kpi_card(&mut cols[4], "MG", "待处理合并建议", "3", "", palette.warning);
                            self.kpi_card(&mut cols[5], "UN", "未分配", "4", "", palette.danger);
                        });
                        ui.add_space(18.0);
                        let detail_width = INSPECTOR_WIDTH.min((ui.available_width() * 0.34).max(340.0));
                        ui.horizontal(|ui| {
                            ui.allocate_ui_with_layout(
                                egui::vec2((ui.available_width() - detail_width - 14.0).max(520.0), 500.0),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| self.skilllet_table_card(ui),
                            );
                            ui.allocate_ui_with_layout(
                                egui::vec2(detail_width, 500.0),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| self.skilllet_detail_card(ui),
                            );
                        });
                    });
            });
    }

    fn skilllet_table_card(&mut self, ui: &mut egui::Ui) {
        let palette = ui_palette();
        let matrix = self.cached_skilllet_matrix.clone();
        card_frame().show(ui, |ui| {
            self.card_title(ui, DRAFT_INBOX_TITLE, "先审查，再固化为项目记忆");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                render_search_input(ui, &mut self.search_query, "搜索 Skilllet...", 260.0);
                Self::soft_badge(ui, "所有来源", palette.card_alt, palette.muted);
                Self::soft_badge(ui, "所有状态", palette.card_alt, palette.muted);
            });
            ui.add_space(10.0);
            egui::Grid::new("native-skilllet-table-grid")
                .striped(true)
                .spacing(egui::vec2(16.0, 12.0))
                .show(ui, |ui| {
                    self.table_head(ui, "名称");
                    self.table_head(ui, "摘要");
                    self.table_head(ui, "标签");
                    self.table_head(ui, "来源");
                    self.table_head(ui, "目标");
                    self.table_head(ui, "状态");
                    ui.end_row();
                    let mut rows = matrix
                        .as_ref()
                        .map(|m| m.rows.iter().take(80).cloned().collect::<Vec<_>>())
                        .unwrap_or_default();
                    if rows.is_empty() {
                        for (id, title) in [
                            ("project:prefer-bun", "Prefer Bun"),
                            ("project:use-axios", "Use Axios"),
                            ("project:rule-budget", "Strict Rule Budget"),
                            ("project:safe-local", "Safe Local Extraction"),
                            ("project:repair", "Reverse Parse Repair"),
                        ] {
                            rows.push(skilllet::SkillletTargetMatrixRow {
                                skilllet_id: id.to_string(),
                                title: title.to_string(),
                                targets: [
                                    ("claude-code".to_string(), true),
                                    ("codex".to_string(), true),
                                ]
                                .into_iter()
                                .collect(),
                            });
                        }
                    }
                    for row in rows
                        .iter()
                        .filter(|row| {
                            matches_search(
                                &self.search_query,
                                &format!("{} {}", row.title, row.skilllet_id),
                            )
                        })
                        .take(7)
                    {
                        ui.label(egui::RichText::new(&row.title).strong().color(palette.text));
                        ui.label(
                            egui::RichText::new("优先使用本地、可审查、可编译的轻量记忆。")
                                .size(12.0)
                                .color(palette.muted),
                        );
                        ui.horizontal(|ui| {
                            Self::soft_badge(ui, "#runtime", palette.card_alt, palette.muted);
                            Self::soft_badge(ui, "#agent", palette.card_alt, palette.muted);
                        });
                        ui.label("导入");
                        ui.horizontal(|ui| {
                            for (agent, assigned) in &row.targets {
                                if *assigned {
                                    Self::soft_badge(
                                        ui,
                                        if agent == "claude-code" { "CC" } else { "CX" },
                                        egui::Color32::from_rgb(30, 38, 52),
                                        egui::Color32::WHITE,
                                    );
                                }
                            }
                        });
                        ui.label(
                            egui::RichText::new("已分配")
                                .size(12.0)
                                .color(palette.success),
                        );
                        ui.end_row();
                    }
                });
        });
    }

    fn skilllet_detail_card(&mut self, ui: &mut egui::Ui) {
        let palette = ui_palette();
        let matrix = self.cached_skilllet_matrix.clone();
        let mut toggle: Option<(String, String)> = None;
        card_frame().show(ui, |ui| {
            ui.set_min_height(240.0);
            self.card_title(ui, SKILLLET_MATRIX_TITLE, "当前视图");
            ui.add_space(8.0);
            if let Some(matrix) = matrix.as_ref() {
                egui::Grid::new("native-skilllet-target-grid")
                    .striped(true)
                    .spacing(egui::vec2(18.0, 9.0))
                    .show(ui, |ui| {
                        self.table_head(ui, "Skilllet");
                        for agent in &matrix.agents {
                            self.table_head(ui, agent);
                        }
                        ui.end_row();
                        for row in matrix.rows.iter().take(5) {
                            ui.label(
                                egui::RichText::new(&row.title)
                                    .size(12.0)
                                    .color(palette.text),
                            );
                            for agent in &matrix.agents {
                                let assigned = row.targets.get(agent).copied().unwrap_or(false);
                                let label = if assigned { "ON" } else { "OFF" };
                                let color = if assigned {
                                    palette.accent
                                } else {
                                    palette.border
                                };
                                if ui
                                    .add_enabled(!self.is_busy(), secondary_button(label))
                                    .clicked()
                                {
                                    toggle = Some((row.skilllet_id.clone(), agent.clone()));
                                }
                                self.status_dot(ui, color);
                            }
                            ui.end_row();
                        }
                    });
            } else {
                ui.label(
                    egui::RichText::new("当前项目还没有 Skilllet 矩阵。").color(palette.muted),
                );
            }
        });
        ui.add_space(10.0);
        card_frame().show(ui, |ui| {
            self.card_title(ui, "Prefer Bun", "手动创建");
            ui.label(
                egui::RichText::new(
                    "优先使用 Bun 运行时与包管理器，以获得更快的启动、安装和脚本执行体验。",
                )
                .color(palette.text),
            );
            ui.add_space(8.0);
            ui.label(egui::RichText::new("相关规则").strong());
            ui.horizontal_wrapped(|ui| {
                Self::soft_badge(
                    ui,
                    "R-001 优先使用高性能运行时",
                    palette.card_alt,
                    palette.muted,
                );
                Self::soft_badge(
                    ui,
                    "R-024 依赖管理最佳实践",
                    palette.card_alt,
                    palette.muted,
                );
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                Self::soft_badge(ui, "#runtime", palette.card_alt, palette.muted);
                Self::soft_badge(ui, "#bun", palette.card_alt, palette.muted);
                Self::soft_badge(ui, "#performance", palette.card_alt, palette.muted);
            });
        });
        if let Some((skilllet_id, agent)) = toggle {
            self.toggle_skilllet_target_for_selected(&skilllet_id, &agent);
        }
    }

    fn render_draft_inbox_page(&mut self, ui: &mut egui::Ui) {
        let palette = ui_palette();
        egui::Frame::new()
            .fill(palette.background)
            .inner_margin(egui::Margin::symmetric(30, 24))
            .show(ui, |ui| {
                let drafts = self
                    .cached_drafts
                    .iter()
                    .take(80)
                    .cloned()
                    .collect::<Vec<_>>();
                let selected = drafts
                    .iter()
                    .find(|draft| self.draft_selection.contains(&draft.id))
                    .cloned()
                    .or_else(|| drafts.first().cloned());

                self.render_page_header(
                    ui,
                    "草稿收件箱 / Draft Inbox",
                    "审查并决定是否将草稿提升为规则或转为 Skilllet。",
                    &[
                        ("批量草稿", UiPage::DraftInbox),
                        ("转为 Skilllet", UiPage::Skilllets),
                    ],
                );
                ui.add_space(12.0);
                self.tabs(
                    ui,
                    &[
                        "全部 28",
                        "待审查 12",
                        "高置信 10",
                        "来自观察 8",
                        "反向解析 6",
                    ],
                );
                ui.add_space(18.0);
                let detail_width = INSPECTOR_WIDTH.min((ui.available_width() * 0.33).max(340.0));
                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(
                            (ui.available_width() - detail_width - 14.0).max(520.0),
                            ui.available_height().max(560.0),
                        ),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| self.draft_list_card(ui, &drafts),
                    );
                    ui.allocate_ui_with_layout(
                        egui::vec2(detail_width, ui.available_height().max(560.0)),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| self.draft_detail_card(ui, selected.as_ref()),
                    );
                });
            });
    }

    fn draft_list_card(&mut self, ui: &mut egui::Ui, drafts: &[draft::DraftRecord]) {
        let palette = ui_palette();
        let mut decision: Option<(String, bool)> = None;
        let mut edit_action: Option<EditAction> = None;
        let mut toggle_selection: Option<String> = None;
        let mut local_edit = self.draft_edit.clone();
        let mut start_merge = false;
        let mut merge_confirm = false;
        let mut merge_cancel = false;
        let mut local_merge = self.merge_edit.clone();

        card_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                render_search_input(
                    ui,
                    &mut self.search_query,
                    "搜索草稿标题、来源或摘要...",
                    320.0,
                );
                let has_selection = !self.draft_selection.is_empty();
                if ui
                    .add_enabled(
                        !self.is_busy() && has_selection,
                        secondary_button("批准选中"),
                    )
                    .clicked()
                {
                    self.decide_drafts_for_selected(self.draft_selection.clone(), true);
                }
                if ui
                    .add_enabled(!self.is_busy() && has_selection, danger_button("拒绝选中"))
                    .clicked()
                {
                    self.decide_drafts_for_selected(self.draft_selection.clone(), false);
                }
            });
            ui.add_space(10.0);
            egui::ScrollArea::vertical()
                .id_salt(DRAFT_SCROLL_ID)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let using_synthetic = drafts.is_empty();
                    let display_drafts = if using_synthetic {
                        vec![
                            self.synthetic_draft(
                                "project:prefer-bun",
                                "Prefer Bun",
                                "优先使用 Bun 作为运行时和包管理器以提升开发效率。",
                                0.92,
                            ),
                            self.synthetic_draft(
                                "project:use-axios",
                                "Use Axios",
                                "使用 Axios 处理 HTTP 请求，保持 API 调用的一致性。",
                                0.89,
                            ),
                            self.synthetic_draft(
                                "project:reverse-parse",
                                "Reverse Parse Fix",
                                "修复反向解析在处理嵌套泛型时的边界情况。",
                                0.74,
                            ),
                            self.synthetic_draft(
                                "project:local-first",
                                "Local-first Provider Safety",
                                "默认使用本地模型提供商，避免将敏感数据发送到云端。",
                                0.90,
                            ),
                            self.synthetic_draft(
                                "project:budget",
                                "Rule Budget Guard",
                                "当规则数量接近上限时发出警告并阻止新增。",
                                0.68,
                            ),
                        ]
                    } else {
                        drafts.to_vec()
                    };

                    for draft in display_drafts.iter().filter(|draft| {
                        matches_search(
                            &self.search_query,
                            &format!("{} {} {}", draft.title, draft.body, draft.id),
                        )
                    }) {
                        let selected = self.draft_selection.contains(&draft.id);
                        let stroke = if selected {
                            egui::Stroke::new(1.2, palette.accent)
                        } else {
                            egui::Stroke::new(1.0, palette.border)
                        };
                        egui::Frame::new()
                            .fill(palette.card)
                            .stroke(stroke)
                            .corner_radius(egui::CornerRadius::same(12))
                            .inner_margin(egui::Margin::symmetric(14, 12))
                            .outer_margin(egui::Margin::symmetric(0, 5))
                            .show(ui, |ui| {
                                let is_editing = local_edit
                                    .as_ref()
                                    .is_some_and(|edit| edit.draft_id == draft.id);
                                if is_editing {
                                    let edit = local_edit.as_mut().expect("edit state");
                                    ui.label(
                                        egui::RichText::new("正在编辑草稿")
                                            .size(12.0)
                                            .color(palette.accent),
                                    );
                                    ui.add(
                                        egui::TextEdit::singleline(&mut edit.title)
                                            .desired_width(ui.available_width()),
                                    );
                                    ui.add(
                                        egui::TextEdit::multiline(&mut edit.body)
                                            .desired_rows(3)
                                            .desired_width(ui.available_width()),
                                    );
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::TextEdit::singleline(&mut edit.kind)
                                                .desired_width(120.0),
                                        );
                                        ui.add(
                                            egui::TextEdit::singleline(&mut edit.scope)
                                                .desired_width(120.0),
                                        );
                                        ui.add(
                                            egui::TextEdit::singleline(&mut edit.targets_text)
                                                .desired_width(180.0),
                                        );
                                    });
                                    ui.horizontal(|ui| {
                                        if ui
                                            .add_enabled(
                                                !self.is_busy(),
                                                primary_button(SAVE_LABEL),
                                            )
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
                                    ui.horizontal(|ui| {
                                        let mut checked = selected;
                                        if ui.checkbox(&mut checked, "").changed() {
                                            toggle_selection = Some(draft.id.clone());
                                        }
                                        self.paint_file_badge(ui, palette.accent);
                                        ui.vertical(|ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    egui::RichText::new(&draft.title)
                                                        .strong()
                                                        .size(16.0)
                                                        .color(palette.text),
                                                );
                                                if let Some(confidence) = draft.confidence {
                                                    Self::soft_badge(
                                                        ui,
                                                        &format!("高置信 {:.2}", confidence),
                                                        egui::Color32::from_rgb(232, 247, 239),
                                                        palette.success,
                                                    );
                                                }
                                                Self::soft_badge(
                                                    ui,
                                                    "反向解析",
                                                    palette.accent_soft,
                                                    palette.accent,
                                                );
                                            });
                                            ui.label(
                                                egui::RichText::new(&draft.body)
                                                    .color(palette.text),
                                            );
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    egui::RichText::new(format!(
                                                        "来源  {}",
                                                        draft.reason.as_deref().unwrap_or("观察")
                                                    ))
                                                    .size(12.0)
                                                    .color(palette.muted),
                                                );
                                                Self::soft_badge(
                                                    ui,
                                                    "CC Claude Code",
                                                    egui::Color32::from_rgb(30, 38, 52),
                                                    egui::Color32::WHITE,
                                                );
                                                Self::soft_badge(
                                                    ui,
                                                    "CX Codex",
                                                    egui::Color32::from_rgb(30, 38, 52),
                                                    egui::Color32::WHITE,
                                                );
                                            });
                                        });
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui
                                                    .add_enabled(
                                                        !self.is_busy() && !using_synthetic,
                                                        danger_button(REJECT_LABEL),
                                                    )
                                                    .clicked()
                                                {
                                                    decision = Some((draft.id.clone(), false));
                                                }
                                                if ui
                                                    .add_enabled(
                                                        !self.is_busy() && !using_synthetic,
                                                        secondary_button(APPROVE_LABEL),
                                                    )
                                                    .clicked()
                                                {
                                                    decision = Some((draft.id.clone(), true));
                                                }
                                                if ui
                                                    .add_enabled(
                                                        !self.is_busy() && !using_synthetic,
                                                        secondary_button(EDIT_LABEL),
                                                    )
                                                    .clicked()
                                                {
                                                    edit_action =
                                                        Some(EditAction::Start(DraftEditState {
                                                            draft_id: draft.id.clone(),
                                                            title: draft.title.clone(),
                                                            body: draft.body.clone(),
                                                            kind: draft.kind.clone(),
                                                            scope: draft.scope.clone(),
                                                            targets_text: draft.targets.join(", "),
                                                        }));
                                                }
                                            },
                                        );
                                    });
                                }
                            });
                    }
                });

            self.draft_edit = reduce_edit_state(local_edit, &edit_action);
            if let Some(draft_id) = toggle_selection {
                if let Some(pos) = self
                    .draft_selection
                    .iter()
                    .position(|item| item == &draft_id)
                {
                    self.draft_selection.remove(pos);
                } else {
                    self.draft_selection.push(draft_id);
                }
                self.draft_selection.sort();
                self.merge_edit = None;
            }

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

            if self.merge_edit.is_some() {
                let merge = local_merge.as_mut().expect("merge state");
                subtle_card_frame().show(ui, |ui| {
                    ui.label(egui::RichText::new("创建合并候选").strong());
                    ui.add(
                        egui::TextEdit::singleline(&mut merge.merged_id)
                            .hint_text("project:merged-draft")
                            .desired_width(ui.available_width()),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut merge.merged_title)
                            .hint_text("合并候选标题")
                            .desired_width(ui.available_width()),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut merge.targets_text)
                            .hint_text("codex, claude-code")
                            .desired_width(ui.available_width()),
                    );
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
        });

        if start_merge {
            self.merge_edit = Some(MergeEditState {
                merged_id: format!("project:merged-{}", self.draft_selection.len()),
                merged_title: String::new(),
                targets_text: "codex, claude-code".to_string(),
            });
        } else if merge_cancel {
            self.draft_selection.clear();
            self.merge_edit = None;
        } else {
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
    }

    fn draft_detail_card(&mut self, ui: &mut egui::Ui, selected: Option<&draft::DraftRecord>) {
        let palette = ui_palette();
        card_frame().show(ui, |ui| {
            ui.set_min_height(560.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("草稿详情")
                        .size(16.0)
                        .strong()
                        .color(palette.text),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    Self::soft_badge(ui, "PIN", palette.card_alt, palette.muted);
                });
            });
            ui.add_space(18.0);
            let draft = selected.cloned().unwrap_or_else(|| {
                self.synthetic_draft(
                    "project:prefer-bun",
                    "Prefer Bun",
                    "优先使用 Bun 作为运行时和包管理器以提升开发效率。",
                    0.92,
                )
            });
            ui.horizontal(|ui| {
                self.paint_file_badge(ui, palette.accent);
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(&draft.title)
                            .size(20.0)
                            .strong()
                            .color(palette.text),
                    );
                    if let Some(confidence) = draft.confidence {
                        Self::soft_badge(
                            ui,
                            &format!("高置信 {:.2}", confidence),
                            egui::Color32::from_rgb(232, 247, 239),
                            palette.success,
                        );
                    }
                });
            });
            ui.add_space(14.0);
            self.inspector_pair(ui, "ID", &draft.id);
            if let Some(project) = self.selected_project() {
                self.inspector_pair(ui, "项目", &compact_middle_path(&project.path));
                let markers = visible_header_markers(&project.markers);
                if !markers.is_empty() {
                    self.inspector_pair(ui, "标记", &markers.join(" / "));
                }
            }
            self.inspector_pair(ui, "摘要", &draft.body);
            if let Some(confidence) = draft.confidence {
                self.inspector_pair(ui, CONFIDENCE_LABEL, &format!("{:.2}", confidence));
            }
            if let Some(template) = draft.matched_template.as_deref() {
                self.inspector_pair(ui, MATCHED_TEMPLATE_LABEL, template);
            }
            self.inspector_pair(
                ui,
                REASON_LABEL,
                draft.reason.as_deref().unwrap_or(
                    "代码库中多处出现 Bun 相关命令，且用户偏好已高于 Node/npm 替代方案。",
                ),
            );
            ui.add_space(10.0);
            self.card_title(ui, "证据片段", "已脱敏");
            for (file, line) in [
                ("package.json", "\"packageManager\": \"bun@1.1.2\","),
                ("scripts/dev.sh", "bun run dev"),
                ("README.md", "我们推荐使用 Bun 以获得更快的冷启动速度。"),
            ] {
                subtle_card_frame().show(ui, |ui| {
                    ui.label(egui::RichText::new(file).strong());
                    ui.monospace(line);
                });
            }
            ui.add_space(10.0);
            self.card_title(ui, "目标 Agents", "");
            self.agent_toggle_row(ui, "CC", "Claude Code", true);
            self.agent_toggle_row(ui, "CX", "Codex", true);
            ui.add_space(10.0);
            subtle_card_frame().show(ui, |ui| {
                self.check_line(ui, "已脱敏：证据中已移除可能包含敏感信息的片段。");
            });
        });
    }

    fn render_catalog_page(&mut self, ui: &mut egui::Ui) {
        let palette = ui_palette();
        egui::Frame::new()
            .fill(palette.background)
            .inner_margin(egui::Margin::symmetric(30, 24))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt(CATALOG_SCROLL_ID)
                    .show(ui, |ui| {
                        self.render_page_header(
                            ui,
                            "Catalog / Skilllet App Store",
                            "安装可复用的轻量 Skilllet 包，并分配给 Claude Code 或 Codex。",
                            &[("刷新目录", UiPage::Catalog), ("编译预览", UiPage::RuleCi)],
                        );
                        ui.add_space(20.0);
                        let status = self.cached_catalog_status.clone();
                        let validation = self.cached_catalog_validation.clone();
                        ui.columns(3, |cols| {
                            let installed = status
                                .as_ref()
                                .map(|s| s.items.iter().filter(|item| item.installed).count())
                                .unwrap_or(0);
                            let total = status.as_ref().map(|s| s.items.len()).unwrap_or(0);
                            self.kpi_card(
                                &mut cols[0],
                                "PK",
                                CATALOG_TITLE,
                                &format!("{installed}/{total}"),
                                "已安装",
                                palette.accent,
                            );
                            self.kpi_card(
                                &mut cols[1],
                                "ER",
                                "Catalog Errors",
                                &validation
                                    .as_ref()
                                    .map(|v| v.errors.to_string())
                                    .unwrap_or_else(|| "0".to_string()),
                                "错误",
                                palette.danger,
                            );
                            self.kpi_card(
                                &mut cols[2],
                                "WA",
                                "Catalog Warnings",
                                &validation
                                    .as_ref()
                                    .map(|v| v.warnings.to_string())
                                    .unwrap_or_else(|| "0".to_string()),
                                "警告",
                                palette.warning,
                            );
                        });
                        ui.add_space(18.0);
                        card_frame().show(ui, |ui| {
                            self.card_title(ui, "可安装 Skilllet 包", "支持 Claude Code / Codex");
                            let mut install: Option<(String, Vec<String>)> = None;
                            if let Some(status) = status.as_ref() {
                                for item in &status.items {
                                    subtle_card_frame().show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.vertical(|ui| {
                                                ui.label(
                                                    egui::RichText::new(&item.package.title)
                                                        .strong()
                                                        .color(palette.text),
                                                );
                                                ui.monospace(&item.package.id);
                                                ui.label(
                                                    egui::RichText::new(&item.package.description)
                                                        .color(palette.muted),
                                                );
                                            });
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if ui
                                                        .add_enabled(
                                                            !self.is_busy(),
                                                            secondary_button(INSTALL_CLAUDE_LABEL),
                                                        )
                                                        .clicked()
                                                    {
                                                        install = Some((
                                                            item.package.id.clone(),
                                                            vec!["claude-code".to_string()],
                                                        ));
                                                    }
                                                    if ui
                                                        .add_enabled(
                                                            !self.is_busy(),
                                                            secondary_button(INSTALL_CODEX_LABEL),
                                                        )
                                                        .clicked()
                                                    {
                                                        install = Some((
                                                            item.package.id.clone(),
                                                            vec!["codex".to_string()],
                                                        ));
                                                    }
                                                },
                                            );
                                        });
                                    });
                                }
                            } else {
                                ui.label(
                                    egui::RichText::new(
                                        "当前项目暂无目录状态。运行扫描或选择一个项目后刷新。",
                                    )
                                    .color(palette.muted),
                                );
                            }
                            if let Some((package_id, targets)) = install {
                                self.install_catalog_for_selected(&package_id, targets);
                            }
                        });
                    });
            });
    }

    fn render_placeholder_page(&mut self, ui: &mut egui::Ui, title: &str, subtitle: &str) {
        let palette = ui_palette();
        egui::Frame::new()
            .fill(palette.background)
            .inner_margin(egui::Margin::symmetric(30, 24))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt(WORKSPACE_SCROLL_ID)
                    .show(ui, |ui| {
                        self.render_page_header(
                            ui,
                            title,
                            subtitle,
                            &[("同步", UiPage::Overview), ("构建预览", UiPage::RuleCi)],
                        );
                        ui.add_space(24.0);
                        ui.columns(3, |cols| {
                            self.kpi_card(
                                &mut cols[0],
                                "LC",
                                "Local-first",
                                "ON",
                                "隐私本地",
                                palette.success,
                            );
                            self.kpi_card(
                                &mut cols[1],
                                "CC",
                                "Claude Code",
                                "Ready",
                                "已支持",
                                palette.accent,
                            );
                            self.kpi_card(
                                &mut cols[2],
                                "CX",
                                "Codex",
                                "Ready",
                                "已支持",
                                palette.purple,
                            );
                        });
                        ui.add_space(18.0);
                        card_frame().show(ui, |ui| {
                            ui.set_min_height(360.0);
                            ui.vertical_centered(|ui| {
                                ui.add_space(90.0);
                                ui.label(
                                    egui::RichText::new(title)
                                        .size(26.0)
                                        .strong()
                                        .color(palette.text),
                                );
                                ui.label(
                                    egui::RichText::new(
                                        "该页面已按新设计系统保留扩展位，后续功能会在同一风格下继续补齐。",
                                    )
                                    .color(palette.muted),
                                );
                            });
                        });
                    });
            });
    }

    fn render_bottom_bar(&mut self, ui: &mut egui::Ui) {
        let palette = ui_palette();
        ui.add_space(18.0);
        subtle_card_frame().show(ui, |ui| {
            ui.set_width(205.0);
            ui.horizontal(|ui| {
                self.status_dot(ui, palette.success);
                ui.label(
                    egui::RichText::new(
                        self.busy_task
                            .as_deref()
                            .map(|task| format!("正在{}...", task))
                            .unwrap_or_else(|| "Ready for review".to_string()),
                    )
                    .size(14.0)
                    .color(palette.text),
                );
            });
        });
        ui.add_space(18.0);
        egui::ScrollArea::horizontal()
            .id_salt(OUTPUT_SCROLL_ID)
            .max_height(26.0)
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(if self.report.is_empty() {
                        &self.status
                    } else {
                        &self.report
                    })
                    .size(12.0)
                    .color(palette.muted),
                );
            });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(18.0);
            if ui
                .add_enabled(!self.is_busy(), primary_button("确认应用"))
                .clicked()
            {
                self.review_selected();
            }
            if ui
                .add_enabled(!self.is_busy(), secondary_button("应用选中项"))
                .clicked()
            {
                self.active_page = UiPage::DraftInbox;
            }
            if ui
                .add_enabled(!self.is_busy(), secondary_button("仅同步"))
                .clicked()
            {
                self.start_scan();
            }
            if !self.report.is_empty() {
                Self::soft_badge(ui, "有输出", palette.accent_soft, palette.accent);
            }
        });
    }

    fn kpi_card(
        &self,
        ui: &mut egui::Ui,
        badge: &str,
        title: &str,
        value: &str,
        subtitle: &str,
        accent: egui::Color32,
    ) {
        let palette = ui_palette();
        card_frame().show(ui, |ui| {
            ui.set_min_height(86.0);
            ui.horizontal(|ui| {
                self.paint_static_badge(ui, badge, accent);
                ui.add_space(10.0);
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(title).size(13.0).color(palette.text));
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(value)
                                .size(27.0)
                                .strong()
                                .color(egui::Color32::from_rgb(9, 15, 30)),
                        );
                        if !subtitle.is_empty() {
                            ui.label(egui::RichText::new(subtitle).size(11.0).color(accent));
                        }
                    });
                });
            });
        });
    }

    fn card_title(&self, ui: &mut egui::Ui, title: &str, badge: &str) {
        let palette = ui_palette();
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(title)
                    .size(16.0)
                    .strong()
                    .color(palette.text),
            );
            if !badge.is_empty() {
                Self::soft_badge(ui, badge, palette.accent_soft, palette.accent);
            }
        });
    }

    fn tabs(&self, ui: &mut egui::Ui, labels: &[&str]) {
        let palette = ui_palette();
        card_frame().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (idx, label) in labels.iter().enumerate() {
                    let fill = if idx == 0 {
                        palette.accent_soft
                    } else {
                        egui::Color32::TRANSPARENT
                    };
                    let color = if idx == 0 {
                        palette.accent
                    } else {
                        palette.muted
                    };
                    Self::soft_badge(ui, label, fill, color);
                    ui.add_space(12.0);
                }
            });
        });
    }

    fn table_head(&self, ui: &mut egui::Ui, text: &str) {
        ui.label(
            egui::RichText::new(text)
                .size(12.0)
                .strong()
                .color(ui_palette().muted),
        );
    }

    fn inspector_pair(&self, ui: &mut egui::Ui, label: &str, value: &str) {
        let palette = ui_palette();
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(label).size(12.0).color(palette.muted));
            ui.label(egui::RichText::new(value).size(13.0).color(palette.text));
        });
        ui.add_space(8.0);
    }

    fn diff_box(&self, ui: &mut egui::Ui, title: &str, lines: &[&str]) {
        let palette = ui_palette();
        subtle_card_frame().show(ui, |ui| {
            ui.label(egui::RichText::new(title).strong().color(palette.text));
            ui.add_space(6.0);
            for (idx, line) in lines.iter().enumerate() {
                let color = if line.starts_with('+') {
                    palette.success
                } else if line.starts_with('-') {
                    palette.danger
                } else {
                    palette.text
                };
                ui.label(
                    egui::RichText::new(format!("{:>2}  {}", idx + 1, line))
                        .monospace()
                        .color(color),
                );
            }
        });
    }

    fn agent_toggle_row(&self, ui: &mut egui::Ui, badge: &str, label: &str, enabled: bool) {
        let palette = ui_palette();
        subtle_card_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                Self::soft_badge(
                    ui,
                    badge,
                    egui::Color32::from_rgb(30, 38, 52),
                    egui::Color32::WHITE,
                );
                ui.label(egui::RichText::new(label).size(14.0).color(palette.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.toggle_visual(ui, enabled);
                });
            });
        });
    }

    fn status_dot(&self, ui: &mut egui::Ui, color: egui::Color32) {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
        ui.painter().circle_filled(rect.center(), 4.0, color);
    }

    fn toggle_visual(&self, ui: &mut egui::Ui, enabled: bool) {
        let palette = ui_palette();
        let fill = if enabled {
            palette.accent
        } else {
            palette.border
        };
        let (rect, _) = ui.allocate_exact_size(egui::vec2(40.0, 22.0), egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(11), fill);
        let knob_x = if enabled {
            rect.right() - 11.0
        } else {
            rect.left() + 11.0
        };
        ui.painter()
            .circle_filled(egui::pos2(knob_x, rect.center().y), 8.0, palette.card);
    }

    fn soft_badge(ui: &mut egui::Ui, text: &str, fill: egui::Color32, color: egui::Color32) {
        egui::Frame::new()
            .fill(fill)
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_white_alpha(120)))
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.label(egui::RichText::new(text).size(12.0).color(color));
            });
    }

    fn paint_static_badge(&self, ui: &mut egui::Ui, text: &str, accent: egui::Color32) {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::hover());
        ui.painter()
            .circle_filled(rect.center(), 18.0, ui_palette().accent_soft);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            text,
            egui::FontId::proportional(12.0),
            accent,
        );
    }

    fn paint_badge_at(
        &self,
        painter: &egui::Painter,
        center: egui::Pos2,
        text: &str,
        selected: bool,
    ) {
        let palette = ui_palette();
        let fill = if selected {
            palette.accent
        } else {
            egui::Color32::from_rgb(242, 246, 252)
        };
        let color = if selected {
            egui::Color32::WHITE
        } else {
            palette.accent
        };
        let rect = egui::Rect::from_center_size(center, egui::vec2(28.0, 24.0));
        painter.rect_filled(rect, egui::CornerRadius::same(7), fill);
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            text,
            egui::FontId::proportional(11.0),
            color,
        );
    }

    fn paint_file_badge(&self, ui: &mut egui::Ui, color: egui::Color32) {
        let palette = ui_palette();
        let (rect, _) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::hover());
        ui.painter().rect(
            rect.shrink(4.0),
            egui::CornerRadius::same(8),
            egui::Color32::from_rgb(246, 250, 255),
            egui::Stroke::new(1.0, palette.border),
            egui::StrokeKind::Outside,
        );
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "MD",
            egui::FontId::proportional(11.0),
            color,
        );
    }

    fn check_line(&self, ui: &mut egui::Ui, text: &str) {
        let palette = ui_palette();
        ui.horizontal(|ui| {
            self.status_dot(ui, palette.success);
            ui.label(egui::RichText::new(text).color(palette.text));
        });
    }

    fn score_bar(&self, ui: &mut egui::Ui, score: f32, color: egui::Color32) {
        let palette = ui_palette();
        let (rect, _) = ui.allocate_exact_size(egui::vec2(90.0, 8.0), egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(4), palette.border);
        ui.painter().rect_filled(
            egui::Rect::from_min_size(
                rect.min,
                egui::vec2(rect.width() * score.clamp(0.0, 1.0), rect.height()),
            ),
            egui::CornerRadius::same(4),
            color,
        );
    }

    fn paint_flow(&self, ui: &mut egui::Ui, labels: &[&str]) {
        let palette = ui_palette();
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 120.0), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        let step = rect.width() / labels.len().max(1) as f32;
        let y = rect.center().y - 10.0;
        for (idx, label) in labels.iter().enumerate() {
            let x = rect.left() + step * (idx as f32 + 0.5);
            if idx > 0 {
                let previous_x = rect.left() + step * (idx as f32 - 0.5);
                painter.line_segment(
                    [egui::pos2(previous_x + 28.0, y), egui::pos2(x - 28.0, y)],
                    egui::Stroke::new(1.4, palette.accent),
                );
            }
            painter.circle_filled(
                egui::pos2(x, y),
                28.0,
                if idx >= 3 {
                    egui::Color32::from_rgb(229, 249, 244)
                } else {
                    palette.accent_soft
                },
            );
            painter.text(
                egui::pos2(x, y),
                egui::Align2::CENTER_CENTER,
                format!("{}", idx + 1),
                egui::FontId::proportional(14.0),
                palette.accent,
            );
            painter.text(
                egui::pos2(x, y + 44.0),
                egui::Align2::CENTER_CENTER,
                *label,
                egui::FontId::proportional(13.0),
                palette.text,
            );
        }
    }

    fn synthetic_draft(
        &self,
        id: &str,
        title: &str,
        body: &str,
        confidence: f32,
    ) -> draft::DraftRecord {
        draft::DraftRecord {
            id: id.to_string(),
            title: title.to_string(),
            body: body.to_string(),
            kind: "preference".to_string(),
            scope: "project".to_string(),
            targets: vec!["claude-code".to_string(), "codex".to_string()],
            evidence: "synthetic native preview".to_string(),
            confidence: Some(confidence),
            reason: Some("来自观察".to_string()),
            matched_template: Some("native:preview".to_string()),
            status: "draft".to_string(),
            created_at: "2m ago".to_string(),
            updated_at: "2m ago".to_string(),
        }
    }
}
