use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AutomationMode {
    Manual,
    Assisted,
    GuardedAuto,
    AgentManaged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelPolicy {
    pub mode: AutomationMode,
    pub auto_approve_min_confidence: f32,
    pub allow_file_writes: bool,
    pub require_review_for_high_risk: bool,
}

impl KernelPolicy {
    pub fn manual() -> Self {
        Self {
            mode: AutomationMode::Manual,
            auto_approve_min_confidence: 1.0,
            allow_file_writes: false,
            require_review_for_high_risk: true,
        }
    }

    pub fn assisted() -> Self {
        Self {
            mode: AutomationMode::Assisted,
            auto_approve_min_confidence: 0.9,
            allow_file_writes: false,
            require_review_for_high_risk: true,
        }
    }

    pub fn guarded_auto() -> Self {
        Self {
            mode: AutomationMode::GuardedAuto,
            auto_approve_min_confidence: 0.86,
            allow_file_writes: true,
            require_review_for_high_risk: true,
        }
    }

    pub fn agent_managed() -> Self {
        Self {
            mode: AutomationMode::AgentManaged,
            auto_approve_min_confidence: 0.72,
            allow_file_writes: true,
            require_review_for_high_risk: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum KernelCommand {
    Observe {
        source_kind: String,
        agent: Option<String>,
    },
    ScanProjects {
        max_depth: usize,
    },
    AddProject {
        path: String,
    },
    ImportProject {
        scan_home: bool,
    },
    UpdateDraft {
        id: String,
    },
    UpdateSkilllet {
        id: String,
    },
    ApproveDraft {
        id: String,
    },
    RejectDraft {
        id: String,
    },
    PromoteCandidate {
        id: String,
    },
    HideCandidate {
        id: String,
    },
    RejectCandidate {
        id: String,
    },
    GcCandidates,
    MergeDrafts {
        ids: Vec<String>,
    },
    MergeSkilllets {
        ids: Vec<String>,
    },
    FuseSkilllets {
        ids: Vec<String>,
        engine: String,
    },
    AssignSkilllet {
        id: String,
        targets: Vec<String>,
    },
    SetAgentEnabled {
        agent: String,
        enabled: bool,
    },
    PromoteSkillletToGlobal {
        id: String,
    },
    InstallGlobalSkilllet {
        id: String,
        targets: Vec<String>,
    },
    InstallCatalogPackage {
        id: String,
        targets: Vec<String>,
    },
    AttachSkillletToSkill {
        skilllet_id: String,
        skill_id: String,
    },
    EvolveProject {
        dry_run: bool,
        engine: String,
    },
    ImportArtifactDrifts,
    CompileProject {
        dry_run: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KernelRisk {
    ReadOnly,
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KernelDisposition {
    Execute,
    ReviewRequired,
    DraftOnly,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KernelDecision {
    pub command: String,
    pub disposition: KernelDisposition,
    pub risk: KernelRisk,
    pub requires_human_review: bool,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KernelPolicyError {
    pub command: String,
    pub risk: KernelRisk,
    pub disposition: KernelDisposition,
    pub reason: String,
}

impl KernelPolicyError {
    fn from_decision(decision: KernelDecision) -> Self {
        Self {
            command: decision.command,
            risk: decision.risk,
            disposition: decision.disposition,
            reason: decision.reason,
        }
    }
}

impl fmt::Display for KernelPolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "kernel policy blocked command '{}' with risk {:?}, disposition {:?}: {}",
            self.command, self.risk, self.disposition, self.reason
        )
    }
}

impl std::error::Error for KernelPolicyError {}

pub fn enforce_command(
    command: &KernelCommand,
    policy: &KernelPolicy,
) -> Result<KernelDecision, KernelPolicyError> {
    let decision = plan_command(command, policy);

    match decision.disposition {
        KernelDisposition::Execute | KernelDisposition::DraftOnly => Ok(decision),
        KernelDisposition::ReviewRequired | KernelDisposition::Reject => {
            Err(KernelPolicyError::from_decision(decision))
        }
    }
}

pub fn plan_command(command: &KernelCommand, policy: &KernelPolicy) -> KernelDecision {
    let risk = command_risk(command);
    let command_label = command_label(command);
    let writes_files = command_writes_files(command);
    let (disposition, requires_human_review, reason) = match policy.mode {
        AutomationMode::Manual => {
            if risk == KernelRisk::ReadOnly {
                (
                    KernelDisposition::Execute,
                    false,
                    "Manual mode allows read-only kernel planning.".to_string(),
                )
            } else {
                (
                    KernelDisposition::ReviewRequired,
                    true,
                    "Manual mode keeps mutating kernel commands in human review.".to_string(),
                )
            }
        }
        AutomationMode::Assisted => {
            if risk == KernelRisk::ReadOnly {
                (
                    KernelDisposition::Execute,
                    false,
                    "Assisted mode allows read-only commands and asks for review before changes."
                        .to_string(),
                )
            } else {
                (
                    KernelDisposition::ReviewRequired,
                    true,
                    "Assisted mode lets AI prepare changes, but user approval is required."
                        .to_string(),
                )
            }
        }
        AutomationMode::GuardedAuto => {
            if risk == KernelRisk::High && policy.require_review_for_high_risk {
                (
                    KernelDisposition::ReviewRequired,
                    true,
                    "Guarded Auto routes high-risk writes through review.".to_string(),
                )
            } else if writes_files && !policy.allow_file_writes {
                (
                    KernelDisposition::ReviewRequired,
                    true,
                    "Kernel policy does not allow file writes without review.".to_string(),
                )
            } else {
                (
                    KernelDisposition::Execute,
                    false,
                    "Guarded Auto allows deterministic low/medium-risk kernel commands."
                        .to_string(),
                )
            }
        }
        AutomationMode::AgentManaged => {
            if writes_files && !policy.allow_file_writes {
                (
                    KernelDisposition::ReviewRequired,
                    true,
                    "Agent Managed policy is active, but file writes are disabled.".to_string(),
                )
            } else {
                (
                    KernelDisposition::Execute,
                    false,
                    "Agent Managed mode allows execution after kernel rule assessment and audit."
                        .to_string(),
                )
            }
        }
    };

    KernelDecision {
        command: command_label,
        disposition,
        risk,
        requires_human_review,
        reason,
        decision_token: None,
    }
}

pub fn command_risk(command: &KernelCommand) -> KernelRisk {
    match command {
        KernelCommand::Observe { .. } => KernelRisk::Low,
        KernelCommand::ScanProjects { .. } => KernelRisk::Low,
        KernelCommand::AddProject { .. } => KernelRisk::Low,
        KernelCommand::ImportProject { .. } => KernelRisk::Medium,
        KernelCommand::UpdateDraft { .. } => KernelRisk::Medium,
        KernelCommand::UpdateSkilllet { .. } => KernelRisk::Medium,
        KernelCommand::RejectDraft { .. } => KernelRisk::Medium,
        KernelCommand::PromoteCandidate { .. } => KernelRisk::Medium,
        KernelCommand::HideCandidate { .. } => KernelRisk::Low,
        KernelCommand::RejectCandidate { .. } => KernelRisk::Low,
        KernelCommand::GcCandidates => KernelRisk::Low,
        KernelCommand::MergeDrafts { .. } => KernelRisk::Medium,
        KernelCommand::AssignSkilllet { .. } => KernelRisk::Low,
        KernelCommand::SetAgentEnabled { .. } => KernelRisk::Low,
        KernelCommand::InstallGlobalSkilllet { .. } => KernelRisk::Medium,
        KernelCommand::InstallCatalogPackage { .. } => KernelRisk::Medium,
        KernelCommand::AttachSkillletToSkill { .. } => KernelRisk::Medium,
        KernelCommand::PromoteSkillletToGlobal { .. } => KernelRisk::Medium,
        KernelCommand::MergeSkilllets { .. } => KernelRisk::Medium,
        KernelCommand::FuseSkilllets { .. } => KernelRisk::Medium,
        KernelCommand::ApproveDraft { .. } => KernelRisk::High,
        KernelCommand::EvolveProject { dry_run, .. } => {
            if *dry_run {
                KernelRisk::ReadOnly
            } else {
                KernelRisk::Medium
            }
        }
        KernelCommand::ImportArtifactDrifts => KernelRisk::Medium,
        KernelCommand::CompileProject { dry_run } => {
            if *dry_run {
                KernelRisk::ReadOnly
            } else {
                KernelRisk::High
            }
        }
    }
}

fn command_writes_files(command: &KernelCommand) -> bool {
    !matches!(
        command,
        KernelCommand::CompileProject { dry_run: true }
            | KernelCommand::EvolveProject { dry_run: true, .. }
    )
}

fn command_label(command: &KernelCommand) -> String {
    match command {
        KernelCommand::Observe { .. } => "observe",
        KernelCommand::ScanProjects { .. } => "scan-projects",
        KernelCommand::AddProject { .. } => "add-project",
        KernelCommand::ImportProject { .. } => "import-project",
        KernelCommand::UpdateDraft { .. } => "update-draft",
        KernelCommand::UpdateSkilllet { .. } => "update-skilllet",
        KernelCommand::ApproveDraft { .. } => "approve-draft",
        KernelCommand::RejectDraft { .. } => "reject-draft",
        KernelCommand::PromoteCandidate { .. } => "promote-candidate",
        KernelCommand::HideCandidate { .. } => "hide-candidate",
        KernelCommand::RejectCandidate { .. } => "reject-candidate",
        KernelCommand::GcCandidates => "gc-candidates",
        KernelCommand::MergeDrafts { .. } => "merge-drafts",
        KernelCommand::MergeSkilllets { .. } => "merge-skilllets",
        KernelCommand::FuseSkilllets { .. } => "fuse-skilllets",
        KernelCommand::AssignSkilllet { .. } => "assign-skilllet",
        KernelCommand::SetAgentEnabled { .. } => "set-agent-enabled",
        KernelCommand::PromoteSkillletToGlobal { .. } => "promote-skilllet-to-global",
        KernelCommand::InstallGlobalSkilllet { .. } => "install-global-skilllet",
        KernelCommand::InstallCatalogPackage { .. } => "install-catalog-package",
        KernelCommand::AttachSkillletToSkill { .. } => "attach-skilllet-to-skill",
        KernelCommand::EvolveProject { .. } => "evolve-project",
        KernelCommand::ImportArtifactDrifts => "import-artifact-drifts",
        KernelCommand::CompileProject { .. } => "compile-project",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_policy_routes_mutating_commands_to_review() {
        let policy = KernelPolicy::manual();
        let command = KernelCommand::ApproveDraft {
            id: "project:prefer-bun".to_string(),
        };

        let decision = plan_command(&command, &policy);

        assert_eq!(decision.disposition, KernelDisposition::ReviewRequired);
        assert!(decision.requires_human_review);
        assert_eq!(decision.risk, KernelRisk::High);
    }

    #[test]
    fn manual_policy_rejects_approve_draft_execution() {
        let policy = KernelPolicy::manual();
        let command = KernelCommand::ApproveDraft {
            id: "project:prefer-bun".to_string(),
        };

        let error = enforce_command(&command, &policy).expect_err("approve draft needs review");

        assert_eq!(error.command, "approve-draft");
        assert_eq!(error.disposition, KernelDisposition::ReviewRequired);
        assert_eq!(error.risk, KernelRisk::High);
        assert!(error.reason.contains("Manual mode"));
        let display = error.to_string();
        assert!(display.contains("approve-draft"));
        assert!(display.contains("High"));
        assert!(display.contains("ReviewRequired"));
        assert!(display.contains("Manual mode"));
    }

    #[test]
    fn guarded_auto_allows_low_risk_assignment_without_review() {
        let policy = KernelPolicy::guarded_auto();
        let command = KernelCommand::AssignSkilllet {
            id: "project:prefer-bun".to_string(),
            targets: vec!["codex".to_string()],
        };

        let decision = plan_command(&command, &policy);

        assert_eq!(decision.disposition, KernelDisposition::Execute);
        assert!(!decision.requires_human_review);
        assert_eq!(decision.risk, KernelRisk::Low);
    }

    #[test]
    fn guarded_auto_executes_assign_skilllet() {
        let policy = KernelPolicy::guarded_auto();
        let command = KernelCommand::AssignSkilllet {
            id: "project:prefer-bun".to_string(),
            targets: vec!["codex".to_string()],
        };

        let decision = enforce_command(&command, &policy).expect("assignment should execute");

        assert_eq!(decision.command, "assign-skilllet");
        assert_eq!(decision.disposition, KernelDisposition::Execute);
        assert_eq!(decision.risk, KernelRisk::Low);
    }

    #[test]
    fn guarded_auto_rejects_non_dry_run_compile_execution() {
        let policy = KernelPolicy::guarded_auto();
        let command = KernelCommand::CompileProject { dry_run: false };

        let error = enforce_command(&command, &policy).expect_err("compile writes need review");

        assert_eq!(error.command, "compile-project");
        assert_eq!(error.disposition, KernelDisposition::ReviewRequired);
        assert_eq!(error.risk, KernelRisk::High);
        assert!(error.reason.contains("high-risk"));
    }

    #[test]
    fn update_skilllet_is_medium_risk_mutation() {
        let policy = KernelPolicy::manual();
        let command = KernelCommand::UpdateSkilllet {
            id: "project:editable".to_string(),
        };

        let decision = plan_command(&command, &policy);

        assert_eq!(decision.command, "update-skilllet");
        assert_eq!(decision.risk, KernelRisk::Medium);
        assert_eq!(decision.disposition, KernelDisposition::ReviewRequired);
    }

    #[test]
    fn agent_managed_still_explains_high_risk_writes() {
        let policy = KernelPolicy::agent_managed();
        let command = KernelCommand::CompileProject { dry_run: false };

        let decision = plan_command(&command, &policy);

        assert_eq!(decision.disposition, KernelDisposition::Execute);
        assert_eq!(decision.risk, KernelRisk::High);
        assert!(decision.reason.contains("Agent Managed"));
    }
}
