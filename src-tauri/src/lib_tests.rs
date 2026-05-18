use super::*;
    use crate::commands::core::{
        add_project, approve_draft, enforce_tauri_kernel_policy,
        enforce_tauri_kernel_policy_for_project, gc_candidates, plan_kernel_command_input,
        reject_draft, sync_project_for_test,
    };

    #[test]
    fn app_state_uses_existing_registry() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project = temp.path().join("demo");
        std::fs::create_dir_all(project.join(".agent-kernel")).expect("project");
        project_registry::add_project(temp.path(), &project).expect("add project");

        let state = app_state_for_home(temp.path()).expect("state");

        assert_eq!(state.registry.projects.len(), 1);
        assert_eq!(state.registry.projects[0].name, "demo");
    }

    #[test]
    fn project_snapshot_loads_reviewable_state() {
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
                evidence: "desktop test".to_string(),
                confidence: Some(0.9),
                reason: Some("Repeated correction".to_string()),
                matched_template: Some("builtin:prefer-bun".to_string()),
                extraction: agent_kernel::candidate::ExtractionMetadata::default(),
            },
        )
        .expect("draft");

        let snapshot = load_project_snapshot(temp.path()).expect("snapshot");

        assert_eq!(snapshot.drafts.len(), 1);
        assert_eq!(snapshot.target_matrix.agents, vec!["claude-code", "codex"]);
    }

    #[test]
    fn project_dashboard_loads_lightweight_counts_for_first_paint() {
        let home = tempfile::tempdir().expect("home");
        let project = tempfile::tempdir().expect("project");
        agent_kernel::candidate::add_candidate(
            project.path(),
            agent_kernel::candidate::NewCandidate {
                id: "project:prefer-bun-candidate".to_string(),
                title: "Prefer Bun".to_string(),
                body: "Use Bun for JavaScript package management and scripts.".to_string(),
                brief: None,
                tags: Vec::new(),
                language: None,
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "desktop test candidate".to_string(),
                confidence: Some(0.91),
                reason: Some("Repeated correction".to_string()),
                matched_template: Some("builtin:prefer-bun".to_string()),
                source_observations: Vec::new(),
                extraction: agent_kernel::candidate::ExtractionMetadata::default(),
            },
        )
        .expect("candidate");
        draft::add_draft(
            project.path(),
            draft::NewDraft {
                id: "project:prefer-bun".to_string(),
                title: "Prefer Bun".to_string(),
                body: "Use Bun for JavaScript package management and scripts.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "desktop test".to_string(),
                confidence: Some(0.9),
                reason: Some("Repeated correction".to_string()),
                matched_template: None,
                extraction: agent_kernel::candidate::ExtractionMetadata::default(),
            },
        )
        .expect("draft");
        memory_card::add_memory_card(
            project.path(),
            "project:use-axios",
            "Use Axios",
            "Use Axios for frontend HTTP requests.",
            "preference",
            "project",
            vec!["codex".to_string()],
        )
        .expect("memory_card");
        memory_card::add_memory_card(
            home.path(),
            "global:prefer-bun",
            "Prefer Bun",
            "Use Bun for JavaScript package management and scripts.",
            "preference",
            "global",
            vec!["codex".to_string()],
        )
        .expect("global memory_card");
        let transcript = project.path().join("session.jsonl");
        std::fs::write(
            &transcript,
            r#"{"type":"user","message":"Always use Vitest for frontend unit tests."}"#,
        )
        .expect("transcript");
        observation::import_observation_file(
            project.path(),
            &transcript,
            "codex-session",
            Some("codex"),
        )
        .expect("observation");

        let dashboard =
            app_service::load_project_dashboard(project.path(), home.path()).expect("dashboard");

        assert_eq!(
            dashboard.project_path,
            fsutil::path_to_slash(project.path())
        );
        assert_eq!(dashboard.candidate_count, 1);
        assert_eq!(dashboard.draft_count, 1);
        assert_eq!(dashboard.memory_card_count, 1);
        assert_eq!(dashboard.observation_count, 1);
        assert_eq!(dashboard.global_memory_card_count, 1);
        assert_eq!(dashboard.enabled_agents, vec!["claude-code", "codex"]);
        assert_eq!(dashboard.warning_count, 0);

        let inbox = app_service::load_project_review_inbox(project.path()).expect("inbox");
        assert_eq!(inbox.project_path, dashboard.project_path);
        assert_eq!(inbox.drafts.len(), 1);

        let library = app_service::load_project_memory_card_library(project.path(), home.path())
            .expect("library");
        assert_eq!(library.memory_cards.len(), 1);
        assert_eq!(library.global_memory_cards.len(), 1);

        let assignment =
            app_service::load_project_assignment_view(project.path()).expect("assignment");
        assert_eq!(assignment.enabled_agents, vec!["claude-code", "codex"]);
        assert_eq!(assignment.target_matrix.rows.len(), 1);

        let quality = app_service::load_project_quality_view(project.path()).expect("quality");
        assert_eq!(quality.project_path, dashboard.project_path);
        assert_eq!(quality.rule_ci.failed, 0);
    }

    #[test]
    fn startup_auto_evolve_processes_registered_agent_project_once() {
        let home = tempfile::tempdir().expect("home");
        let project = tempfile::tempdir().expect("project");
        project_registry::add_project(home.path(), project.path()).expect("add project");
        let session = home
            .path()
            .join(".codex")
            .join("sessions")
            .join("2026")
            .join("05")
            .join("02")
            .join("rollout.jsonl");
        std::fs::create_dir_all(session.parent().expect("session parent")).expect("session dir");
        std::fs::write(
            &session,
            format!(
                "{}\n{}",
                serde_json::json!({
                    "type": "session_meta",
                    "payload": { "cwd": project.path().to_string_lossy() }
                }),
                serde_json::json!({
                    "type": "user",
                    "message": "Always use Vitest for frontend unit tests."
                })
            ),
        )
        .expect("session");

        let first = observation::auto_evolve_registered_projects(
            home.path(),
            "local",
            vec!["codex".to_string()],
        )
        .expect("first");
        let second = observation::auto_evolve_registered_projects(
            home.path(),
            "local",
            vec!["codex".to_string()],
        )
        .expect("second");

        assert_eq!(first.imported, 1);
        assert_eq!(first.drafts_created, 1);
        assert_eq!(second.imported, 0);
        assert_eq!(
            agent_kernel::candidate::load_candidates(project.path())
                .expect("candidates")
                .len(),
            1
        );
        assert_eq!(draft::load_drafts(project.path()).expect("drafts").len(), 0);
    }

    #[test]
    fn task_store_tracks_running_steps_and_completion() {
        let store = DesktopTaskStore::default();

        store.begin("启动", "建立本地记忆索引", "正在发现本地项目", 10);
        store.step(
            "启动",
            "读取历史会话",
            "正在读取 Claude Code / Codex 历史",
            55,
        );
        let running = store.snapshot();

        assert!(running.running);
        assert_eq!(running.key, "启动");
        assert_eq!(running.label, "读取历史会话");
        assert_eq!(running.percent, 55);
        assert_eq!(running.details.len(), 2);
        assert_eq!(running.details[0].label, "建立本地记忆索引");
        assert_eq!(running.details[1].label, "读取历史会话");

        store.finish("启动", "增量整理完成");
        let finished = store.snapshot();

        assert!(!finished.running);
        assert_eq!(finished.percent, 100);
        assert_eq!(finished.message, "增量整理完成");
        assert_eq!(
            finished.details.last().expect("finish detail").label,
            "完成"
        );
    }

    #[test]
    fn task_store_behaves_like_job_manager_with_logs_and_result_summary() {
        let store = DesktopTaskStore::default();

        let job_id = store.begin_job(
            "整理历史",
            "读取本地会话",
            "正在扫描 Claude Code / Codex 增量会话",
            12,
        );
        store.job_step(
            &job_id,
            "过滤候选",
            "已过滤一次性闲聊，只保留高价值候选",
            46,
        );
        store.finish_job(&job_id, "已生成 8 条候选建议，2 条需要人工审阅");

        let finished = store.snapshot();

        assert_eq!(finished.job_id, job_id);
        assert_eq!(finished.key, "整理历史");
        assert_eq!(finished.stage, "完成");
        assert!(!finished.running);
        assert!(finished.started_at.is_some());
        assert!(finished.finished_at.is_some());
        assert_eq!(
            finished.result_summary.as_deref(),
            Some("已生成 8 条候选建议，2 条需要人工审阅")
        );
        assert_eq!(finished.logs.len(), 3);
        assert_eq!(finished.logs[0].stage, "读取本地会话");
        assert_eq!(finished.logs[1].stage, "过滤候选");
        assert_eq!(finished.logs[2].stage, "完成");
        assert!(finished.logs.iter().all(|entry| entry.timestamp.len() > 10));
    }

    #[test]
    fn job_start_payload_exposes_job_id_for_long_running_commands() {
        let store = DesktopTaskStore::default();

        let job_id = store.begin_job("整理历史", "发现会话文件", "后台任务已创建", 10);
        let payload = job_start_from_status(&store.snapshot(), "已加入后台整理队列");

        assert_eq!(payload.job_id, job_id);
        assert_eq!(payload.key, "整理历史");
        assert_eq!(payload.stage, "发现会话文件");
        assert_eq!(payload.message, "已加入后台整理队列");
        assert!(payload.accepted);
        let json = serde_json::to_value(&payload).expect("serialize payload");
        assert_eq!(json["job_id"], job_id);
    }

    #[test]
    fn task_store_keeps_recent_job_history_and_accepts_cancel_requests() {
        let store = DesktopTaskStore::default();

        let first = store.begin_job("扫描", "扫描本地项目", "正在准备扫描", 10);
        store.finish_job(&first, "扫描完成");
        let second = store.begin_job("整理历史", "发现会话文件", "后台任务已创建", 12);
        store.job_step(&second, "导入增量观察", "正在读取新增对话", 34);

        let cancel = store.request_cancel(&second).expect("cancel request");
        let history = store.recent_jobs();

        assert_eq!(cancel.job_id, second);
        assert!(cancel.cancel_requested);
        assert_eq!(cancel.lifecycle, "cancelling");
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].job_id, second);
        assert_eq!(history[1].job_id, first);
        assert_eq!(history[1].lifecycle, "completed");
    }

    #[test]
    fn task_store_persists_recent_job_history_to_local_jsonl() {
        let home = tempfile::tempdir().expect("home");
        let store = DesktopTaskStore::with_job_history(home.path());

        let job_id = store.begin_job("同步", "编译 Agent 产物", "正在写入 Agent 文件", 20);
        store.finish_job(&job_id, "已同步生成产物");

        let restored = DesktopTaskStore::with_job_history(home.path());
        let history = restored.recent_jobs();

        assert_eq!(history.len(), 1);
        assert_eq!(history[0].job_id, job_id);
        assert_eq!(history[0].lifecycle, "completed");
        assert_eq!(history[0].result_summary.as_deref(), Some("已同步生成产物"));
        assert!(
            home.path()
                .join(".agent-kernel")
                .join("jobs")
                .join("history.jsonl")
                .exists()
        );
    }

    #[test]
    fn task_store_persists_replay_payload_for_precise_retry() {
        let home = tempfile::tempdir().expect("home");
        let store = DesktopTaskStore::with_job_history(home.path());

        let job_id = store.begin_job("整理历史", "发现会话文件", "正在准备可重试的整理任务", 12);
        store.attach_replay(
            &job_id,
            DesktopJobReplay {
                command: "evolve_project".to_string(),
                args: serde_json::json!({
                    "projectPath": "C:/repo/demo",
                    "targets": ["codex"],
                    "dryRun": false,
                    "engine": "claude-code"
                }),
            },
        );
        store.finish_job(&job_id, "整理失败：外部引擎不可用");

        let restored = DesktopTaskStore::with_job_history(home.path());
        let history = restored.recent_jobs();

        assert_eq!(history[0].job_id, job_id);
        let replay = history[0].replay.as_ref().expect("replay payload");
        assert_eq!(replay.command, "evolve_project");
        assert_eq!(replay.args["engine"], "claude-code");
        assert_eq!(replay.args["targets"][0], "codex");
    }

    #[test]
    fn task_store_can_describe_replayable_memory_card_fusion_jobs() {
        let home = tempfile::tempdir().expect("home");
        let store = DesktopTaskStore::with_job_history(home.path());

        let job_id = store.begin_job(
            "融合 Memory Card",
            "创建融合草稿",
            "正在准备可重试的 Memory Card 融合任务",
            15,
        );
        store.attach_replay(
            &job_id,
            DesktopJobReplay {
                command: "fuse_memory_cards_to_draft".to_string(),
                args: serde_json::json!({
                    "projectPath": "C:/repo/demo",
                    "input": {
                        "id": "draft:fused-ui-rules",
                        "title": "融合 UI 规则",
                        "sources": ["project:ui-fluid", "project:ui-polish"],
                        "targets": ["claude-code", "codex"]
                    },
                    "confirmedPolicy": agent_kernel::kernel::KernelPolicy::agent_managed()
                }),
            },
        );

        let job = store.recent_jobs().first().cloned().expect("job history");
        let replay = job.replay.expect("replay payload");
        assert_eq!(job.key, "融合 Memory Card");
        assert_eq!(replay.command, "fuse_memory_cards_to_draft");
        assert_eq!(replay.args["input"]["sources"][1], "project:ui-polish");
    }

    #[test]
    fn kernel_plan_helper_routes_manual_approve_to_review() {
        let input = KernelPlanInput {
            command: agent_kernel::kernel::KernelCommand::ApproveDraft {
                id: "project:prefer-bun".to_string(),
            },
            policy: agent_kernel::kernel::KernelPolicy::manual(),
            project_path: None,
            payload: serde_json::json!({}),
        };

        let decision = plan_kernel_command_input(input).expect("plan");

        assert_eq!(
            decision.disposition,
            agent_kernel::kernel::KernelDisposition::ReviewRequired
        );
        assert!(decision.requires_human_review);
    }

    #[test]
    fn kernel_plan_helper_issues_project_bound_decision_token() {
        let temp = tempfile::tempdir().expect("tempdir");
        let input = KernelPlanInput {
            command: agent_kernel::kernel::KernelCommand::ApproveDraft {
                id: "project:prefer-bun".to_string(),
            },
            policy: agent_kernel::kernel::KernelPolicy::manual(),
            project_path: Some(fsutil::path_to_slash(temp.path())),
            payload: serde_json::json!({ "id": "project:prefer-bun" }),
        };

        let decision = plan_kernel_command_input(input).expect("plan");

        assert!(decision.decision_token.is_some());
        let token = decision.decision_token.expect("token");
        agent_kernel::kernel::verify_decision_token(
            temp.path(),
            &token,
            &agent_kernel::kernel::KernelCommand::ApproveDraft {
                id: "project:prefer-bun".to_string(),
            },
            &serde_json::json!({ "id": "project:prefer-bun" }),
        )
        .expect("token verifies");
    }

    #[test]
    fn manual_policy_blocks_high_risk_tauri_mutations_without_confirmation() {
        let command = agent_kernel::kernel::KernelCommand::ApproveDraft {
            id: "project:prefer-bun".to_string(),
        };

        let err = enforce_tauri_kernel_policy(command, None)
            .expect_err("manual mode should block approval without confirmation");

        assert!(err.contains("review"));
        assert!(err.contains("approve-draft"));
    }

    #[test]
    fn confirmed_policy_allows_high_risk_tauri_mutations() {
        let command = agent_kernel::kernel::KernelCommand::ApproveDraft {
            id: "project:prefer-bun".to_string(),
        };

        let decision = enforce_tauri_kernel_policy(
            command,
            Some(agent_kernel::kernel::KernelPolicy::agent_managed()),
        )
        .expect("confirmed agent managed policy should allow approval");

        assert_eq!(decision.command, "approve-draft");
    }

    #[test]
    fn approve_draft_tauri_command_blocks_without_confirmed_policy() {
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
                evidence: "desktop test".to_string(),
                confidence: Some(0.9),
                reason: Some("Repeated correction".to_string()),
                matched_template: None,
                extraction: agent_kernel::candidate::ExtractionMetadata::default(),
            },
        )
        .expect("draft");

        let err = approve_draft(
            fsutil::path_to_slash(temp.path()),
            "project:prefer-bun".to_string(),
            None,
            None,
        )
        .expect_err("unconfirmed approval should be blocked");

        assert!(err.contains("approve-draft"));
        assert_eq!(draft::load_drafts(temp.path()).expect("drafts").len(), 1);
        assert_eq!(
            memory_card::load_memory_cards(temp.path())
                .expect("memory_cards")
                .len(),
            0
        );
        let audit = agent_kernel::kernel::load_audit_entries(temp.path()).expect("audit");
        assert_eq!(audit.len(), 1);
        assert_eq!(audit[0].command, "approve-draft");
        assert_eq!(
            audit[0].status,
            agent_kernel::kernel::KernelAuditStatus::Blocked
        );
    }

    #[test]
    fn agent_managed_approve_draft_tauri_command_requires_decision_token() {
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
                evidence: "desktop test".to_string(),
                confidence: Some(0.9),
                reason: Some("Repeated correction".to_string()),
                matched_template: None,
                extraction: agent_kernel::candidate::ExtractionMetadata::default(),
            },
        )
        .expect("draft");

        let err = approve_draft(
            fsutil::path_to_slash(temp.path()),
            "project:prefer-bun".to_string(),
            Some(agent_kernel::kernel::KernelPolicy::agent_managed()),
            None,
        )
        .expect_err("agent-managed approval still needs a backend decision token");

        assert!(err.contains("decision token is required"));
        assert_eq!(draft::load_drafts(temp.path()).expect("drafts").len(), 1);
        assert_eq!(memory_card::load_memory_cards(temp.path()).expect("memory_cards").len(), 0);
        let audit = agent_kernel::kernel::load_audit_entries(temp.path()).expect("audit");
        assert_eq!(audit.len(), 1);
        assert_eq!(
            audit[0].status,
            agent_kernel::kernel::KernelAuditStatus::Blocked
        );
    }

    #[test]
    fn agent_managed_evolve_project_requires_decision_token_when_writing() {
        let temp = tempfile::tempdir().expect("tempdir");

        let err = enforce_tauri_kernel_policy_for_project(
            &fsutil::path_to_slash(temp.path()),
            agent_kernel::kernel::KernelCommand::EvolveProject {
                dry_run: false,
                engine: "local".to_string(),
            },
            serde_json::json!({
                "targets": ["codex", "claude-code"],
                "dry_run": false,
                "engine": "local"
            }),
            Some(agent_kernel::kernel::KernelPolicy::agent_managed()),
            None,
        )
        .expect_err("non-dry-run evolution still needs a backend decision token");

        assert!(err.contains("decision token is required"));
        let audit = agent_kernel::kernel::load_audit_entries(temp.path()).expect("audit");
        assert_eq!(audit.len(), 1);
        assert_eq!(
            audit[0].status,
            agent_kernel::kernel::KernelAuditStatus::Blocked
        );
    }

    #[test]
    fn approve_draft_tauri_command_executes_with_confirmed_policy_and_token() {
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
                evidence: "desktop test".to_string(),
                confidence: Some(0.9),
                reason: Some("Repeated correction".to_string()),
                matched_template: None,
                extraction: agent_kernel::candidate::ExtractionMetadata::default(),
            },
        )
        .expect("draft");
        let token = agent_kernel::kernel::create_decision_token(
            temp.path(),
            &agent_kernel::kernel::KernelCommand::ApproveDraft {
                id: "project:prefer-bun".to_string(),
            },
            &serde_json::json!({ "id": "project:prefer-bun" }),
            &agent_kernel::kernel::KernelPolicy::manual(),
        )
        .expect("token");

        let ack = approve_draft(
            fsutil::path_to_slash(temp.path()),
            "project:prefer-bun".to_string(),
            Some(agent_kernel::kernel::KernelPolicy::agent_managed()),
            Some(token),
        )
        .expect("confirmed approval should execute");

        assert_eq!(ack.project_path, fsutil::path_to_slash(temp.path()));
        assert_eq!(draft::load_drafts(temp.path()).expect("drafts").len(), 0);
        assert_eq!(memory_card::load_memory_cards(temp.path()).expect("memory_cards").len(), 1);
        let audit = agent_kernel::kernel::load_audit_entries(temp.path()).expect("audit");
        assert_eq!(audit.len(), 1);
        assert_eq!(audit[0].command, "approve-draft");
        assert_eq!(
            audit[0].status,
            agent_kernel::kernel::KernelAuditStatus::Authorized
        );
    }

    #[test]
    fn gc_candidates_tauri_command_hides_low_value_candidates() {
        let temp = tempfile::tempdir().expect("tempdir");
        agent_kernel::candidate::add_candidate(
            temp.path(),
            agent_kernel::candidate::NewCandidate {
                id: "project:do-not-touch-rust".to_string(),
                title: "不要改 Rust".to_string(),
                body: "不要改 Rust".to_string(),
                brief: None,
                tags: Vec::new(),
                language: None,
                kind: "constraint".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "不要改 Rust。".to_string(),
                confidence: Some(0.94),
                reason: Some("test".to_string()),
                matched_template: Some("constraint".to_string()),
                source_observations: Vec::new(),
                extraction: agent_kernel::candidate::ExtractionMetadata::default(),
            },
        )
        .expect("candidate");

        let report = gc_candidates(
            fsutil::path_to_slash(temp.path()),
            Some(agent_kernel::kernel::KernelPolicy::agent_managed()),
            None,
        )
        .expect("gc candidates");

        assert_eq!(report.hidden, vec!["project:do-not-touch-rust"]);
        assert_eq!(
            agent_kernel::candidate::list_visible_candidates(temp.path())
                .expect("visible")
                .len(),
            0
        );
    }

    #[test]
    fn reject_draft_tauri_command_executes_with_confirmed_policy_and_token() {
        let temp = tempfile::tempdir().expect("tempdir");
        draft::add_draft(
            temp.path(),
            draft::NewDraft {
                id: "project:delete-me".to_string(),
                title: "Delete Me".to_string(),
                body: "Temporary draft that should be removed from review.".to_string(),
                kind: "observation".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "desktop test".to_string(),
                confidence: Some(0.8),
                reason: Some("Low value after review".to_string()),
                matched_template: None,
                extraction: agent_kernel::candidate::ExtractionMetadata::default(),
            },
        )
        .expect("draft");
        let token = agent_kernel::kernel::create_decision_token(
            temp.path(),
            &agent_kernel::kernel::KernelCommand::RejectDraft {
                id: "project:delete-me".to_string(),
            },
            &serde_json::json!({ "id": "project:delete-me" }),
            &agent_kernel::kernel::KernelPolicy::manual(),
        )
        .expect("token");

        let ack = reject_draft(
            fsutil::path_to_slash(temp.path()),
            "project:delete-me".to_string(),
            Some(agent_kernel::kernel::KernelPolicy::agent_managed()),
            Some(token),
        )
        .expect("confirmed rejection should execute");

        assert_eq!(ack.project_path, fsutil::path_to_slash(temp.path()));
        assert_eq!(draft::load_drafts(temp.path()).expect("drafts").len(), 0);
        let audit = agent_kernel::kernel::load_audit_entries(temp.path()).expect("audit");
        assert_eq!(audit.len(), 1);
        assert_eq!(audit[0].command, "reject-draft");
        assert_eq!(
            audit[0].status,
            agent_kernel::kernel::KernelAuditStatus::Authorized
        );
    }

    #[test]
    fn sync_project_tauri_command_blocks_without_confirmed_policy() {
        let temp = tempfile::tempdir().expect("tempdir");

        let err = sync_project_for_test(fsutil::path_to_slash(temp.path()), None, None)
            .expect_err("unconfirmed sync should be blocked");

        assert!(err.contains("compile-project"));
    }

    #[test]
    fn add_project_tauri_command_blocks_without_confirmed_policy() {
        let home = tempfile::tempdir().expect("home");
        let project = tempfile::tempdir().expect("project");

        let err = add_project(
            Some(fsutil::path_to_slash(home.path())),
            fsutil::path_to_slash(project.path()),
            None,
        )
        .expect_err("unconfirmed registry writes should be blocked");

        assert!(err.contains("add-project"));
        let state = app_state_for_home(home.path()).expect("state");
        assert_eq!(state.registry.projects.len(), 0);
    }
