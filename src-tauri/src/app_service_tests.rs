use super::*;

    fn seed_draft(project_root: &Path, id: &str) {
        draft::add_draft(
            project_root,
            draft::NewDraft {
                id: id.to_string(),
                title: "Prefer Bun".to_string(),
                body: "Use Bun for JavaScript package management and scripts.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "service test".to_string(),
                confidence: Some(0.9),
                reason: Some("Repeated correction".to_string()),
                matched_template: None,
                extraction: agent_kernel::candidate::ExtractionMetadata::default(),
            },
        )
        .expect("seed draft");
    }

    fn seed_memory_card(project_root: &Path, id: &str) {
        memory_card::add_memory_card(
            project_root,
            id,
            "Use Axios",
            "Use Axios for frontend HTTP requests.",
            "preference",
            "project",
            vec!["codex".to_string()],
        )
        .expect("seed memory_card");
    }

    #[test]
    fn approve_draft_promotes_to_memory_card_and_removes_from_inbox() {
        let temp = tempfile::tempdir().expect("tempdir");
        seed_draft(temp.path(), "project:prefer-bun");

        approve_draft(temp.path(), "project:prefer-bun").expect("approve draft");
        let memory_cards = memory_card::load_memory_cards(temp.path()).expect("memory_cards");

        assert_eq!(memory_cards[0].id, "project:prefer-bun");
        assert_eq!(load_project_review_inbox(temp.path()).expect("inbox").drafts.len(), 0);
        assert_eq!(memory_cards.len(), 1);
    }

    #[test]
    fn reject_draft_removes_draft_without_creating_memory_card() {
        let temp = tempfile::tempdir().expect("tempdir");
        seed_draft(temp.path(), "project:reject-me");

        reject_draft(temp.path(), "project:reject-me").expect("reject draft");

        assert_eq!(load_project_review_inbox(temp.path()).expect("inbox").drafts.len(), 0);
        assert_eq!(memory_card::load_memory_cards(temp.path()).expect("memory_cards").len(), 0);
    }

    #[test]
    fn update_draft_maps_editor_input_and_persists_fields() {
        let temp = tempfile::tempdir().expect("tempdir");
        seed_draft(temp.path(), "project:editable");

        let updated = update_draft(
            temp.path(),
            "project:editable",
            DraftUpdateInput {
                title: Some("Prefer Bun".to_string()),
                body: Some("Use Bun for JavaScript package management.".to_string()),
                brief: Some("Use Bun".to_string()),
                tags: Some(vec!["tooling".to_string()]),
                language: Some("en".to_string()),
                kind: Some("preference".to_string()),
                scope: Some("project".to_string()),
                targets: Some(Vec::new()),
            },
        )
        .expect("update draft");

        assert_eq!(updated.title, "Prefer Bun");
        assert_eq!(updated.brief, "Use Bun");
        assert_eq!(updated.tags, vec!["tooling"]);
        assert_eq!(updated.targets, Vec::<String>::new());
    }

    #[test]
    fn merge_drafts_creates_combined_draft_and_removes_sources() {
        let temp = tempfile::tempdir().expect("tempdir");
        seed_draft(temp.path(), "project:first");
        seed_draft(temp.path(), "project:second");

        merge_drafts(
            temp.path(),
            DraftMergeInput {
                id: "project:merged".to_string(),
                title: "Merged".to_string(),
                sources: vec!["project:first".to_string(), "project:second".to_string()],
                targets: vec!["codex".to_string()],
            },
        )
        .expect("merge drafts");

        let drafts = draft::load_drafts(temp.path()).expect("drafts");
        assert_eq!(drafts.len(), 1);
        let merged = drafts
            .iter()
            .find(|draft| draft.id == "project:merged")
            .expect("merged draft");
        assert!(merged.body.contains("Use Bun"));
        assert!(!drafts.iter().any(|draft| draft.id == "project:first"));
        assert!(!drafts.iter().any(|draft| draft.id == "project:second"));
    }

    #[test]
    fn assignment_services_update_agent_and_memory_card_targets() {
        let temp = tempfile::tempdir().expect("tempdir");
        seed_memory_card(temp.path(), "project:use-axios");

        set_agent_enabled(temp.path(), "claude-code", false).expect("disable agent");
        set_memory_card_targets(
            temp.path(),
            "project:use-axios",
            vec!["claude-code".to_string()],
        )
        .expect("set targets");

        let assignment = load_project_assignment_view(temp.path()).expect("assignment");
        assert!(!assignment.enabled_agents.contains(&"claude-code".to_string()));
        let row = assignment
            .target_matrix
            .rows
            .iter()
            .find(|row| row.memory_card_id == "project:use-axios")
            .expect("memory_card row");
        assert_eq!(row.targets.get("claude-code"), Some(&true));
    }

    #[test]
    fn update_memory_card_maps_editor_input_and_persists_fields() {
        let temp = tempfile::tempdir().expect("tempdir");
        seed_memory_card(temp.path(), "project:editable");

        let updated = update_memory_card(
            temp.path(),
            "project:editable",
            MemoryCardUpdateInput {
                title: Some("Use Axios Everywhere".to_string()),
                body: Some("Use Axios for every frontend HTTP request.".to_string()),
                brief: Some("Axios standard".to_string()),
                tags: Some(vec!["frontend".to_string()]),
                language: Some("en".to_string()),
                kind: Some("preference".to_string()),
                scope: Some("project".to_string()),
            },
        )
        .expect("update memory_card");

        assert_eq!(updated.title, "Use Axios Everywhere");
        assert_eq!(updated.brief, "Axios standard");
        assert_eq!(updated.tags, vec!["frontend"]);
    }

    #[test]
    fn global_memory_card_services_promote_and_install_with_targets() {
        let source = tempfile::tempdir().expect("source");
        let target = tempfile::tempdir().expect("target");
        let home = tempfile::tempdir().expect("home");
        seed_memory_card(source.path(), "project:review-before-sync");

        let global = promote_memory_card_to_global(source.path(), home.path(), "project:review-before-sync")
            .expect("promote global");
        let installed = install_global_memory_card_to_project(
            target.path(),
            home.path(),
            &global.id,
            vec!["codex".to_string()],
        )
        .expect("install global");

        assert_eq!(global.id, "global:review-before-sync");
        assert_eq!(installed.id, "global:review-before-sync");
        assert_eq!(memory_card::load_memory_cards(target.path()).expect("memory_cards").len(), 1);
        let config = config::load_or_default_project_config(target.path()).expect("config");
        assert_eq!(config.memory_cards.include[0].targets, vec!["codex"]);
    }

    #[test]
    fn merge_memory_cards_service_creates_combined_memory_card() {
        let temp = tempfile::tempdir().expect("tempdir");
        seed_memory_card(temp.path(), "project:ui-a");
        seed_memory_card(temp.path(), "project:ui-b");

        merge_memory_cards(
            temp.path(),
            MemoryCardMergeInput {
                id: "project:ui-merged".to_string(),
                title: "Merged UI".to_string(),
                sources: vec!["project:ui-a".to_string(), "project:ui-b".to_string()],
                targets: vec!["codex".to_string()],
            },
        )
        .expect("merge memory_cards");

        let merged = memory_card::load_memory_cards(temp.path())
            .expect("memory_cards")
            .into_iter()
            .find(|record| record.id == "project:ui-merged")
            .expect("merged memory_card");
        assert_eq!(merged.id, "project:ui-merged");
        assert!(merged.body.contains("Use Axios"));
    }

    #[test]
    fn attach_memory_card_service_updates_skill_supplements() {
        let temp = tempfile::tempdir().expect("tempdir");
        seed_memory_card(temp.path(), "project:use-axios");
        config::save_skill_index(
            temp.path(),
            &config::SkillIndex {
                generated_at: "test".to_string(),
                skills: vec![config::SkillRecord {
                    id: "skill:frontend".to_string(),
                    name: "Frontend".to_string(),
                    description: "Frontend workflow".to_string(),
                    source_path: ".agents/skills/frontend".to_string(),
                    source_kind: "project".to_string(),
                    source_hash: "hash".to_string(),
                    warnings: Vec::new(),
                }],
            },
        )
        .expect("skill index");

        attach_memory_card_to_skill(temp.path(), "project:use-axios", "skill:frontend")
            .expect("attach memory_card");

        let config = config::load_or_default_project_config(temp.path()).expect("config");
        assert_eq!(config.skills.supplements[0].skill, "skill:frontend");
        assert_eq!(config.skills.supplements[0].memory_cards[0], "project:use-axios");
    }

    #[test]
    fn skill_library_shows_links_and_recommendations() {
        let temp = tempfile::tempdir().expect("tempdir");
        config::save_skill_index(
            temp.path(),
            &config::SkillIndex {
                generated_at: "test".to_string(),
                skills: vec![config::SkillRecord {
                    id: "skill:frontend".to_string(),
                    name: "Frontend".to_string(),
                    description: "Use when improving React frontend workflows".to_string(),
                    source_path: ".agents/skills/frontend".to_string(),
                    source_kind: "project".to_string(),
                    source_hash: "hash".to_string(),
                    warnings: Vec::new(),
                }],
            },
        )
        .expect("skill index");
        config::add_mirror(temp.path(), "skill:frontend", "codex").expect("mirror skill");
        seed_memory_card(temp.path(), "project:use-axios");
        memory_card::add_memory_card(
            temp.path(),
            "project:frontend-workflow",
            "Frontend Workflow",
            "Run focused React tests before syncing frontend Skills.",
            "procedure",
            "project",
            vec!["codex".to_string()],
        )
        .expect("workflow card");
        attach_memory_card_to_skill(temp.path(), "project:use-axios", "skill:frontend")
            .expect("attach memory_card");

        let library = load_project_skill_library(temp.path()).expect("skill library");

        assert_eq!(library.skills.len(), 1);
        let skill = &library.skills[0];
        assert_eq!(skill.id, "skill:frontend");
        assert_eq!(skill.mirror_targets, vec!["codex"]);
        assert_eq!(skill.linked_memory_cards[0].id, "project:use-axios");
        assert!(
            skill
                .recommended_memory_cards
                .iter()
                .any(|card| card.id == "project:frontend-workflow")
        );
        assert!(
            !skill
                .recommended_memory_cards
                .iter()
                .any(|card| card.id == "project:use-axios")
        );
    }

    #[test]
    fn install_catalog_package_service_installs_memory_card_with_targets() {
        let temp = tempfile::tempdir().expect("tempdir");

        let package = install_catalog_package(
            temp.path(),
            "core:rust-quality-gate",
            vec!["codex".to_string()],
        )
        .expect("install catalog package");

        assert_eq!(package.id, "core:rust-quality-gate");
        let memory_cards = memory_card::load_memory_cards(temp.path()).expect("memory_cards");
        assert_eq!(memory_cards[0].id, "core:rust-quality-gate");
        let config = config::load_or_default_project_config(temp.path()).expect("config");
        assert_eq!(config.memory_cards.include[0].targets, vec!["codex"]);
    }

    #[test]
    fn sync_job_service_records_replayable_background_job_ticket() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = crate::DesktopTaskStore::default();

        let start = start_sync_project_job(store.clone(), fsutil::path_to_slash(temp.path()));

        let status = store.snapshot();
        let replay = status.replay.expect("replay payload");
        assert!(start.accepted);
        assert_eq!(start.key, "同步");
        assert_eq!(status.job_id, start.job_id);
        assert_eq!(replay.command, "sync_project");
        assert_eq!(replay.args["projectPath"], fsutil::path_to_slash(temp.path()));
    }

    #[test]
    fn evolve_job_service_records_engine_targets_and_home_in_replay() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = tempfile::tempdir().expect("home");
        let store = crate::DesktopTaskStore::default();

        let start = start_evolve_project_job(
            store.clone(),
            fsutil::path_to_slash(temp.path()),
            home.path().to_path_buf(),
            vec!["codex".to_string()],
            true,
            "local".to_string(),
        );

        let status = store.snapshot();
        let replay = status.replay.expect("replay payload");
        assert!(start.accepted);
        assert_eq!(start.key, "整理历史");
        assert_eq!(status.job_id, start.job_id);
        assert_eq!(replay.command, "evolve_project");
        assert_eq!(replay.args["home"], fsutil::path_to_slash(home.path()));
        assert_eq!(replay.args["targets"][0], "codex");
        assert_eq!(replay.args["dryRun"], true);
        assert_eq!(replay.args["engine"], "local");
    }

    #[test]
    fn scan_job_service_records_roots_depth_and_home_in_replay() {
        let root = tempfile::tempdir().expect("root");
        let home = tempfile::tempdir().expect("home");
        let store = crate::DesktopTaskStore::default();

        let start = start_scan_projects_job(
            store.clone(),
            home.path().to_path_buf(),
            vec![root.path().to_path_buf()],
            3,
        );

        let status = store.snapshot();
        let replay = status.replay.expect("replay payload");
        assert!(start.accepted);
        assert_eq!(start.key, "扫描");
        assert_eq!(status.job_id, start.job_id);
        assert_eq!(replay.command, "scan_projects");
        assert_eq!(replay.args["home"], fsutil::path_to_slash(home.path()));
        assert_eq!(replay.args["roots"][0], fsutil::path_to_slash(root.path()));
        assert_eq!(replay.args["maxDepth"], 3);
    }

    #[test]
    fn fuse_job_service_records_precise_input_in_replay() {
        let project = tempfile::tempdir().expect("project");
        let store = crate::DesktopTaskStore::default();
        let input = MemoryCardMergeInput {
            id: "draft:fused-ui".to_string(),
            title: "融合 UI 规则".to_string(),
            sources: vec!["project:ui-a".to_string(), "project:ui-b".to_string()],
            targets: vec!["codex".to_string()],
        };

        let start = start_fuse_memory_cards_to_draft_job(
            store.clone(),
            fsutil::path_to_slash(project.path()),
            input,
        );

        let status = store.snapshot();
        let replay = status.replay.expect("replay payload");
        assert!(start.accepted);
        assert_eq!(start.key, "融合 Memory Card");
        assert_eq!(status.job_id, start.job_id);
        assert_eq!(replay.command, "fuse_memory_cards_to_draft");
        assert_eq!(replay.args["input"]["id"], "draft:fused-ui");
        assert_eq!(replay.args["input"]["sources"][1], "project:ui-b");
        assert_eq!(replay.args["input"]["targets"][0], "codex");
    }

    #[test]
    fn import_project_service_initializes_project_metadata() {
        let temp = tempfile::tempdir().expect("tempdir");

        import_project(temp.path(), false).expect("import project");

        let config = config::load_or_default_project_config(temp.path()).expect("config");
        assert_eq!(config.version, 1);
    }

    #[test]
    fn quality_mutation_services_return_build_reports() {
        let temp = tempfile::tempdir().expect("tempdir");

        let preview = build_preview(temp.path()).expect("build preview");
        import_artifact_drifts(temp.path()).expect("import drifts");
        compile_project(temp.path()).expect("compile project");

        assert!(preview.render().contains("AGENTS.md"));
    }
