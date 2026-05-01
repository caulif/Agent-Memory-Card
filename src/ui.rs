use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;

use crate::build;
use crate::catalog;
use crate::config;
use crate::draft::{self, DraftRecord};
use crate::extract;
use crate::fsutil;
use crate::observation;
use crate::review;
use crate::rule_test;
use crate::skilllet::{self, SkillletRecord};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_json_value_includes_summary_and_drafts() {
        let temp = tempfile::tempdir().expect("tempdir");
        draft::add_draft(
            temp.path(),
            draft::NewDraft {
                id: "project:prefer-pnpm".to_string(),
                title: "Prefer pnpm".to_string(),
                body: "Use pnpm for package management.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "manual test".to_string(),
            },
        )
        .expect("add draft");

        let value = review_json_value(temp.path()).expect("review json");

        assert_eq!(value["ok"], true);
        assert_eq!(value["report"]["summary"]["drafts_pending"], 1);
        assert_eq!(value["report"]["drafts"][0]["id"], "project:prefer-pnpm");
    }

    #[test]
    fn catalog_validation_json_reports_clean_default_catalog() {
        let temp = tempfile::tempdir().expect("tempdir");

        let value = catalog_validation_json_value(temp.path()).expect("catalog validation");

        assert_eq!(value["ok"], true);
        assert_eq!(value["report"]["errors"], 0);
    }

    #[test]
    fn canvas_html_renders_target_matrix_as_table() {
        assert!(INDEX_HTML.contains("target-matrix-table"));
        assert!(INDEX_HTML.contains("matrix-cell assigned"));
    }

    #[test]
    fn canvas_html_has_artifact_status_panel() {
        assert!(INDEX_HTML.contains("Artifact Status"));
        assert!(INDEX_HTML.contains("artifact-status"));
    }

    #[test]
    fn canvas_html_has_import_artifacts_action() {
        assert!(INDEX_HTML.contains("import-artifacts"));
        assert!(INDEX_HTML.contains("Import Artifacts"));
    }

    #[test]
    fn canvas_html_has_observations_panel() {
        assert!(INDEX_HTML.contains("Observations"));
        assert!(INDEX_HTML.contains("observation-count"));
    }
}

#[derive(Clone)]
struct AppState {
    project_root: Arc<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct MirrorRequest {
    skill: String,
    agent: String,
}

#[derive(Debug, Deserialize)]
struct DraftActionRequest {
    id: String,
}

#[derive(Debug, Deserialize)]
struct ExtractRequest {
    text: String,
    targets: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CatalogInstallRequest {
    id: String,
    targets: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct AgentEnabledRequest {
    agent: String,
    enabled: bool,
}

#[derive(Debug, Deserialize)]
struct SkillletTargetsRequest {
    id: String,
    targets: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ApiState {
    project_root: String,
    project: config::ProjectConfig,
    skill_index: config::SkillIndex,
    skilllets: Vec<SkillletRecord>,
    drafts: Vec<DraftRecord>,
    imported_rules: serde_yaml::Value,
    observations: Vec<observation::ObservationRecord>,
}

pub async fn serve(project: PathBuf, port: u16, open_browser: bool) -> Result<()> {
    let root = fsutil::normalize_project_root(&project)?;
    let state = AppState {
        project_root: Arc::new(root),
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/favicon.ico", get(favicon))
        .route("/api/state", get(api_state))
        .route("/api/mirror", post(api_mirror))
        .route("/api/agent/enabled", post(api_agent_enabled))
        .route("/api/skilllet/matrix", get(api_skilllet_matrix))
        .route("/api/skilllet/targets", post(api_skilllet_targets))
        .route("/api/draft/approve", post(api_draft_approve))
        .route("/api/draft/reject", post(api_draft_reject))
        .route("/api/extract", post(api_extract))
        .route("/api/build/preview", post(api_build_preview))
        .route("/api/artifacts/import", post(api_artifacts_import))
        .route("/api/sync", post(api_sync))
        .route("/api/status", get(api_status))
        .route("/api/rule-tests", get(api_rule_tests))
        .route("/api/review", get(api_review))
        .route("/api/catalog", get(api_catalog))
        .route("/api/catalog/validate", get(api_catalog_validate))
        .route("/api/catalog/install", post(api_catalog_install))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let url = format!("http://{addr}");
    println!("Agent-Kernel UI running at {url}");
    if open_browser {
        let _ = open::that(&url);
    }

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn favicon() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn api_state(State(state): State<AppState>) -> Json<ApiState> {
    let root = state.project_root.as_ref();
    let project = config::load_or_default_project_config(root)
        .unwrap_or_else(|_| config::default_project_config(root));
    let skill_index = config::load_skill_index(root).unwrap_or_default();
    let skilllets = skilllet::load_skilllets(root).unwrap_or_default();
    let drafts = draft::load_drafts(root).unwrap_or_default();
    let observations = observation::load_observations(root).unwrap_or_default();
    let rules_path = config::kernel_dir(root).join("imported-rules.yml");
    let imported_rules = fs::read_to_string(rules_path)
        .ok()
        .and_then(|text| serde_yaml::from_str(&text).ok())
        .unwrap_or(serde_yaml::Value::Sequence(Vec::new()));

    Json(ApiState {
        project_root: fsutil::path_to_slash(root),
        project,
        skill_index,
        skilllets,
        drafts,
        imported_rules,
        observations,
    })
}

async fn api_mirror(
    State(state): State<AppState>,
    Json(req): Json<MirrorRequest>,
) -> Json<serde_json::Value> {
    match build::mirror(state.project_root.as_ref(), &req.skill, &req.agent) {
        Ok(_) => Json(serde_json::json!({ "ok": true })),
        Err(error) => Json(serde_json::json!({ "ok": false, "error": error.to_string() })),
    }
}

async fn api_agent_enabled(
    State(state): State<AppState>,
    Json(req): Json<AgentEnabledRequest>,
) -> Json<serde_json::Value> {
    match config::set_agent_enabled(state.project_root.as_ref(), &req.agent, req.enabled) {
        Ok(_) => Json(serde_json::json!({ "ok": true })),
        Err(error) => Json(serde_json::json!({ "ok": false, "error": error.to_string() })),
    }
}

async fn api_skilllet_targets(
    State(state): State<AppState>,
    Json(req): Json<SkillletTargetsRequest>,
) -> Json<serde_json::Value> {
    match skilllet::set_skilllet_targets(state.project_root.as_ref(), &req.id, req.targets) {
        Ok(_) => Json(serde_json::json!({ "ok": true })),
        Err(error) => Json(serde_json::json!({ "ok": false, "error": error.to_string() })),
    }
}

async fn api_skilllet_matrix(State(state): State<AppState>) -> Json<serde_json::Value> {
    match skilllet::skilllet_target_matrix(state.project_root.as_ref()) {
        Ok(matrix) => Json(serde_json::json!({ "ok": true, "matrix": matrix })),
        Err(error) => Json(serde_json::json!({ "ok": false, "error": error.to_string() })),
    }
}

async fn api_draft_approve(
    State(state): State<AppState>,
    Json(req): Json<DraftActionRequest>,
) -> Json<serde_json::Value> {
    match draft::approve_draft(state.project_root.as_ref(), &req.id) {
        Ok(_) => Json(serde_json::json!({ "ok": true })),
        Err(error) => Json(serde_json::json!({ "ok": false, "error": error.to_string() })),
    }
}

async fn api_draft_reject(
    State(state): State<AppState>,
    Json(req): Json<DraftActionRequest>,
) -> Json<serde_json::Value> {
    match draft::reject_draft(state.project_root.as_ref(), &req.id) {
        Ok(_) => Json(serde_json::json!({ "ok": true })),
        Err(error) => Json(serde_json::json!({ "ok": false, "error": error.to_string() })),
    }
}

async fn api_extract(
    State(state): State<AppState>,
    Json(req): Json<ExtractRequest>,
) -> Json<serde_json::Value> {
    match extract::extract_text_to_drafts(
        state.project_root.as_ref(),
        &req.text,
        req.targets,
        "ui",
        Some("local".to_string()),
        false,
    ) {
        Ok(report) => Json(serde_json::json!({
            "ok": true,
            "created": report.created,
            "skipped": report.skipped,
            "candidates": report.candidates,
            "redacted": report.redacted,
            "text": report.render(),
        })),
        Err(error) => Json(serde_json::json!({ "ok": false, "error": error.to_string() })),
    }
}

async fn api_build_preview(State(state): State<AppState>) -> Json<serde_json::Value> {
    match build::preview_as_json(state.project_root.as_ref()) {
        Ok(value) => Json(value),
        Err(error) => Json(serde_json::json!({ "preview": true, "text": error.to_string() })),
    }
}

async fn api_artifacts_import(State(state): State<AppState>) -> Json<serde_json::Value> {
    match build::import_artifact_drifts(state.project_root.as_ref()) {
        Ok(report) => Json(serde_json::json!({
            "ok": true,
            "text": report.render(),
            "report": report,
        })),
        Err(error) => Json(serde_json::json!({ "ok": false, "text": error.to_string() })),
    }
}

async fn api_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    match build::status_project(state.project_root.as_ref()) {
        Ok(report) => Json(serde_json::to_value(report).unwrap_or_else(
            |error| serde_json::json!({ "rows": [], "artifact_rows": [], "warnings": [error.to_string()] }),
        )),
        Err(error) => Json(serde_json::json!({ "rows": [], "artifact_rows": [], "warnings": [error.to_string()] })),
    }
}

async fn api_sync(State(state): State<AppState>) -> Json<serde_json::Value> {
    match build::sync_project(state.project_root.as_ref()) {
        Ok(report) => Json(serde_json::json!({ "ok": true, "text": report.render() })),
        Err(error) => Json(serde_json::json!({ "ok": false, "text": error.to_string() })),
    }
}

async fn api_rule_tests(State(state): State<AppState>) -> Json<serde_json::Value> {
    match rule_test::run_rule_tests(state.project_root.as_ref()) {
        Ok(report) => Json(serde_json::to_value(report).unwrap_or_else(
            |error| serde_json::json!({ "passed": 0, "failed": 0, "rows": [], "error": error.to_string() }),
        )),
        Err(error) => Json(serde_json::json!({
            "passed": 0,
            "failed": 0,
            "rows": [],
            "error": error.to_string(),
        })),
    }
}

async fn api_review(State(state): State<AppState>) -> Json<serde_json::Value> {
    match review_json_value(state.project_root.as_ref()) {
        Ok(value) => Json(value),
        Err(error) => Json(serde_json::json!({ "ok": false, "error": error.to_string() })),
    }
}

fn review_json_value(project_root: &std::path::Path) -> Result<serde_json::Value> {
    let report = review::review_project(project_root)?;
    Ok(serde_json::json!({
        "ok": true,
        "report": report,
    }))
}

async fn api_catalog(State(state): State<AppState>) -> Json<serde_json::Value> {
    match catalog::catalog_status(state.project_root.as_ref()) {
        Ok(catalog) => Json(serde_json::json!({ "ok": true, "catalog": catalog })),
        Err(error) => Json(serde_json::json!({ "ok": false, "error": error.to_string() })),
    }
}

async fn api_catalog_validate(State(state): State<AppState>) -> Json<serde_json::Value> {
    match catalog_validation_json_value(state.project_root.as_ref()) {
        Ok(value) => Json(value),
        Err(error) => Json(serde_json::json!({ "ok": false, "error": error.to_string() })),
    }
}

fn catalog_validation_json_value(project_root: &std::path::Path) -> Result<serde_json::Value> {
    let catalog = catalog::load_or_default_catalog(project_root)?;
    let report = catalog::validate_catalog(&catalog);
    Ok(serde_json::json!({
        "ok": true,
        "report": report,
    }))
}

async fn api_catalog_install(
    State(state): State<AppState>,
    Json(req): Json<CatalogInstallRequest>,
) -> Json<serde_json::Value> {
    match catalog::install_catalog_package(state.project_root.as_ref(), &req.id, req.targets) {
        Ok(package) => Json(serde_json::json!({ "ok": true, "package": package })),
        Err(error) => Json(serde_json::json!({ "ok": false, "error": error.to_string() })),
    }
}

const INDEX_HTML: &str = r##"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Agent-Kernel</title>
  <style>
    :root {
      color-scheme: light;
      --bg: #f6f7f9;
      --panel: #ffffff;
      --text: #171b22;
      --muted: #657083;
      --line: #d8dee8;
      --accent: #0f766e;
      --accent-2: #2563eb;
      --good: #15803d;
      --warn: #b45309;
      --danger: #b91c1c;
      font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }
    * { box-sizing: border-box; }
    body { margin: 0; background: var(--bg); color: var(--text); }
    .app { display: grid; grid-template-columns: 300px 1fr 360px; grid-template-rows: 56px 1fr 84px; min-height: 100vh; }
    header { grid-column: 1 / 4; display: flex; align-items: center; justify-content: space-between; padding: 0 18px; border-bottom: 1px solid var(--line); background: var(--panel); }
    h1 { font-size: 16px; margin: 0; font-weight: 700; }
    .path { color: var(--muted); font-size: 12px; }
    aside, .inspector { background: var(--panel); border-right: 1px solid var(--line); overflow: auto; }
    .inspector { border-right: 0; border-left: 1px solid var(--line); }
    .pane-title { display: flex; align-items: center; justify-content: space-between; padding: 14px 14px 8px; color: var(--muted); font-size: 12px; text-transform: uppercase; letter-spacing: .04em; }
    .search { margin: 0 14px 10px; width: calc(100% - 28px); padding: 9px 10px; border: 1px solid var(--line); border-radius: 6px; }
    .skill { margin: 8px 12px; padding: 10px; border: 1px solid var(--line); border-radius: 8px; background: #fbfcfe; cursor: grab; }
    .skill:hover { border-color: var(--accent); }
    .skill strong { display: block; font-size: 13px; }
    .skill p { margin: 6px 0 0; color: var(--muted); font-size: 12px; line-height: 1.35; }
    .tag { display: inline-block; margin-top: 8px; color: var(--accent); font-size: 11px; border: 1px solid #99d8d0; border-radius: 999px; padding: 2px 7px; background: #ecfdf9; }
    main { position: relative; overflow: hidden; background-image: radial-gradient(#d7dce5 1px, transparent 1px); background-size: 22px 22px; }
    .canvas { position: relative; width: 100%; height: 100%; min-height: calc(100vh - 140px); }
    .node { position: absolute; min-width: 150px; max-width: 220px; padding: 14px; border: 1px solid var(--line); border-radius: 10px; background: rgba(255,255,255,.96); box-shadow: 0 10px 30px rgba(15, 23, 42, .08); }
    .node.project { left: calc(50% - 105px); top: 38%; min-width: 210px; border-color: #0f766e; }
    .node h2 { margin: 0; font-size: 14px; }
    .node p { margin: 6px 0 0; color: var(--muted); font-size: 12px; }
    .agent { cursor: pointer; }
    .agent.active { outline: 3px solid rgba(37,99,235,.18); border-color: var(--accent-2); }
    .codex { left: 12%; top: 16%; }
    .claude-code { right: 12%; top: 16%; }
    .cursor { left: 15%; bottom: 16%; }
    .cline { right: 15%; bottom: 16%; }
    svg.edges { position: absolute; inset: 0; width: 100%; height: 100%; pointer-events: none; }
    .inspector section { padding: 14px; border-bottom: 1px solid var(--line); }
    .inspector h3 { margin: 0 0 8px; font-size: 14px; }
    .inspector p, .inspector li { color: var(--muted); font-size: 12px; line-height: 1.45; }
    .empty { color: var(--muted); padding: 14px; font-size: 13px; }
    .btn { border: 1px solid var(--line); background: var(--panel); color: var(--text); border-radius: 7px; padding: 8px 11px; cursor: pointer; font-weight: 600; }
    .btn.primary { background: var(--accent); color: white; border-color: var(--accent); }
    .btn:disabled { opacity: .45; cursor: not-allowed; }
    footer { grid-column: 1 / 4; display: flex; align-items: center; gap: 10px; padding: 12px 16px; border-top: 1px solid var(--line); background: var(--panel); }
    pre { width: 100%; max-height: 58px; overflow: auto; margin: 0; color: var(--muted); font-size: 12px; white-space: pre-wrap; }
    .store { display: none; position: fixed; inset: 70px 70px; background: var(--panel); border: 1px solid var(--line); border-radius: 12px; box-shadow: 0 24px 80px rgba(15, 23, 42, .2); z-index: 4; overflow: hidden; }
    .store.open { display: grid; grid-template-rows: 52px 1fr; }
    .store-head { display: flex; align-items: center; justify-content: space-between; padding: 0 16px; border-bottom: 1px solid var(--line); }
    .store-grid { overflow: auto; padding: 16px; display: grid; grid-template-columns: repeat(auto-fill, minmax(230px, 1fr)); gap: 12px; }
    .card { border: 1px solid var(--line); border-radius: 10px; padding: 12px; background: #fbfcfe; }
    .card h4 { margin: 0 0 8px; font-size: 14px; }
    .card p { color: var(--muted); font-size: 12px; line-height: 1.4; }
    .review-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
    .metric { border: 1px solid var(--line); border-radius: 8px; padding: 10px; background: #fbfcfe; }
    .metric strong { display: block; font-size: 18px; }
    .metric span { color: var(--muted); font-size: 11px; }
    .target-matrix-table { width: 100%; border-collapse: collapse; font-size: 12px; }
    .target-matrix-table th { color: var(--muted); font-weight: 700; text-align: left; padding: 6px 4px; border-bottom: 1px solid var(--line); }
    .target-matrix-table td { padding: 6px 4px; border-bottom: 1px solid var(--line); vertical-align: middle; }
    .target-matrix-table .skilllet-name { max-width: 128px; overflow-wrap: anywhere; color: var(--text); font-weight: 600; }
    .matrix-cell { width: 28px; height: 28px; border: 1px solid var(--line); border-radius: 6px; background: #f8fafc; color: var(--muted); cursor: pointer; font-weight: 700; }
    .matrix-cell.assigned { border-color: #8bd3ca; background: #ecfdf9; color: var(--accent); }
    .ok { color: var(--good); }
    .bad { color: var(--danger); }
  </style>
</head>
<body>
  <div class="app">
    <header>
      <div>
        <h1>Agent-Kernel Canvas</h1>
        <div class="path" id="project-path"></div>
      </div>
      <button class="btn" id="open-store">App Store</button>
    </header>

    <aside>
      <div class="pane-title">Skill Library <span id="skill-count">0</span></div>
      <input class="search" id="search" placeholder="Search skills" />
      <div id="skills"></div>
      <div class="pane-title">Skilllets <span id="skilllet-count">0</span></div>
      <div id="skilllets"></div>
      <div class="pane-title">Draft Inbox <span id="draft-count">0</span></div>
      <div id="drafts"></div>
      <div class="pane-title">Observations <span id="observation-count">0</span></div>
      <div id="observations"></div>
    </aside>

    <main>
      <svg class="edges" id="edges"></svg>
      <div class="canvas" id="canvas">
        <div class="node project" data-node="project">
          <h2>Current Project</h2>
          <p>Drop skills onto agents to create mirrors.</p>
        </div>
      </div>
    </main>

    <div class="inspector">
      <section>
        <h3>Inspector</h3>
        <div id="inspector" class="empty">Select a skill or agent.</div>
      </section>
      <section>
        <h3>Extract Drafts</h3>
        <textarea id="extract-text" placeholder="Paste a correction or session note" style="width:100%;min-height:90px;resize:vertical;border:1px solid var(--line);border-radius:7px;padding:8px"></textarea>
        <div id="extract-targets" style="display:flex;flex-wrap:wrap;gap:6px;margin-top:8px"></div>
        <button class="btn" id="extract-drafts" style="margin-top:8px">Extract</button>
      </section>
      <section>
        <h3>Declared Mirrors</h3>
        <div id="mirrors"></div>
      </section>
      <section>
        <h3>Target Matrix</h3>
        <div id="target-matrix"></div>
      </section>
      <section>
        <h3>Review</h3>
        <div id="review"></div>
      </section>
      <section>
        <h3>Mirror Status</h3>
        <div id="mirror-status"></div>
      </section>
      <section>
        <h3>Artifact Status</h3>
        <div id="artifact-status"></div>
      </section>
      <section>
        <h3>Rule CI</h3>
        <div id="rule-tests"></div>
      </section>
      <section>
        <h3>Imported Rules</h3>
        <div id="rules"></div>
      </section>
    </div>

    <footer>
      <button class="btn primary" id="preview">Preview Build</button>
      <button class="btn" id="review-button">Review</button>
      <button class="btn" id="run-rule-tests">Rule CI</button>
      <button class="btn" id="import-artifacts">Import Artifacts</button>
      <button class="btn" id="sync">Sync Mirrors</button>
      <button class="btn" id="refresh">Refresh</button>
      <pre id="build-output">Ready.</pre>
    </footer>
  </div>

  <div class="store" id="store">
    <div class="store-head">
      <strong>Skill App Store</strong>
      <button class="btn" id="close-store">Close</button>
    </div>
    <div class="store-grid" id="store-grid"></div>
  </div>

  <script>
    let state = null;
    let selectedSkill = null;
    let selectedAgent = null;
    let catalogPackages = [];

    const agentPositions = {
      "codex": "codex",
      "claude-code": "claude-code"
    };

    async function loadState() {
      const res = await fetch("/api/state");
      state = await res.json();
      document.getElementById("project-path").textContent = state.project_root;
      renderSkills();
      renderSkilllets();
      renderDrafts();
      renderObservations();
      renderCanvas();
      renderMirrors();
      await renderTargetMatrix();
      await renderStatus();
      await renderRuleTests(false);
      await renderReview(false);
      renderRules();
      await renderStore();
      renderExtractTargets();
    }

    function renderSkills() {
      const q = document.getElementById("search").value.toLowerCase();
      const skills = state.skill_index.skills.filter(s =>
        s.id.toLowerCase().includes(q) || s.name.toLowerCase().includes(q) || (s.description || "").toLowerCase().includes(q)
      );
      document.getElementById("skill-count").textContent = skills.length;
      document.getElementById("skills").innerHTML = skills.map(skill => `
        <div class="skill" draggable="true" data-skill="${escapeHtml(skill.id)}">
          <strong>${escapeHtml(skill.id)}</strong>
          <p>${escapeHtml(skill.description || "No description")}</p>
          <span class="tag">${escapeHtml(skill.source_kind)}</span>
          ${skill.warnings && skill.warnings.length ? `<span class="tag" style="color:#b45309;border-color:#f0c36a;background:#fffbeb">${skill.warnings.length} warning</span>` : ""}
        </div>
      `).join("") || `<div class="empty">No skills indexed. Run import with --scan-home.</div>`;
      document.querySelectorAll(".skill").forEach(el => {
        el.addEventListener("click", () => selectSkill(el.dataset.skill));
        el.addEventListener("dragstart", ev => {
          ev.dataTransfer.setData("text/plain", el.dataset.skill);
        });
      });
    }

    function renderSkilllets() {
      const skilllets = state.skilllets || [];
      const agents = Object.entries(state.project.agents || {});
      document.getElementById("skilllet-count").textContent = skilllets.length;
      document.getElementById("skilllets").innerHTML = skilllets.map(item => {
        const ref = (state.project.skilllets?.include || []).find(entry => entry.id === item.id);
        const targets = ref ? ref.targets || [] : [];
        const buttons = agents.map(([name, agent]) => {
          const active = targets.length === 0 ? agent.enabled : targets.includes(name);
          return `<button class="btn" style="margin-top:6px;margin-right:4px;border-color:${active ? "var(--accent)" : "var(--line)"}" onclick="toggleSkillletTarget('${escapeAttr(item.id)}','${escapeAttr(name)}')">${active ? "✓ " : ""}${escapeHtml(name)}</button>`;
        }).join("");
        return `
          <div class="skill">
            <strong>${escapeHtml(item.id)}</strong>
            <p>${escapeHtml(item.body)}</p>
            <span class="tag">${escapeHtml(item.kind)}</span>
            <div>${buttons}</div>
          </div>
        `;
      }).join("") || `<div class="empty">No owned skilllets yet.</div>`;
    }

    function renderDrafts() {
      const drafts = state.drafts || [];
      document.getElementById("draft-count").textContent = drafts.length;
      document.getElementById("drafts").innerHTML = drafts.map(item => `
        <div class="skill">
          <strong>${escapeHtml(item.id)}</strong>
          <p>${escapeHtml(item.body)}</p>
          <span class="tag">${escapeHtml(item.status)}</span>
          <div style="display:flex;gap:6px;margin-top:8px">
            <button class="btn" onclick="approveDraft('${escapeAttr(item.id)}')">Approve</button>
            <button class="btn" onclick="rejectDraft('${escapeAttr(item.id)}')">Reject</button>
          </div>
        </div>
      `).join("") || `<div class="empty">No drafts yet.</div>`;
    }

    function renderObservations() {
      const observations = state.observations || [];
      document.getElementById("observation-count").textContent = observations.length;
      document.getElementById("observations").innerHTML = observations.slice(0, 12).map(item => `
        <div class="skill">
          <strong>${escapeHtml(item.id)}</strong>
          <p>${escapeHtml((item.body || "").slice(0, 180))}</p>
          <span class="tag">${escapeHtml(item.agent || "local")}</span>
          <span class="tag">${escapeHtml(item.source_kind)}</span>
        </div>
      `).join("") || `<div class="empty">No observations yet.</div>`;
    }

    function renderCanvas() {
      const canvas = document.getElementById("canvas");
      canvas.querySelectorAll(".agent").forEach(n => n.remove());
      Object.entries(state.project.agents).forEach(([name, agent]) => {
        const node = document.createElement("div");
        node.className = `node agent ${agentPositions[name] || ""}`;
        node.dataset.agent = name;
        node.innerHTML = `<h2>${escapeHtml(name)}</h2><p>${agent.enabled ? "Enabled" : "Disabled"}${agent.exports.skills_dir ? " · skills" : ""}</p><button class="btn" style="margin-top:8px" onclick="event.stopPropagation(); setAgentEnabled('${escapeAttr(name)}', ${agent.enabled ? "false" : "true"})">${agent.enabled ? "Disable" : "Enable"}</button>`;
        node.addEventListener("click", () => selectAgent(name));
        node.addEventListener("dragover", ev => ev.preventDefault());
        node.addEventListener("drop", async ev => {
          ev.preventDefault();
          const skill = ev.dataTransfer.getData("text/plain");
          await mirror(skill, name);
        });
        canvas.appendChild(node);
      });
      drawEdges();
    }

    function drawEdges() {
      const svg = document.getElementById("edges");
      svg.innerHTML = "";
      const canvasRect = document.querySelector("main").getBoundingClientRect();
      const project = document.querySelector(".project").getBoundingClientRect();
      document.querySelectorAll(".agent").forEach(agent => {
        const rect = agent.getBoundingClientRect();
        const x1 = project.left + project.width / 2 - canvasRect.left;
        const y1 = project.top + project.height / 2 - canvasRect.top;
        const x2 = rect.left + rect.width / 2 - canvasRect.left;
        const y2 = rect.top + rect.height / 2 - canvasRect.top;
        const line = document.createElementNS("http://www.w3.org/2000/svg", "line");
        line.setAttribute("x1", x1);
        line.setAttribute("y1", y1);
        line.setAttribute("x2", x2);
        line.setAttribute("y2", y2);
        line.setAttribute("stroke", "#bac3d1");
        line.setAttribute("stroke-width", "2");
        svg.appendChild(line);
      });
    }

    function renderMirrors() {
      const mirrors = state.project.skills?.mirrors || [];
      document.getElementById("mirrors").innerHTML = mirrors.length ? `<ul>${mirrors.map(m =>
        `<li><strong>${escapeHtml(m.ref)}</strong><br>${escapeHtml(m.targets.join(", "))}</li>`
      ).join("")}</ul>` : `<p>No mirrors declared yet.</p>`;
    }

    async function renderTargetMatrix() {
      const res = await fetch("/api/skilllet/matrix");
      const payload = await res.json();
      if (!payload.ok) {
        document.getElementById("target-matrix").innerHTML = `<p>${escapeHtml(payload.error)}</p>`;
        return;
      }
      const matrix = payload.matrix;
      document.getElementById("target-matrix").innerHTML = matrix.rows && matrix.rows.length ? `
        <table class="target-matrix-table">
          <thead>
            <tr>
              <th>Skilllet</th>
              ${matrix.agents.map(agent => `<th title="${escapeHtml(agent)}">${escapeHtml(shortAgentLabel(agent))}</th>`).join("")}
            </tr>
          </thead>
          <tbody>
            ${matrix.rows.map(row => `
              <tr>
                <td class="skilllet-name">${escapeHtml(row.skilllet_id)}</td>
                ${matrix.agents.map(agent => {
                  const assigned = Boolean(row.targets[agent]);
                  const label = assigned ? "✓" : "−";
                  const className = assigned ? "matrix-cell assigned" : "matrix-cell";
                  return `<td><button class="${className}" title="${escapeHtml(row.skilllet_id)} ${assigned ? "uses" : "does not use"} ${escapeHtml(agent)}" onclick="toggleSkillletTarget('${escapeAttr(row.skilllet_id)}','${escapeAttr(agent)}')">${label}</button></td>`;
                }).join("")}
              </tr>
            `).join("")}
          </tbody>
        </table>
      ` : `<p>No skilllets found.</p>`;
    }

    async function renderStatus() {
      const res = await fetch("/api/status");
      const status = await res.json();
      const rows = status.rows || [];
      document.getElementById("mirror-status").innerHTML = rows.length ? `<ul>${rows.map(row =>
        `<li><strong>${escapeHtml(row.skill)}</strong><br>${escapeHtml(row.agent)} · ${escapeHtml(row.status)}</li>`
      ).join("")}</ul>` : `<p>No mirror status yet.</p>`;
      const artifactRows = status.artifact_rows || [];
      document.getElementById("artifact-status").innerHTML = artifactRows.length ? `<ul>${artifactRows.map(row =>
        `<li><strong>${escapeHtml(row.path)}</strong><br>${escapeHtml(row.kind)} · <span class="${row.status === "synced" ? "ok" : "bad"}">${escapeHtml(row.status)}</span></li>`
      ).join("")}</ul>` : `<p>No generated artifacts recorded yet.</p>`;
    }

    async function renderRuleTests(writeOutput) {
      const res = await fetch("/api/rule-tests");
      const report = await res.json();
      const rows = report.rows || [];
      const html = rows.length ? `<ul>${rows.map(row =>
        `<li><strong>${escapeHtml(row.name)}</strong><br>${escapeHtml(row.status)}${row.details && row.details.length ? `<br>${row.details.map(escapeHtml).join("<br>")}` : ""}</li>`
      ).join("")}</ul><p>${report.passed || 0} passed · ${report.failed || 0} failed</p>` : `<p>No Rule CI tests found.</p>`;
      document.getElementById("rule-tests").innerHTML = html;
      if (writeOutput) {
        document.getElementById("build-output").textContent = rows.map(row => `${row.name}: ${row.status}`).join("\n") || "No Rule CI tests found.";
      }
    }

    async function renderReview(writeOutput) {
      const res = await fetch("/api/review");
      const payload = await res.json();
      if (!payload.ok) {
        document.getElementById("review").innerHTML = `<p>${escapeHtml(payload.error)}</p>`;
        if (writeOutput) document.getElementById("build-output").textContent = payload.error;
        return;
      }
      const report = payload.report;
      const summary = report.summary || {};
      const failed = Number(summary.rule_tests_failed || 0);
      const pending = Number(summary.drafts_pending || 0);
      const artifactDrifts = Number(summary.artifact_drifts || 0);
      const actions = report.build_preview?.actions || [];
      document.getElementById("review").innerHTML = `
        <div class="review-grid">
          <div class="metric"><strong>${pending}</strong><span>Pending drafts</span></div>
          <div class="metric"><strong class="${failed ? "bad" : "ok"}">${failed}</strong><span>Rule CI failures</span></div>
          <div class="metric"><strong class="${artifactDrifts ? "bad" : "ok"}">${artifactDrifts}</strong><span>Artifact drifts</span></div>
        </div>
        <p>${actions.length} build actions · ${(report.mirror_status?.warnings || []).length} warnings</p>
      `;
      if (writeOutput) {
        document.getElementById("build-output").textContent = [
          `Drafts pending: ${pending}`,
          `Rule CI failures: ${failed}`,
          `Artifact drifts: ${artifactDrifts}`,
          `Build actions: ${actions.length}`
        ].join("\n");
      }
    }

    function renderRules() {
      const rules = Array.isArray(state.imported_rules) ? state.imported_rules : [];
      document.getElementById("rules").innerHTML = rules.length ? `<ul>${rules.map(rule =>
        `<li>${escapeHtml(rule.kind)}<br>${escapeHtml(rule.path)}</li>`
      ).join("")}</ul>` : `<p>No rule files indexed.</p>`;
    }

    function renderExtractTargets() {
      const agents = Object.entries(state.project.agents || {}).filter(([, agent]) => agent.enabled);
      document.getElementById("extract-targets").innerHTML = agents.map(([name]) => `
        <label style="font-size:12px;color:var(--muted)">
          <input type="checkbox" class="extract-target" value="${escapeHtml(name)}" ${name === "codex" ? "checked" : ""}>
          ${escapeHtml(name)}
        </label>
      `).join("");
    }

    async function renderStore() {
      const res = await fetch("/api/catalog");
      const payload = await res.json();
      const validationRes = await fetch("/api/catalog/validate");
      const validation = await validationRes.json();
      catalogPackages = payload.ok ? payload.catalog.items || [] : [];
      const validationReport = validation.ok ? validation.report : { errors: 0, warnings: 0, rows: [] };
      const validationCard = `
        <div class="card" style="grid-column:1/-1">
          <h4>Catalog Health</h4>
          <p><strong class="${validationReport.errors ? "bad" : "ok"}">${validationReport.errors || 0}</strong> errors · ${validationReport.warnings || 0} warnings</p>
          ${validationReport.rows && validationReport.rows.length ? `<p>${validationReport.rows.map(row => `${escapeHtml(row.level)} ${escapeHtml(row.package_id)}: ${escapeHtml(row.message)}`).join("<br>")}</p>` : `<p>Metadata is ready for local installation.</p>`}
        </div>
      `;
      const catalogCards = catalogPackages.map(item => `
        <div class="card">
          <h4>${escapeHtml(item.package.id)}</h4>
          <p>${escapeHtml(item.package.description || item.package.body)}</p>
          <p><strong>Version:</strong> ${escapeHtml(item.package.version)}<br><strong>Source:</strong> ${escapeHtml(item.package.source_url || "local")}</p>
          ${item.package.tags && item.package.tags.length ? `<p>${item.package.tags.map(tag => `<span class="tag">${escapeHtml(tag)}</span>`).join(" ")}</p>` : ""}
          <p><strong>Kind:</strong> ${escapeHtml(item.package.kind)} · ${escapeHtml(item.package.scope)}</p>
          <button class="btn ${item.installed ? "" : "primary"}" ${item.installed ? "disabled" : ""} onclick="installCatalogPackage('${escapeAttr(item.package.id)}')">${item.installed ? "Installed" : "Install"}</button>
        </div>
      `).join("");
      const indexedSkillCards = state.skill_index.skills.map(skill => `
        <div class="card">
          <h4>${escapeHtml(skill.id)}</h4>
          <p>${escapeHtml(skill.description || "No description")}</p>
          <p><strong>Source:</strong> ${escapeHtml(skill.source_kind)}</p>
          <button class="btn" onclick="selectSkill('${escapeAttr(skill.id)}')">Select</button>
        </div>
      `).join("");
      document.getElementById("store-grid").innerHTML = `
        ${validationCard}
        ${catalogCards ? `<div class="card" style="grid-column:1/-1"><h4>Catalog Packages</h4><p>Install lightweight Skilllets into this project.</p></div>${catalogCards}` : ""}
        ${indexedSkillCards ? `<div class="card" style="grid-column:1/-1"><h4>Indexed Skills</h4><p>Reference existing Skills through Mirror mode.</p></div>${indexedSkillCards}` : ""}
      ` || `<div class="empty">No packages or skills available.</div>`;
    }

    function selectSkill(id) {
      selectedSkill = state.skill_index.skills.find(s => s.id === id);
      selectedAgent = null;
      renderInspector();
    }

    function selectAgent(name) {
      selectedAgent = name;
      document.querySelectorAll(".agent").forEach(el => el.classList.toggle("active", el.dataset.agent === name));
      renderInspector();
    }

    function renderInspector() {
      const target = document.getElementById("inspector");
      if (selectedSkill) {
        target.innerHTML = `
          <p><strong>${escapeHtml(selectedSkill.id)}</strong></p>
          <p>${escapeHtml(selectedSkill.description || "No description")}</p>
          <p>${escapeHtml(selectedSkill.source_path)}</p>
          ${selectedSkill.warnings && selectedSkill.warnings.length ? `<p><strong>Warnings</strong><br>${selectedSkill.warnings.map(escapeHtml).join("<br>")}</p>` : ""}
          <p>Drag this skill onto an agent, or select an agent and use Mirror.</p>
          ${selectedAgent ? `<button class="btn primary" id="mirror-selected">Mirror to ${escapeHtml(selectedAgent)}</button>` : ""}
        `;
        const button = document.getElementById("mirror-selected");
        if (button) button.addEventListener("click", () => mirror(selectedSkill.id, selectedAgent));
      } else if (selectedAgent) {
        target.innerHTML = `<p><strong>${escapeHtml(selectedAgent)}</strong></p><p>Select or drag a skill to create a project-scoped mirror.</p>`;
      } else {
        target.innerHTML = "Select a skill or agent.";
      }
    }

    async function mirror(skill, agent) {
      if (!skill || !agent) return;
      const res = await fetch("/api/mirror", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ skill, agent })
      });
      const result = await res.json();
      if (!result.ok) {
        document.getElementById("build-output").textContent = result.error;
        return;
      }
      document.getElementById("build-output").textContent = `Mirrored declaration added: ${skill} -> ${agent}`;
      await loadState();
    }

    async function setAgentEnabled(agent, enabled) {
      const res = await fetch("/api/agent/enabled", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ agent, enabled })
      });
      const result = await res.json();
      document.getElementById("build-output").textContent = result.ok ? `${enabled ? "Enabled" : "Disabled"} agent: ${agent}` : result.error;
      await loadState();
    }

    async function toggleSkillletTarget(id, agent) {
      const ref = (state.project.skilllets?.include || []).find(entry => entry.id === id);
      const enabledAgents = Object.entries(state.project.agents || {}).filter(([, cfg]) => cfg.enabled).map(([name]) => name);
      const current = ref && ref.targets && ref.targets.length ? [...ref.targets] : enabledAgents;
      const next = current.includes(agent)
        ? current.filter(name => name !== agent)
        : [...current, agent].sort();
      const res = await fetch("/api/skilllet/targets", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ id, targets: next })
      });
      const result = await res.json();
      document.getElementById("build-output").textContent = result.ok ? `Updated targets: ${id} -> ${next.join(", ") || "none"}` : result.error;
      await loadState();
      await renderTargetMatrix();
    }

    async function previewBuild() {
      const res = await fetch("/api/build/preview", { method: "POST" });
      const result = await res.json();
      document.getElementById("build-output").textContent = (result.actions || []).length
        ? result.actions.map(action => `- ${action}`).join("\n")
        : result.text;
    }

    async function approveDraft(id) {
      const res = await fetch("/api/draft/approve", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ id })
      });
      const result = await res.json();
      document.getElementById("build-output").textContent = result.ok ? `Approved draft: ${id}` : result.error;
      await loadState();
      await renderReview(false);
    }

    async function rejectDraft(id) {
      const res = await fetch("/api/draft/reject", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ id })
      });
      const result = await res.json();
      document.getElementById("build-output").textContent = result.ok ? `Rejected draft: ${id}` : result.error;
      await loadState();
      await renderReview(false);
    }

    async function syncMirrors() {
      const res = await fetch("/api/sync", { method: "POST" });
      const result = await res.json();
      document.getElementById("build-output").textContent = result.text;
      await loadState();
    }

    async function importArtifacts() {
      const res = await fetch("/api/artifacts/import", { method: "POST" });
      const result = await res.json();
      document.getElementById("build-output").textContent = result.text;
      await loadState();
    }

    async function extractDrafts() {
      const text = document.getElementById("extract-text").value;
      const targets = Array.from(document.querySelectorAll(".extract-target:checked")).map(input => input.value);
      const res = await fetch("/api/extract", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ text, targets })
      });
      const result = await res.json();
      document.getElementById("build-output").textContent = result.ok ? result.text : result.error;
      await loadState();
    }

    async function installCatalogPackage(id) {
      const targets = Array.from(document.querySelectorAll(".extract-target:checked")).map(input => input.value);
      const res = await fetch("/api/catalog/install", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ id, targets })
      });
      const result = await res.json();
      document.getElementById("build-output").textContent = result.ok ? `Installed package: ${id}` : result.error;
      await loadState();
    }

    function escapeHtml(value) {
      return String(value ?? "").replace(/[&<>"']/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
    }

    function escapeAttr(value) {
      return String(value ?? "").replace(/\\/g, "\\\\").replace(/'/g, "\\'");
    }

    function shortAgentLabel(value) {
      const labels = {
        "claude-code": "Claude",
        "codex": "Codex"
      };
      return labels[value] || value;
    }

    document.getElementById("search").addEventListener("input", renderSkills);
    document.getElementById("preview").addEventListener("click", previewBuild);
    document.getElementById("review-button").addEventListener("click", () => renderReview(true));
    document.getElementById("run-rule-tests").addEventListener("click", () => renderRuleTests(true));
    document.getElementById("sync").addEventListener("click", syncMirrors);
    document.getElementById("import-artifacts").addEventListener("click", importArtifacts);
    document.getElementById("extract-drafts").addEventListener("click", extractDrafts);
    document.getElementById("refresh").addEventListener("click", loadState);
    document.getElementById("open-store").addEventListener("click", () => document.getElementById("store").classList.add("open"));
    document.getElementById("close-store").addEventListener("click", () => document.getElementById("store").classList.remove("open"));
    window.addEventListener("resize", drawEdges);
    loadState();
  </script>
</body>
</html>
"##;
