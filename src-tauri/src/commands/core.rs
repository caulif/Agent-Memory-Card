#![allow(unused_imports)]

pub(crate) use super::artifact::{
    build_preview, discard_artifact_drift_path, discard_artifact_drifts, import_artifact_drift_path,
    import_artifact_drifts, keep_artifact_drift_path, keep_artifact_drifts, sync_project,
};
#[cfg(test)]
pub(crate) use super::artifact::sync_project_for_test;
pub(crate) use super::assignment::{
    clear_memory_card_targets, set_agent_enabled, set_memory_card_targets,
};
pub(crate) use super::memory::{
    attach_memory_card_to_skill, delete_memory_card, fuse_memory_cards_to_draft,
    install_catalog_package, install_global_memory_card_to_project, merge_memory_cards,
    promote_memory_card_to_global, update_memory_card,
};
pub(crate) use super::project::{
    add_project, clear_project_history, enforce_tauri_kernel_policy,
    enforce_tauri_kernel_policy_for_project, get_app_state, get_project_assignment_view,
    get_project_candidate_inbox, get_project_dashboard, get_project_eval_run,
    get_project_memory_card_library, get_project_quality_view, get_project_review_inbox,
    get_project_skill_library, get_project_snapshot, import_project, plan_kernel_command,
    plan_kernel_command_input, scan_projects,
};
pub(crate) use super::provider::{
    get_custom_provider_config, get_setup_checklist, save_custom_provider_config,
    test_provider_status,
};
pub(crate) use super::review::{
    approve_draft, evolve_project, gc_candidates, hide_candidate, merge_drafts, promote_candidate,
    reject_candidate, reject_draft, review_project, update_candidate, update_draft,
};

#[cfg(test)]
mod tests {
    use agent_kernel::{config, fsutil, kernel, memory_card};

    use super::*;

    #[test]
    fn clear_memory_card_targets_command_clears_agent_assignments() {
        let temp = tempfile::tempdir().expect("tempdir");
        memory_card::add_memory_card(
            temp.path(),
            "project:use-axios",
            "Use Axios",
            "Use Axios for frontend HTTP requests.",
            "preference",
            "project",
            vec!["codex".to_string(), "claude-code".to_string()],
        )
        .expect("add memory_card");

        let ack = clear_memory_card_targets(
            fsutil::path_to_slash(temp.path()),
            Some("codex".to_string()),
            Some(kernel::KernelPolicy::agent_managed()),
            None,
        )
        .expect("clear targets");

        assert_eq!(ack.project_path, fsutil::path_to_slash(temp.path()));
        let project = agent_kernel::config::load_or_default_project_config(temp.path())
            .expect("project");
        assert_eq!(project.memory_cards.include[0].targets, vec!["claude-code"]);
        let audit = kernel::load_audit_entries(temp.path()).expect("audit");
        assert_eq!(audit[0].command, "clear-memory-card-targets");
        assert_eq!(audit[0].status, kernel::KernelAuditStatus::Authorized);
    }

    #[test]
    fn clear_project_history_removes_observation_index() {
        let temp = tempfile::tempdir().expect("tempdir");
        let kernel_dir = config::kernel_dir(temp.path());
        std::fs::create_dir_all(kernel_dir.join("observations")).expect("observations dir");
        std::fs::write(
            kernel_dir.join("observations").join("old.yml"),
            "id: obs:old\n",
        )
        .expect("observation");
        std::fs::write(
            kernel_dir.join("observation-index.yml"),
            "version: 1\nsources:\n- source_path: old.jsonl\n",
        )
        .expect("index");

        super::super::project::clear_project_history_files(temp.path()).expect("clear history");

        assert!(!kernel_dir.join("observations").exists());
        assert!(!kernel_dir.join("observation-index.yml").exists());
    }

    #[test]
    fn delete_memory_card_command_removes_assignment_and_audits() {
        let temp = tempfile::tempdir().expect("tempdir");
        memory_card::add_memory_card(
            temp.path(),
            "project:use-axios",
            "Use Axios",
            "Use Axios for frontend HTTP requests.",
            "preference",
            "project",
            vec!["codex".to_string()],
        )
        .expect("add memory_card");

        let token = kernel::create_decision_token(
            temp.path(),
            &kernel::KernelCommand::DeleteMemoryCard {
                id: "project:use-axios".to_string(),
            },
            &serde_json::json!({ "id": "project:use-axios" }),
            &kernel::KernelPolicy::manual(),
        )
        .expect("token");

        delete_memory_card(
            fsutil::path_to_slash(temp.path()),
            "project:use-axios".to_string(),
            Some(kernel::KernelPolicy::agent_managed()),
            Some(token),
        )
        .expect("delete memory_card");

        assert!(memory_card::load_memory_cards(temp.path())
            .expect("memory_cards")
            .is_empty());
        let project = config::load_or_default_project_config(temp.path()).expect("project");
        assert!(project.memory_cards.include.is_empty());
        let audit = kernel::load_audit_entries(temp.path()).expect("audit");
        assert_eq!(audit[0].command, "delete-memory-card");
        assert_eq!(audit[0].status, kernel::KernelAuditStatus::Authorized);
    }
}
