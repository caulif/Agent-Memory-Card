use super::*;
use std::fs;

fn write_skill(path: &Path, body: &str) {
    fs::create_dir_all(path).expect("create skill dir");
    fs::write(
        path.join("SKILL.md"),
        format!(
            "---\nname: demo\ndescription: Use when testing mirror status behavior\n---\n{body}\n"
        ),
    )
    .expect("write skill");
}

#[test]
fn status_distinguishes_source_updated_and_target_drifted() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    let source = root.join("source-skill");
    write_skill(&source, "# Demo");

    let mut project = config::default_project_config(root);
    project.skills.mirrors.push(config::MirrorDecl {
        reference: "local:demo".to_string(),
        targets: vec!["codex".to_string()],
    });
    config::save_project_config(root, &project).expect("save project");
    config::save_skill_index(
        root,
        &config::SkillIndex {
            generated_at: "test".to_string(),
            skills: vec![SkillRecord {
                id: "local:demo".to_string(),
                name: "demo".to_string(),
                description: "Use when testing mirror status behavior".to_string(),
                source_path: fsutil::path_to_slash(&source),
                source_kind: "referenced".to_string(),
                source_hash: fsutil::sha256_dir(&source).expect("source hash"),
                warnings: Vec::new(),
            }],
        },
    )
    .expect("save index");

    sync_project(root).expect("sync");
    let synced = status_project(root).expect("status");
    assert_eq!(synced.rows[0].status, "synced");

    write_skill(&source, "# Demo\nUpdated");
    let source_updated = status_project(root).expect("status");
    assert_eq!(source_updated.rows[0].status, "source updated");

    let target = root.join(".agents").join("skills").join("demo");
    fs::write(target.join("LOCAL.md"), "local edit").expect("target edit");
    let both = status_project(root).expect("status");
    assert_eq!(both.rows[0].status, "source updated + target drifted");
}

#[test]
fn render_instructions_includes_targeted_skilllets() {
    let mut skilllets = BTreeMap::new();
    skilllets.insert(
        "project:use-axios".to_string(),
        SkillletRecord {
            schema_version: 1,
            id: "project:use-axios".to_string(),
            title: "Use Axios".to_string(),
            kind: "preference".to_string(),
            scope: "project".to_string(),
            body: "Use Axios for frontend requests.".to_string(),
            brief: "Use Axios for similar future work.".to_string(),
            tags: vec!["frontend".to_string()],
            language: "en".to_string(),
            activation: "always-on".to_string(),
            trigger_description: None,
            source_project: None,
            extraction: None,
            approved_from: None,
            evidence: None,
            merge_history: Vec::new(),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        },
    );
    let refs = vec![config::SkillletRef {
        id: "project:use-axios".to_string(),
        targets: vec!["codex".to_string()],
        scope: Some("project".to_string()),
    }];

    let codex = render_instructions("codex", &[], &refs, &skilllets);
    let claude = render_instructions("claude-code", &[], &refs, &skilllets);

    assert!(codex.contains("Use Axios for frontend requests."));
    assert!(!claude.contains("Use Axios for frontend requests."));
}

#[test]
fn sync_project_applies_skilllet_agent_targets_to_generated_artifacts() {
    let temp = tempfile::tempdir().expect("tempdir");
    skilllet::add_skilllet(
        temp.path(),
        "project:use-axios",
        "Use Axios",
        "Use Axios for frontend HTTP requests.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("add skilllet");

    sync_project(temp.path()).expect("sync codex target");
    let codex = fs::read_to_string(temp.path().join("AGENTS.md")).expect("codex instructions");
    let claude = fs::read_to_string(temp.path().join("CLAUDE.md")).expect("claude instructions");
    assert!(codex.contains("Use Axios for frontend HTTP requests."));
    assert!(!claude.contains("Use Axios for frontend HTTP requests."));

    skilllet::set_skilllet_targets(
        temp.path(),
        "project:use-axios",
        vec!["claude-code".to_string()],
    )
    .expect("retarget skilllet");
    sync_project(temp.path()).expect("sync claude target");

    let codex = fs::read_to_string(temp.path().join("AGENTS.md")).expect("codex instructions");
    let claude = fs::read_to_string(temp.path().join("CLAUDE.md")).expect("claude instructions");
    assert!(!codex.contains("Use Axios for frontend HTTP requests."));
    assert!(claude.contains("Use Axios for frontend HTTP requests."));
}

#[test]
fn render_instructions_only_includes_compile_enabled_always_on_skilllets() {
    let mut skilllets = BTreeMap::new();
    skilllets.insert(
        "project:always-on".to_string(),
        test_skilllet_with_artifact_kind(
            "project:always-on",
            "Always On",
            "Use Axios for frontend HTTP requests.",
            "always_on_rule",
            true,
        ),
    );
    skilllets.insert(
        "project:workflow".to_string(),
        test_skilllet_with_artifact_kind(
            "project:workflow",
            "Workflow",
            "Create a SKILL.md draft for UI handoff prompts.",
            "workflow_skill",
            false,
        ),
    );
    skilllets.insert(
        "project:review-only".to_string(),
        test_skilllet_with_artifact_kind(
            "project:review-only",
            "Review Only",
            "Review AI project improvements before compiling them.",
            "review_only",
            false,
        ),
    );
    let refs = vec![
        config::SkillletRef {
            id: "project:always-on".to_string(),
            targets: vec!["codex".to_string()],
            scope: Some("project".to_string()),
        },
        config::SkillletRef {
            id: "project:workflow".to_string(),
            targets: vec!["codex".to_string()],
            scope: Some("project".to_string()),
        },
        config::SkillletRef {
            id: "project:review-only".to_string(),
            targets: vec!["codex".to_string()],
            scope: Some("project".to_string()),
        },
    ];

    let codex = render_instructions("codex", &[], &refs, &skilllets);

    assert!(codex.contains("Use Axios for frontend HTTP requests."));
    assert!(!codex.contains("Create a SKILL.md draft"));
    assert!(!codex.contains("Review AI project improvements"));
}

#[test]
fn build_compiles_procedure_skilllets_as_agent_skills() {
    let temp = tempfile::tempdir().expect("tempdir");
    skilllet::add_skilllet(
        temp.path(),
        "project:frontend-workflow",
        "Frontend Workflow",
        "Use Axios for frontend HTTP requests and Bun for scripts.",
        "procedure",
        "project",
        vec!["claude-code".to_string()],
    )
    .expect("add skilllet");

    sync_project(temp.path()).expect("sync");

    let instructions =
        fs::read_to_string(temp.path().join("CLAUDE.md")).expect("claude instructions");
    let skill = fs::read_to_string(
        temp.path()
            .join(".claude")
            .join("skills")
            .join("frontend-workflow")
            .join("SKILL.md"),
    )
    .expect("compiled skill");
    let reference = fs::read_to_string(
        temp.path()
            .join(".claude")
            .join("skills")
            .join("frontend-workflow")
            .join("references")
            .join("project-frontend-workflow.md"),
    )
    .expect("compiled reference");

    assert!(!instructions.contains("Use Axios for frontend HTTP requests"));
    assert!(skill.contains("name: frontend-workflow"));
    assert!(skill.contains("description: Use when"));
    assert!(reference.contains("Use Axios for frontend HTTP requests"));
}

#[test]
fn build_compiles_hook_skilllets_into_claude_settings_local() {
    let temp = tempfile::tempdir().expect("tempdir");
    skilllet::add_skilllet(
        temp.path(),
        "project:run-clippy-before-session-end",
        "Run Clippy Before Session End",
        "cargo clippy --quiet -- -D warnings",
        "workflow",
        "project",
        vec!["claude-code".to_string()],
    )
    .expect("add skilllet");

    let path = temp
        .path()
        .join(".agent-kernel")
        .join("skilllets")
        .join("project")
        .join("run-clippy-before-session-end.yml");
    let text = fs::read_to_string(&path).expect("skilllet yaml");
    let mut record: SkillletRecord = serde_yaml::from_str(&text).expect("parse skilllet yaml");
    record.activation = "hook".to_string();
    record.tags = vec![
        "hook:event:session-end".to_string(),
        "hook:matcher:git commit".to_string(),
        "workflow".to_string(),
    ];
    fs::write(
        &path,
        serde_yaml::to_string(&record).expect("serialize skilllet"),
    )
    .expect("rewrite skilllet yaml");

    sync_project(temp.path()).expect("sync");

    let settings = fs::read_to_string(temp.path().join(".claude/settings.local.json"))
        .expect("settings local");
    assert!(settings.contains("\"SessionEnd\""));
    assert!(settings.contains("cargo clippy --quiet -- -D warnings"));
    assert!(settings.contains("\"matcher\": \"git commit\""));
    assert!(!settings.contains("AGENTS.md"));
}

fn test_skilllet_with_artifact_kind(
    id: &str,
    title: &str,
    body: &str,
    artifact_kind: &str,
    compile_enabled: bool,
) -> SkillletRecord {
    SkillletRecord {
        schema_version: 1,
        id: id.to_string(),
        title: title.to_string(),
        kind: "preference".to_string(),
        scope: "project".to_string(),
        body: body.to_string(),
        brief: String::new(),
        tags: Vec::new(),
        language: "en".to_string(),
        activation: "model-decision".to_string(),
        trigger_description: None,
        source_project: None,
        extraction: Some(ExtractionMetadata {
            classification: Some(crate::extract::classify::KnowledgeClassification {
                signal: "preference".to_string(),
                artifact_kind: artifact_kind.to_string(),
                activation: if artifact_kind == "always_on_rule" {
                    "always_on".to_string()
                } else {
                    "manual".to_string()
                },
                hardness: "low".to_string(),
                control: "default".to_string(),
                rationale: "test".to_string(),
                tags: Vec::new(),
            }),
            suggested_action: Some(crate::candidate::ExtractionAction {
                action: "new_candidate".to_string(),
                route: artifact_kind.to_string(),
                target_record: None,
                compile_enabled: Some(compile_enabled),
                record_id: None,
                similarity: None,
                reason: None,
                rationale: Some("test route".to_string()),
            }),
            ..ExtractionMetadata::default()
        }),
        approved_from: None,
        evidence: None,
        merge_history: Vec::new(),
        created_at: "now".to_string(),
        updated_at: "now".to_string(),
    }
}

#[test]
fn build_preview_warns_when_instruction_artifact_exceeds_budget() {
    let temp = tempfile::tempdir().expect("tempdir");
    let oversized_body = "Keep this instruction.\n".repeat(1800);
    skilllet::add_skilllet(
        temp.path(),
        "project:large-context",
        "Large Context",
        &oversized_body,
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("add skilllet");

    let report = build_project(temp.path(), true).expect("build preview");
    let rendered = report.render();

    assert!(rendered.contains("exceeds 32768 byte budget"));
}

#[test]
fn build_writes_custom_rules_artifact_from_targeted_skilllet() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut project = config::default_project_config(temp.path());
    project.agents.insert(
        "custom-agent".to_string(),
        config::AgentConfig {
            enabled: true,
            exports: config::AgentExports {
                instructions: None,
                skills_dir: None,
                rules_dir: Some(".custom-agent/rules".to_string()),
            },
        },
    );
    config::save_project_config(temp.path(), &project).expect("save project");

    skilllet::add_skilllet(
        temp.path(),
        "project:custom-rule",
        "Custom Rule",
        "Use strict TypeScript in custom-agent edits.",
        "constraint",
        "project",
        vec!["custom-agent".to_string()],
    )
    .expect("add skilllet");

    build_project(temp.path(), false).expect("build");

    let rule_path = temp
        .path()
        .join(".custom-agent")
        .join("rules")
        .join("agent-kernel.md");
    let text = fs::read_to_string(rule_path).expect("custom rule");
    assert!(text.contains("# Agent Kernel Rules"));
    assert!(text.contains("Use strict TypeScript in custom-agent edits."));
}

#[test]
fn status_reports_generated_artifact_drift() {
    let temp = tempfile::tempdir().expect("tempdir");
    skilllet::add_skilllet(
        temp.path(),
        "project:codex-rule",
        "Codex Rule",
        "Use Bun for JavaScript package management and scripts.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("add skilllet");

    sync_project(temp.path()).expect("sync");
    let synced = status_project(temp.path()).expect("status");
    assert!(
        synced
            .artifact_rows
            .iter()
            .any(|row| row.path.ends_with("AGENTS.md") && row.status == "synced")
    );

    fs::write(temp.path().join("AGENTS.md"), "manual edit").expect("manual edit");
    let drifted = status_project(temp.path()).expect("status");

    assert!(
        drifted
            .artifact_rows
            .iter()
            .any(|row| row.path.ends_with("AGENTS.md") && row.status == "artifact drifted")
    );
}

#[test]
fn sync_project_blocks_generated_artifact_drift_before_overwrite() {
    let temp = tempfile::tempdir().expect("tempdir");
    skilllet::add_skilllet(
        temp.path(),
        "project:codex-rule",
        "Codex Rule",
        "Use Bun for JavaScript package management and scripts.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("add skilllet");

    sync_project(temp.path()).expect("initial sync");
    let path = temp.path().join("AGENTS.md");
    fs::write(&path, "manual edit that should be imported first").expect("manual edit");

    let err = sync_project(temp.path()).expect_err("drifted artifact must not be overwritten");

    assert!(err.to_string().contains("artifact drift"));
    let preserved = fs::read_to_string(&path).expect("read agents");
    assert_eq!(preserved, "manual edit that should be imported first");
}

#[test]
fn import_artifact_drifts_creates_reviewable_draft_from_manual_edit() {
    let temp = tempfile::tempdir().expect("tempdir");
    skilllet::add_skilllet(
        temp.path(),
        "project:codex-rule",
        "Codex Rule",
        "Use Bun for JavaScript package management and scripts.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("add skilllet");
    sync_project(temp.path()).expect("sync");
    let path = temp.path().join("AGENTS.md");
    let mut text = fs::read_to_string(&path).expect("read agents");
    text.push_str("\nAlways use Vitest for frontend unit tests.\n");
    fs::write(&path, text).expect("manual edit");

    let report = import_artifact_drifts(temp.path()).expect("import drifts");
    let drafts = draft::load_drafts(temp.path()).expect("drafts");

    assert_eq!(report.created, 1);
    assert_eq!(drafts.len(), 1);
    assert_eq!(drafts[0].targets, vec!["codex"]);
    assert!(
        drafts[0]
            .body
            .contains("Always use Vitest for frontend unit tests.")
    );
}

#[test]
fn mirrored_skill_includes_attached_skilllet_supplement() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    let source = root.join("source-skill");
    write_skill(&source, "# Demo Skill");

    config::save_skill_index(
        root,
        &config::SkillIndex {
            generated_at: "test".to_string(),
            skills: vec![SkillRecord {
                id: "local:demo".to_string(),
                name: "demo".to_string(),
                description: "Use when testing skilllet supplements".to_string(),
                source_path: fsutil::path_to_slash(&source),
                source_kind: "referenced".to_string(),
                source_hash: fsutil::sha256_dir(&source).expect("source hash"),
                warnings: Vec::new(),
            }],
        },
    )
    .expect("save index");
    config::add_mirror(root, "local:demo", "codex").expect("mirror");
    skilllet::add_skilllet(
        root,
        "project:extra-context",
        "Extra Context",
        "Use Vitest for frontend unit tests.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("add skilllet");
    config::add_skill_supplement(root, "local:demo", "project:extra-context")
        .expect("attach supplement");

    sync_project(root).expect("sync");

    let supplement = fs::read_to_string(
        root.join(".agents")
            .join("skills")
            .join("demo")
            .join("AGENT_KERNEL_SKILLLETS.md"),
    )
    .expect("supplement");
    assert!(supplement.contains("Extra Context"));
    assert!(supplement.contains("Use Vitest for frontend unit tests."));
}
