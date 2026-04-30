use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use axum::extract::State;
use axum::response::Html;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;

use crate::build;
use crate::config;
use crate::fsutil;

#[derive(Clone)]
struct AppState {
    project_root: Arc<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct MirrorRequest {
    skill: String,
    agent: String,
}

#[derive(Debug, Serialize)]
struct ApiState {
    project_root: String,
    project: config::ProjectConfig,
    skill_index: config::SkillIndex,
    imported_rules: serde_yaml::Value,
}

pub async fn serve(project: PathBuf, port: u16, open_browser: bool) -> Result<()> {
    let root = fsutil::normalize_project_root(&project)?;
    let state = AppState {
        project_root: Arc::new(root),
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/api/state", get(api_state))
        .route("/api/mirror", post(api_mirror))
        .route("/api/build/preview", post(api_build_preview))
        .route("/api/status", get(api_status))
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

async fn api_state(State(state): State<AppState>) -> Json<ApiState> {
    let root = state.project_root.as_ref();
    let project = config::load_or_default_project_config(root)
        .unwrap_or_else(|_| config::default_project_config(root));
    let skill_index = config::load_skill_index(root).unwrap_or_default();
    let rules_path = config::kernel_dir(root).join("imported-rules.yml");
    let imported_rules = fs::read_to_string(rules_path)
        .ok()
        .and_then(|text| serde_yaml::from_str(&text).ok())
        .unwrap_or(serde_yaml::Value::Sequence(Vec::new()));

    Json(ApiState {
        project_root: fsutil::path_to_slash(root),
        project,
        skill_index,
        imported_rules,
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

async fn api_build_preview(State(state): State<AppState>) -> Json<serde_json::Value> {
    match build::preview_as_json(state.project_root.as_ref()) {
        Ok(value) => Json(value),
        Err(error) => Json(serde_json::json!({ "preview": true, "text": error.to_string() })),
    }
}

async fn api_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    match build::status_project(state.project_root.as_ref()) {
        Ok(report) => Json(serde_json::to_value(report).unwrap_or_else(
            |error| serde_json::json!({ "rows": [], "warnings": [error.to_string()] }),
        )),
        Err(error) => Json(serde_json::json!({ "rows": [], "warnings": [error.to_string()] })),
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
        <h3>Declared Mirrors</h3>
        <div id="mirrors"></div>
      </section>
      <section>
        <h3>Mirror Status</h3>
        <div id="mirror-status"></div>
      </section>
      <section>
        <h3>Imported Rules</h3>
        <div id="rules"></div>
      </section>
    </div>

    <footer>
      <button class="btn primary" id="preview">Preview Build</button>
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

    const agentPositions = {
      "codex": "codex",
      "claude-code": "claude-code",
      "cursor": "cursor",
      "cline": "cline"
    };

    async function loadState() {
      const res = await fetch("/api/state");
      state = await res.json();
      document.getElementById("project-path").textContent = state.project_root;
      renderSkills();
      renderCanvas();
      renderMirrors();
      await renderStatus();
      renderRules();
      renderStore();
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

    function renderCanvas() {
      const canvas = document.getElementById("canvas");
      canvas.querySelectorAll(".agent").forEach(n => n.remove());
      Object.entries(state.project.agents).forEach(([name, agent]) => {
        const node = document.createElement("div");
        node.className = `node agent ${agentPositions[name] || ""}`;
        node.dataset.agent = name;
        node.innerHTML = `<h2>${escapeHtml(name)}</h2><p>${agent.enabled ? "Enabled" : "Disabled"}${agent.exports.skills_dir ? " · skills" : ""}</p>`;
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

    async function renderStatus() {
      const res = await fetch("/api/status");
      const status = await res.json();
      const rows = status.rows || [];
      document.getElementById("mirror-status").innerHTML = rows.length ? `<ul>${rows.map(row =>
        `<li><strong>${escapeHtml(row.skill)}</strong><br>${escapeHtml(row.agent)} · ${escapeHtml(row.status)}</li>`
      ).join("")}</ul>` : `<p>No mirror status yet.</p>`;
    }

    function renderRules() {
      const rules = Array.isArray(state.imported_rules) ? state.imported_rules : [];
      document.getElementById("rules").innerHTML = rules.length ? `<ul>${rules.map(rule =>
        `<li>${escapeHtml(rule.kind)}<br>${escapeHtml(rule.path)}</li>`
      ).join("")}</ul>` : `<p>No rule files indexed.</p>`;
    }

    function renderStore() {
      document.getElementById("store-grid").innerHTML = state.skill_index.skills.map(skill => `
        <div class="card">
          <h4>${escapeHtml(skill.id)}</h4>
          <p>${escapeHtml(skill.description || "No description")}</p>
          <p><strong>Source:</strong> ${escapeHtml(skill.source_kind)}</p>
          <button class="btn" onclick="selectSkill('${escapeAttr(skill.id)}')">Select</button>
        </div>
      `).join("") || `<div class="empty">No skills available.</div>`;
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

    async function previewBuild() {
      const res = await fetch("/api/build/preview", { method: "POST" });
      const result = await res.json();
      document.getElementById("build-output").textContent = result.text;
    }

    function escapeHtml(value) {
      return String(value ?? "").replace(/[&<>"']/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
    }

    function escapeAttr(value) {
      return String(value ?? "").replace(/\\/g, "\\\\").replace(/'/g, "\\'");
    }

    document.getElementById("search").addEventListener("input", renderSkills);
    document.getElementById("preview").addEventListener("click", previewBuild);
    document.getElementById("refresh").addEventListener("click", loadState);
    document.getElementById("open-store").addEventListener("click", () => document.getElementById("store").classList.add("open"));
    document.getElementById("close-store").addEventListener("click", () => document.getElementById("store").classList.remove("open"));
    window.addEventListener("resize", drawEdges);
    loadState();
  </script>
</body>
</html>
"##;
