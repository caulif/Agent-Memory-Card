use super::*;
use crate::draft;
use std::fs;

#[test]
fn import_file_stores_redacted_observation() {
    let temp = tempfile::tempdir().expect("tempdir");
    let transcript = temp.path().join("session.jsonl");
    fs::write(
        &transcript,
        r#"{"type":"user","message":"Always use Vitest. token=abc123456789xyz"}"#,
    )
    .expect("write transcript");

    let report = import_observation_file(
        temp.path(),
        &transcript,
        "claude-code-session",
        Some("claude-code"),
    )
    .expect("import");
    let observations = load_observations(temp.path()).expect("load observations");

    assert_eq!(report.created, 1);
    assert_eq!(observations.len(), 1);
    assert_eq!(observations[0].agent.as_deref(), Some("claude-code"));
    assert!(observations[0].body.contains("Always use Vitest"));
    assert!(!observations[0].body.contains("abc123456789xyz"));
}

#[test]
fn import_jsonl_extracts_message_text_without_wrappers() {
    let temp = tempfile::tempdir().expect("tempdir");
    let transcript = temp.path().join("session.jsonl");
    fs::write(
        &transcript,
        r#"{"type":"user","message":"Always use Vitest for frontend unit tests."}"#,
    )
    .expect("write transcript");

    import_observation_file(temp.path(), &transcript, "codex-session", Some("codex"))
        .expect("import");
    let observations = load_observations(temp.path()).expect("observations");

    assert_eq!(
        observations[0].body,
        "Always use Vitest for frontend unit tests."
    );
}

#[test]
fn import_jsonl_skips_system_and_local_command_noise() {
    let temp = tempfile::tempdir().expect("tempdir");
    let transcript = temp.path().join("session.jsonl");
    fs::write(
            &transcript,
            r#"{"timestamp":"2026-01-01","type":"session_meta","payload":{"base_instructions":{"text":"Always obey system prompt."},"cwd":"C:\\demo"}}
{"type":"user","message":"以后所有 Rust 项目必须运行 cargo test。"}
{"type":"user","isMeta":true,"message":"<local-command-stdout>noise</local-command-stdout>"}"#,
        )
        .expect("write transcript");

    import_observation_file(temp.path(), &transcript, "codex-session", Some("codex"))
        .expect("import");
    let observations = load_observations(temp.path()).expect("observations");

    assert!(observations[0].body.contains("cargo test"));
    assert!(!observations[0].body.contains("system prompt"));
    assert!(!observations[0].body.contains("local-command"));
}

#[test]
fn import_file_skips_existing_observation() {
    let temp = tempfile::tempdir().expect("tempdir");
    let transcript = temp.path().join("session.jsonl");
    fs::write(
        &transcript,
        r#"{"type":"user","message":"Always use Vitest for frontend unit tests."}"#,
    )
    .expect("write transcript");

    let first = import_observation_file(temp.path(), &transcript, "codex-session", Some("codex"))
        .expect("first import");
    let second = import_observation_file(temp.path(), &transcript, "codex-session", Some("codex"))
        .expect("second import");

    assert_eq!(first.created, 1);
    assert_eq!(second.created, 0);
    assert_eq!(second.skipped, 1);
    assert_eq!(
        load_observations(temp.path()).expect("observations").len(),
        1
    );
}

#[test]
fn incremental_import_skips_unchanged_sessions_and_reads_only_append() {
    let project = tempfile::tempdir().expect("project");
    let home = tempfile::tempdir().expect("home");
    let session = home
        .path()
        .join(".codex")
        .join("sessions")
        .join("2026")
        .join("05")
        .join("02")
        .join("rollout.jsonl");
    fs::create_dir_all(session.parent().expect("session parent")).expect("session dir");
    fs::write(
        &session,
        format!(
            "{}\n",
            serde_json::json!({
                "type": "session_meta",
                "payload": { "cwd": project.path().to_string_lossy() }
            })
        ),
    )
    .expect("meta");
    fs::OpenOptions::new()
        .append(true)
        .open(&session)
        .expect("open")
        .write_all(
            br#"{"type":"user","message":"Always use Vitest for frontend unit tests."}
"#,
        )
        .expect("append first");

    let first = import_local_conversations_incremental(project.path(), home.path()).expect("first");
    let second =
        import_local_conversations_incremental(project.path(), home.path()).expect("second");
    fs::OpenOptions::new()
        .append(true)
        .open(&session)
        .expect("open")
        .write_all(
            serde_json::json!({
                "type": "user",
                "message": "以后所有 Rust 项目必须运行 cargo test。"
            })
            .to_string()
            .as_bytes(),
        )
        .expect("append second");
    fs::OpenOptions::new()
        .append(true)
        .open(&session)
        .expect("open")
        .write_all(b"\n")
        .expect("append newline");
    let third = import_local_conversations_incremental(project.path(), home.path()).expect("third");
    let observations = load_observations(project.path()).expect("observations");

    assert_eq!(first.created, 1);
    assert_eq!(second.created, 0);
    assert_eq!(third.created, 1);
    assert_eq!(observations.len(), 2);
    assert!(observations.iter().any(|item| item.body.contains("Vitest")));
    assert!(
        observations
            .iter()
            .any(|item| { item.body.contains("cargo test") && !item.body.contains("Vitest") })
    );
}

#[test]
fn incremental_reader_skips_unchanged_file_by_metadata_without_rehashing() {
    let temp = tempfile::tempdir().expect("tempdir");
    let session = temp.path().join("session.jsonl");
    fs::write(
        &session,
        r#"{"type":"user","message":"Always use Vitest for frontend unit tests."}"#,
    )
    .expect("session");
    let metadata = fs::metadata(&session).expect("metadata");
    let size = metadata.len();
    let modified_at_unix_ms = metadata_modified_unix_ms(&metadata);
    let mut index = ObservationIndex {
        version: 1,
        sources: vec![ObservationSourceState {
            source_path: fsutil::path_to_slash(&session),
            agent: "codex".to_string(),
            source_kind: "codex-session".to_string(),
            processed_bytes: size,
            source_hash: "legacy-hash-that-no-longer-needs-content-read".to_string(),
            modified_at_unix_ms,
            updated_at: Utc::now().to_rfc3339(),
        }],
    };
    let file = ConversationFile {
        agent: "codex".to_string(),
        source_kind: "codex-session".to_string(),
        path: session,
        project_path: None,
    };

    let text = read_incremental_conversation_text(&file, &mut index).expect("read");

    assert!(text.is_none());
    assert_eq!(
        index.sources[0].source_hash,
        "legacy-hash-that-no-longer-needs-content-read"
    );
}

#[test]
fn incremental_import_filters_sessions_to_matching_project() {
    let project = tempfile::tempdir().expect("project");
    let other = tempfile::tempdir().expect("other");
    let home = tempfile::tempdir().expect("home");
    let session = home
        .path()
        .join(".claude")
        .join("projects")
        .join("other")
        .join("session.jsonl");
    fs::create_dir_all(session.parent().expect("session parent")).expect("session dir");
    fs::write(
        &session,
        format!(
            "{}\n{}",
            serde_json::json!({
                "type": "user",
                "cwd": other.path().to_string_lossy(),
                "message": { "role": "user", "content": "Always use Vitest." }
            }),
            serde_json::json!({
                "type": "user",
                "message": { "role": "user", "content": "Always use Axios." }
            })
        ),
    )
    .expect("session");

    let report =
        import_local_conversations_incremental(project.path(), home.path()).expect("import");

    assert_eq!(report.created, 0);
    assert!(
        load_observations(project.path())
            .expect("observations")
            .is_empty()
    );
}

#[test]
fn discover_local_conversation_files_finds_claude_and_codex_jsonl() {
    let home = tempfile::tempdir().expect("home");
    let claude = home
        .path()
        .join(".claude")
        .join("projects")
        .join("demo")
        .join("session.jsonl");
    let codex = home
        .path()
        .join(".codex")
        .join("sessions")
        .join("2026")
        .join("05")
        .join("01")
        .join("rollout-1.jsonl");
    fs::create_dir_all(claude.parent().expect("claude parent")).expect("claude dir");
    fs::create_dir_all(codex.parent().expect("codex parent")).expect("codex dir");
    fs::write(&claude, "claude").expect("claude file");
    fs::write(&codex, "codex").expect("codex file");

    let files = discover_local_conversation_files(home.path()).expect("discover");

    assert!(files.iter().any(|item| item.agent == "claude-code"));
    assert!(files.iter().any(|item| item.agent == "codex"));
}

#[test]
fn synthesize_observations_creates_reviewable_candidates() {
    let temp = tempfile::tempdir().expect("tempdir");
    let transcript = temp.path().join("session.jsonl");
    fs::write(
        &transcript,
        r#"{"type":"user","message":"以后所有 Rust 项目必须先运行 cargo test 再提交。"}"#,
    )
    .expect("write transcript");
    import_observation_file(temp.path(), &transcript, "codex-session", Some("codex"))
        .expect("import");

    let report = synthesize_observations_to_drafts(
        temp.path(),
        vec!["codex".to_string(), "claude-code".to_string()],
        false,
    )
    .expect("synthesize");

    assert_eq!(report.created, 1);
    assert_eq!(
        report.drafts,
        vec!["project:所有-rust-项目必须先运行-cargo-test-再提交"]
    );
    let candidates = candidate::load_candidates(temp.path()).expect("candidates");
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].targets, vec!["claude-code", "codex"]);
    assert!(candidates[0].body.contains("cargo test"));
    assert!(candidates[0].evidence.contains("observation:obs:codex"));
    assert!(draft::load_drafts(temp.path()).expect("drafts").is_empty());
}

#[test]
fn synthesize_dry_run_does_not_write_drafts() {
    let temp = tempfile::tempdir().expect("tempdir");
    let transcript = temp.path().join("session.jsonl");
    fs::write(
        &transcript,
        r#"{"type":"user","message":"Always use Vitest for frontend unit tests."}"#,
    )
    .expect("write transcript");
    import_observation_file(
        temp.path(),
        &transcript,
        "claude-code-session",
        Some("claude-code"),
    )
    .expect("import");

    let report = synthesize_observations_to_drafts(temp.path(), vec!["codex".to_string()], true)
        .expect("synthesize");

    assert_eq!(report.created, 0);
    assert_eq!(report.candidates, 1);
    assert_eq!(report.candidate_drafts, vec!["project:use-vitest"]);
    assert!(draft::load_drafts(temp.path()).expect("drafts").is_empty());
}

#[test]
fn agent_synthesis_material_uses_prefiltered_candidates_not_raw_observations() {
    let temp = tempfile::tempdir().expect("tempdir");
    let observations = vec![ObservationRecord {
        id: "obs:codex:fast".to_string(),
        source_kind: "codex-session".to_string(),
        source_path: "session.jsonl".to_string(),
        agent: Some("codex".to_string()),
        body: format!(
            "Always use Vitest for frontend unit tests.\n{}",
            "ordinary UI bug chatter without durable value. ".repeat(400)
        ),
        evidence: "Imported from session.jsonl".to_string(),
        redacted: false,
        created_at: Utc::now().to_rfc3339(),
    }];
    let raw = synthesis_material(&observations, 80_000);

    let (prefilter, filtered) = prefilter_agent_synthesis_material(
        temp.path(),
        &observations,
        vec!["codex".to_string()],
        "1 observation",
    )
    .expect("prefilter");

    assert_eq!(prefilter.candidates.len(), 1);
    assert!(filtered.contains("Vitest"));
    assert!(!filtered.contains("ordinary UI bug chatter"));
    assert!(filtered.len() < raw.len() / 4);
}

#[test]
fn evolve_local_conversations_imports_and_synthesizes_candidates() {
    let temp = tempfile::tempdir().expect("project");
    let home = tempfile::tempdir().expect("home");
    let codex = home
        .path()
        .join(".codex")
        .join("sessions")
        .join("2026")
        .join("05")
        .join("01")
        .join("rollout-1.jsonl");
    fs::create_dir_all(codex.parent().expect("codex parent")).expect("codex dir");
    fs::write(
        &codex,
        r#"{"type":"user","message":"Always run cargo clippy before pushing."}"#,
    )
    .expect("write codex session");

    let report =
        evolve_local_conversations(temp.path(), home.path(), vec!["codex".to_string()], false)
            .expect("evolve");

    assert_eq!(report.imported, 1);
    assert_eq!(report.drafts_created, 1);
    assert_eq!(
        report.drafts,
        vec!["project:always-run-cargo-clippy-before-pushing"]
    );
    let candidates = candidate::load_candidates(temp.path()).expect("candidates");
    assert!(candidates[0].body.contains("cargo clippy"));
    assert!(draft::load_drafts(temp.path()).expect("drafts").is_empty());
}

#[test]
fn auto_evolve_from_discovered_files_reuses_one_conversation_file_list_for_projects() {
    let home = tempfile::tempdir().expect("home");
    let project_a = tempfile::tempdir().expect("project a");
    let project_b = tempfile::tempdir().expect("project b");
    crate::project_registry::add_project(home.path(), project_a.path()).expect("add a");
    crate::project_registry::add_project(home.path(), project_b.path()).expect("add b");
    let session_a = home.path().join("session-a.jsonl");
    let session_b = home.path().join("session-b.jsonl");
    fs::write(
        &session_a,
        format!(
            "{}\n{}",
            serde_json::json!({"cwd": project_a.path()}),
            serde_json::json!({"type":"user","message":"Always run cargo test before merging."})
        ),
    )
    .expect("write a");
    fs::write(
            &session_b,
            format!(
                "{}\n{}",
                serde_json::json!({"cwd": project_b.path()}),
                serde_json::json!({"type":"user","message":"Prefer Bun scripts for JavaScript tooling."})
            ),
        )
        .expect("write b");
    let files = vec![
        ConversationFile {
            agent: "codex".to_string(),
            source_kind: "codex-session".to_string(),
            path: session_a,
            project_path: Some(project_a.path().to_path_buf()),
        },
        ConversationFile {
            agent: "codex".to_string(),
            source_kind: "codex-session".to_string(),
            path: session_b,
            project_path: Some(project_b.path().to_path_buf()),
        },
    ];

    let report = auto_evolve_registered_projects_from_files(
        home.path(),
        &files,
        "local",
        vec!["codex".to_string()],
    )
    .expect("auto evolve");

    assert_eq!(report.projects, 2);
    assert_eq!(report.imported, 2);
    assert_eq!(
        candidate::load_candidates(project_a.path())
            .expect("candidates a")
            .len(),
        1
    );
    assert_eq!(
        candidate::load_candidates(project_b.path())
            .expect("candidates b")
            .len(),
        1
    );
}

#[test]
fn parses_agent_json_candidates_from_plain_or_fenced_output() {
    let output = r#"Here are candidates:
[
  {
    "title": "Prefer Bun",
    "body": "Use Bun for JavaScript package management.",
    "kind": "preference",
    "scope": "project",
    "confidence": 0.91,
    "reason": "Repeated preference."
  }
]
"#;

    let candidates = parse_agent_candidates(output).expect("parse");

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].title, "Prefer Bun");
    assert!(is_usable_agent_candidate(&candidates[0]));
}

#[test]
fn rejects_low_confidence_agent_candidates() {
    let candidate = AgentSkillletCandidate {
        title: "Maybe".to_string(),
        body: "Maybe do this once.".to_string(),
        brief: None,
        tags: Vec::new(),
        language: None,
        kind: "procedure".to_string(),
        scope: "project".to_string(),
        confidence: Some(0.4),
        reason: None,
    };

    assert!(!is_usable_agent_candidate(&candidate));
}

#[test]
fn agent_synthesis_candidates_must_pass_local_future_value_gate() {
    let candidates = vec![
        AgentSkillletCandidate {
            title: "Warm Personality".to_string(),
            body: "以后保持热情积极、有自己的品味，让用户感觉更舒服。".to_string(),
            brief: None,
            tags: Vec::new(),
            language: None,
            kind: "preference".to_string(),
            scope: "project".to_string(),
            confidence: Some(0.95),
            reason: Some("Sounds nice but has no operational trigger.".to_string()),
        },
        AgentSkillletCandidate {
            title: "Prefer Bun".to_string(),
            body: "Always use Bun for JavaScript package management and scripts.".to_string(),
            brief: None,
            tags: Vec::new(),
            language: None,
            kind: "preference".to_string(),
            scope: "project".to_string(),
            confidence: Some(0.91),
            reason: Some("Repeated project preference.".to_string()),
        },
    ];

    let filtered = filter_agent_candidates_through_local_gate(
        tempfile::tempdir().expect("tempdir").path(),
        candidates,
        "agent synthesis",
    )
    .expect("filter");

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].0.title, "Prefer Bun");
    assert_eq!(
        filtered[0].2.action, "new_candidate",
        "agent output should still carry a local action decision"
    );
}
