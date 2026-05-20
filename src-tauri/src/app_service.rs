use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::thread;

use agent_kernel::{
    build, candidate, catalog, config, draft, eval, fsutil, kernel, memory_card, observation,
    project_registry, provider, rule_test, scanner,
};
use chrono::Utc;
use serde::Serialize;

use crate::{
    CandidateUpdateInput, DesktopJobReplay, DesktopJobStart, DesktopTaskStore, DraftMergeInput, DraftUpdateInput,
    MemoryCardMergeInput, MemoryCardUpdateInput,
};

#[derive(Debug, Clone, Serialize)]
pub struct ProjectDashboard {
    pub project_path: String,
    pub candidate_count: usize,
    pub draft_count: usize,
    pub memory_card_count: usize,
    pub observation_count: usize,
    pub global_memory_card_count: usize,
    pub enabled_agents: Vec<String>,
    pub warning_count: usize,
}

#[derive(Debug, Serialize)]
pub struct ProjectReviewInbox {
    pub project_path: String,
    pub drafts: Vec<draft::DraftRecord>,
}

#[derive(Debug, Serialize)]
pub struct ProjectCandidateInbox {
    pub project_path: String,
    pub candidates: Vec<candidate::CandidateRecord>,
}

#[derive(Debug, Serialize)]
pub struct ProjectMemoryCardLibrary {
    pub project_path: String,
    pub memory_cards: Vec<memory_card::MemoryCardRecord>,
    pub global_memory_cards: Vec<memory_card::MemoryCardRecord>,
    pub catalog_status: catalog::CatalogStatus,
}

#[derive(Debug, Serialize)]
pub struct ProjectSkillLibrary {
    pub project_path: String,
    pub generated_at: String,
    pub source_counts: BTreeMap<String, usize>,
    pub skills: Vec<ProjectSkillView>,
}

#[derive(Debug, Serialize)]
pub struct ProjectSkillView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source_path: String,
    pub source_kind: String,
    pub source_hash: String,
    pub warnings: Vec<String>,
    pub mirror_targets: Vec<String>,
    pub linked_memory_cards: Vec<memory_card::MemoryCardRecord>,
    pub recommended_memory_cards: Vec<memory_card::MemoryCardRecord>,
}

#[derive(Debug, Serialize)]
pub struct ProjectAssignmentView {
    pub project_path: String,
    pub enabled_agents: Vec<String>,
    pub target_matrix: memory_card::MemoryCardTargetMatrix,
}

#[derive(Debug, Serialize)]
pub struct ProjectQualityView {
    pub project_path: String,
    pub rule_ci: rule_test::RuleTestReport,
    pub build_preview: build::BuildReport,
    pub status: build::StatusReport,
}

#[derive(Debug, Serialize)]
pub struct ProjectEvalMetricView {
    pub label: String,
    pub percent: Option<f32>,
    pub count: usize,
    pub total: usize,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct ProjectEvalRunView {
    pub project_path: String,
    pub status: String,
    pub provider: Option<String>,
    pub pipeline_version: Option<u32>,
    pub timestamp: Option<String>,
    pub recall: Option<ProjectEvalMetricView>,
    pub precision: Option<ProjectEvalMetricView>,
    pub one_off_false_positive: Option<ProjectEvalMetricView>,
    pub duplicate_cluster_risk: Option<ProjectEvalMetricView>,
    pub evidence_validity: Option<ProjectEvalMetricView>,
    pub provider_evidence_validity: Option<ProjectEvalMetricView>,
    pub recommendations: Vec<String>,
}

pub fn load_project_dashboard(
    project_root: &Path,
    home: &Path,
) -> anyhow::Result<ProjectDashboard> {
    let root = fsutil::normalize_project_root(project_root)?;
    let config = config::load_or_default_project_config(&root)?;
    let enabled_agents = config
        .agents
        .iter()
        .filter(|(_, agent)| agent.enabled)
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();

    Ok(ProjectDashboard {
        project_path: fsutil::path_to_slash(&root),
        candidate_count: candidate::list_visible_candidates(&root)?.len(),
        draft_count: draft::load_reviewable_drafts(&root)?.len(),
        memory_card_count: memory_card::load_memory_cards(&root)?.len(),
        observation_count: observation::count_observations(&root)?,
        global_memory_card_count: memory_card::count_global_memory_cards(home)?,
        enabled_agents,
        warning_count: 0,
    })
}

pub fn load_project_review_inbox(project_root: &Path) -> anyhow::Result<ProjectReviewInbox> {
    let root = fsutil::normalize_project_root(project_root)?;
    Ok(ProjectReviewInbox {
        project_path: fsutil::path_to_slash(&root),
        drafts: draft::load_reviewable_drafts(&root)?,
    })
}

pub fn load_project_candidate_inbox(project_root: &Path) -> anyhow::Result<ProjectCandidateInbox> {
    let root = fsutil::normalize_project_root(project_root)?;
    Ok(ProjectCandidateInbox {
        project_path: fsutil::path_to_slash(&root),
        candidates: candidate::list_visible_candidates(&root)?,
    })
}

pub fn approve_candidate_to_memory_card(
    project_root: &Path,
    id: &str,
) -> anyhow::Result<memory_card::MemoryCardRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    candidate::approve_candidate_to_memory_card(&root, id)
}

pub fn hide_candidate(
    project_root: &Path,
    id: &str,
) -> anyhow::Result<candidate::CandidateRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    candidate::hide_candidate(&root, id)
}

pub fn reject_candidate(
    project_root: &Path,
    id: &str,
    reason: Option<String>,
) -> anyhow::Result<candidate::CandidateRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    candidate::reject_candidate(&root, id, reason)
}

pub fn update_candidate(
    project_root: &Path,
    id: &str,
    input: CandidateUpdateInput,
) -> anyhow::Result<candidate::CandidateRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    candidate::update_candidate(&root, id, candidate_update_from_input(input))
}

pub fn gc_candidates(project_root: &Path) -> anyhow::Result<candidate::CandidateGcReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    candidate::gc_candidates(&root)
}

pub fn approve_draft(project_root: &Path, id: &str) -> anyhow::Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    draft::approve_draft(&root, id)
}

pub fn reject_draft(project_root: &Path, id: &str) -> anyhow::Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    draft::reject_draft(&root, id)
}

pub fn update_draft(
    project_root: &Path,
    id: &str,
    input: DraftUpdateInput,
) -> anyhow::Result<draft::DraftRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    draft::update_draft(&root, id, draft_update_from_input(input))
}

pub fn merge_drafts(project_root: &Path, input: DraftMergeInput) -> anyhow::Result<draft::DraftRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    draft::merge_drafts(
        &root,
        &input.id,
        &input.title,
        input.sources,
        input.targets,
    )
}

pub fn set_agent_enabled(project_root: &Path, agent: &str, enabled: bool) -> anyhow::Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    config::set_agent_enabled(&root, agent, enabled)
}

pub fn set_memory_card_targets(
    project_root: &Path,
    id: &str,
    targets: Vec<String>,
) -> anyhow::Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    memory_card::set_memory_card_targets(&root, id, targets)
}

pub fn update_memory_card(
    project_root: &Path,
    id: &str,
    input: MemoryCardUpdateInput,
) -> anyhow::Result<memory_card::MemoryCardRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    memory_card::update_memory_card(&root, id, memory_card_update_from_input(input))
}

pub fn promote_memory_card_to_global(
    project_root: &Path,
    home: &Path,
    id: &str,
) -> anyhow::Result<memory_card::MemoryCardRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    memory_card::promote_memory_card_to_global(&root, home, id)
}

pub fn install_global_memory_card_to_project(
    project_root: &Path,
    home: &Path,
    id: &str,
    targets: Vec<String>,
) -> anyhow::Result<memory_card::MemoryCardRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    memory_card::install_global_memory_card_to_project(&root, home, id, targets)
}

pub fn merge_memory_cards(
    project_root: &Path,
    input: MemoryCardMergeInput,
) -> anyhow::Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    memory_card::merge_memory_cards(
        &root,
        &input.id,
        &input.title,
        input.sources,
        input.targets,
    )
}

pub fn attach_memory_card_to_skill(
    project_root: &Path,
    memory_card_id: &str,
    skill_id: &str,
    fusion_mode: Option<&str>,
) -> anyhow::Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    if fusion_mode == Some("auto") {
        attach_memory_card_to_skill_with_fusion(&root, memory_card_id, skill_id)
    } else {
        config::add_skill_supplement(&root, skill_id, memory_card_id)
    }
}

pub fn install_catalog_package(
    project_root: &Path,
    package_id: &str,
    targets: Vec<String>,
) -> anyhow::Result<catalog::CatalogPackage> {
    let root = fsutil::normalize_project_root(project_root)?;
    catalog::install_catalog_package(&root, package_id, targets)
}

pub fn import_project(project_root: &Path, scan_home: bool) -> anyhow::Result<scanner::ImportSummary> {
    let root = fsutil::normalize_project_root(project_root)?;
    scanner::import_project(&root, scan_home)
}

pub fn import_artifact_drifts(project_root: &Path) -> anyhow::Result<build::ArtifactImportReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    build::import_artifact_drifts(&root)
}

pub fn import_artifact_drift_path(
    project_root: &Path,
    artifact_path: &str,
) -> anyhow::Result<build::ArtifactImportReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    build::import_artifact_drift_path(&root, artifact_path)
}

pub fn keep_artifact_drifts(
    project_root: &Path,
) -> anyhow::Result<build::ArtifactDriftResolutionReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    build::keep_artifact_drifts(&root)
}

pub fn keep_artifact_drift_path(
    project_root: &Path,
    artifact_path: &str,
) -> anyhow::Result<build::ArtifactDriftResolutionReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    build::keep_artifact_drift_path(&root, artifact_path)
}

pub fn discard_artifact_drifts(
    project_root: &Path,
) -> anyhow::Result<build::ArtifactDriftResolutionReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    build::discard_artifact_drifts(&root)
}

pub fn discard_artifact_drift_path(
    project_root: &Path,
    artifact_path: &str,
) -> anyhow::Result<build::ArtifactDriftResolutionReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    build::discard_artifact_drift_path(&root, artifact_path)
}

pub fn build_preview(project_root: &Path) -> anyhow::Result<build::BuildReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    build::build_project(&root, true)
}

#[cfg(test)]
pub fn compile_project(project_root: &Path) -> anyhow::Result<build::BuildReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    build::sync_project(&root)
}

pub fn sync_project_importing_artifact_drifts(
    project_root: &Path,
) -> anyhow::Result<build::ArtifactImportReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    let import = build::import_artifact_drifts(&root)?;
    build::sync_project(&root)?;
    Ok(import)
}

fn draft_update_from_input(input: DraftUpdateInput) -> draft::DraftUpdate {
    draft::DraftUpdate {
        title: input.title,
        body: input.body,
        brief: input.brief,
        tags: input.tags,
        language: input.language,
        kind: input.kind,
        scope: input.scope,
        targets: input.targets,
    }
}

fn candidate_update_from_input(input: CandidateUpdateInput) -> candidate::CandidateUpdate {
    candidate::CandidateUpdate {
        title: input.title,
        body: input.body,
        brief: input.brief,
        tags: input.tags,
        language: input.language,
        kind: input.kind,
        scope: input.scope,
        targets: input.targets,
    }
}

fn memory_card_update_from_input(input: MemoryCardUpdateInput) -> memory_card::MemoryCardUpdate {
    memory_card::MemoryCardUpdate {
        title: input.title,
        body: input.body,
        brief: input.brief,
        tags: input.tags,
        language: input.language,
        kind: input.kind,
        scope: input.scope,
    }
}

pub fn load_project_memory_card_library(
    project_root: &Path,
    home: &Path,
) -> anyhow::Result<ProjectMemoryCardLibrary> {
    let root = fsutil::normalize_project_root(project_root)?;
    Ok(ProjectMemoryCardLibrary {
        project_path: fsutil::path_to_slash(&root),
        memory_cards: memory_card::load_memory_cards(&root)?,
        global_memory_cards: memory_card::load_global_memory_cards(home)?,
        catalog_status: catalog::catalog_status(&root)?,
    })
}

pub fn load_project_skill_library(project_root: &Path) -> anyhow::Result<ProjectSkillLibrary> {
    let root = fsutil::normalize_project_root(project_root)?;
    let project = config::load_or_default_project_config(&root)?;
    let index = config::load_skill_index(&root)?;
    let memory_cards = memory_card::load_memory_cards(&root)?;
    let mut source_counts = BTreeMap::new();
    for skill in &index.skills {
        *source_counts.entry(skill.source_kind.clone()).or_insert(0) += 1;
    }

    let skills = index
        .skills
        .into_iter()
        .map(|skill| {
            let supplement_decl = project
                .skills
                .supplements
                .iter()
                .find(|supplement| supplement.skill == skill.id);
            let linked_ids = supplement_decl
                .map(|supplement| supplement.memory_cards.iter().cloned().collect::<BTreeSet<_>>())
                .unwrap_or_default();
            let mirror_targets = project
                .skills
                .mirrors
                .iter()
                .find(|mirror| mirror.reference == skill.id)
                .map(|mirror| mirror.targets.clone())
                .unwrap_or_default();
            let linked_memory_cards = memory_cards
                .iter()
                .filter(|record| linked_ids.contains(&record.id))
                .map(|record| {
                    let mut linked = record.clone();
                    if let Some(entry) = supplement_decl.and_then(|decl| {
                        decl.entries
                            .iter()
                            .find(|entry| entry.memory_card == record.id)
                    }) {
                        linked.title = entry.title.clone();
                        linked.body = entry.body.clone();
                        linked.brief = format!("Skill supplement ({})：{}", entry.mode, entry.body);
                    }
                    linked
                })
                .collect::<Vec<_>>();
            let recommended_memory_cards = memory_cards
                .iter()
                .filter(|record| !linked_ids.contains(&record.id))
                .filter(|record| memory_card_recommends_for_skill(record))
                .cloned()
                .collect::<Vec<_>>();

            ProjectSkillView {
                id: skill.id,
                name: skill.name,
                description: skill.description,
                source_path: skill.source_path,
                source_kind: skill.source_kind,
                source_hash: skill.source_hash,
                warnings: skill.warnings,
                mirror_targets,
                linked_memory_cards,
                recommended_memory_cards,
            }
        })
        .collect();

    Ok(ProjectSkillLibrary {
        project_path: fsutil::path_to_slash(&root),
        generated_at: index.generated_at,
        source_counts,
        skills,
    })
}

fn attach_memory_card_to_skill_with_fusion(
    project_root: &Path,
    memory_card_id: &str,
    skill_id: &str,
) -> anyhow::Result<()> {
    let index = config::load_skill_index(project_root)?;
    let skill = index
        .skills
        .iter()
        .find(|skill| skill.id == skill_id)
        .ok_or_else(|| anyhow::anyhow!("skill `{skill_id}` was not found in skill index"))?;
    let memory_card = memory_card::load_memory_cards(project_root)?
        .into_iter()
        .find(|record| record.id == memory_card_id)
        .ok_or_else(|| anyhow::anyhow!("memory_card `{memory_card_id}` does not exist"))?;

    let body = fuse_memory_card_for_skill_with_provider(project_root, skill, &memory_card)
        .unwrap_or_else(|| fuse_memory_card_for_skill_deterministic(skill, &memory_card));
    let entry = config::SkillSupplementEntry {
        memory_card: memory_card_id.to_string(),
        title: format!("{} · Skill supplement", memory_card.title),
        body,
        mode: "auto-fused".to_string(),
        updated_at: Utc::now().to_rfc3339(),
    };
    config::add_skill_supplement_entry(project_root, skill_id, entry)
}

#[derive(serde::Deserialize)]
struct SkillFusionResponse {
    body: String,
}

fn fuse_memory_card_for_skill_with_provider(
    project_root: &Path,
    skill: &config::SkillRecord,
    memory_card: &memory_card::MemoryCardRecord,
) -> Option<String> {
    let cfg = provider::load_or_default_provider_config(project_root).ok()?;
    let request = provider::ProviderRequest {
        system_prompt: r#"你是 Agent Skill supplement 编辑器。

目标：把一条 Memory Card 转写成当前 Skill 的补充说明，而不是简单拼接。

规则：
- 输出必须能直接写入 AGENT_KERNEL_MEMORY_CARDS.md。
- 用 When / Do / Boundary 或中文等价结构组织。
- 保留 Memory Card 的真实意图，但只写与该 Skill 使用场景相关的部分。
- 不要编造工具、路径、API 或用户没有确认的规则。
- 如果原 Memory Card 与 Skill 无关，写成“仅在相关任务中参考”的弱补充边界。
- 只返回 JSON。"#
            .to_string(),
        user_prompt: serde_json::json!({
            "skill": {
                "id": skill.id,
                "name": skill.name,
                "description": skill.description,
                "source_kind": skill.source_kind,
            },
            "memory_card": {
                "id": memory_card.id,
                "title": memory_card.title,
                "kind": memory_card.kind,
                "scope": memory_card.scope,
                "body": memory_card.body,
                "brief": memory_card.brief,
                "tags": memory_card.tags,
            }
        })
        .to_string(),
        json_schema: Some(provider::ProviderJsonSchema {
            name: "SkillSupplementFusion".to_string(),
            strict: true,
            schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "body": { "type": "string" }
                },
                "required": ["body"]
            }),
        }),
    };
    let output = provider::call_provider_for_role(
        &cfg,
        provider::ProviderRole::Refine,
        &request,
        2048,
    )
    .ok()?;
    let parsed: SkillFusionResponse = serde_json::from_str(output.trim()).ok()?;
    let body = parsed.body.trim().to_string();
    (body.chars().count() >= 24 && body.chars().count() <= 1200).then_some(body)
}

fn fuse_memory_card_for_skill_deterministic(
    skill: &config::SkillRecord,
    memory_card: &memory_card::MemoryCardRecord,
) -> String {
    if memory_card.language == "zh" {
        format!(
            "When / 触发：使用 `{}` 处理与 `{}` 相关的任务时。\n\nDo / 动作：参考 Memory Card「{}」：{}\n\nBoundary / 边界：仅在该规则与当前 Skill 的职责相符时应用；若证据不足，先保留人工审阅边界。",
            skill.name, memory_card.kind, memory_card.title, memory_card.body
        )
    } else {
        format!(
            "When: Use `{}` for work related to `{}`.\n\nDo: Apply Memory Card \"{}\": {}\n\nBoundary: Apply only when it fits this Skill's responsibility; keep human review when evidence is weak.",
            skill.name, memory_card.kind, memory_card.title, memory_card.body
        )
    }
}

fn memory_card_recommends_for_skill(record: &memory_card::MemoryCardRecord) -> bool {
    matches!(record.activation.as_str(), "skill")
        || matches!(
            record.kind.as_str(),
            "procedure" | "workflow" | "template" | "supplement"
        )
}

pub fn load_project_assignment_view(project_root: &Path) -> anyhow::Result<ProjectAssignmentView> {
    let root = fsutil::normalize_project_root(project_root)?;
    let config = config::load_or_default_project_config(&root)?;
    let enabled_agents = config
        .agents
        .iter()
        .filter(|(_, agent)| agent.enabled)
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();

    Ok(ProjectAssignmentView {
        project_path: fsutil::path_to_slash(&root),
        enabled_agents,
        target_matrix: memory_card::memory_card_target_matrix(&root)?,
    })
}

pub fn load_project_quality_view(project_root: &Path) -> anyhow::Result<ProjectQualityView> {
    let root = fsutil::normalize_project_root(project_root)?;
    Ok(ProjectQualityView {
        project_path: fsutil::path_to_slash(&root),
        rule_ci: rule_test::run_rule_tests(&root)?,
        build_preview: build::build_project(&root, true)?,
        status: build::status_project(&root)?,
    })
}

pub fn load_project_eval_run_view(project_root: &Path) -> anyhow::Result<ProjectEvalRunView> {
    let root = fsutil::normalize_project_root(project_root)?;
    let Some(run) = eval::load_latest_golden_set_eval_run(&root)? else {
        return Ok(ProjectEvalRunView {
            project_path: fsutil::path_to_slash(&root),
            status: "missing".to_string(),
            provider: None,
            pipeline_version: None,
            timestamp: None,
            recall: None,
            precision: None,
            one_off_false_positive: None,
            duplicate_cluster_risk: None,
            evidence_validity: None,
            provider_evidence_validity: None,
            recommendations: vec![
                "Run `cargo run --quiet -- eval --golden-set --project . --json` to create the first eval baseline.".to_string(),
            ],
        });
    };
    let report = run.report;
    let provider_evidence_validity = match (
        report.provider_evidence_valid_count,
        report.provider_evidence_valid_percent,
    ) {
        (Some(count), Some(percent)) => Some(metric(
            "Provider evidence validity",
            Some(percent),
            count,
            report.positive_total,
            percent >= 80.0,
        )),
        _ => None,
    };
    let recall = metric(
        "Recall",
        Some(report.positive_recall_percent),
        report.positive_hits,
        report.positive_total,
        report.positive_recall_percent >= 85.0,
    );
    let precision = metric(
        "Precision",
        Some(report.negative_precision_percent),
        report.negative_rejected,
        report.negative_total,
        report.negative_precision_percent >= 90.0,
    );
    let one_off_false_positive = metric(
        "One-off false positives",
        Some(report.one_off_false_positive_percent),
        report.one_off_false_positive_count,
        report.negative_total,
        report.one_off_false_positive_count == 0,
    );
    let duplicate_cluster_risk = metric(
        "Duplicate risk",
        Some(report.duplicate_cluster_risk_percent),
        report.duplicate_cluster_risk_count,
        report.positive_total,
        report.duplicate_cluster_risk_count == 0,
    );
    let evidence_validity = metric(
        "Evidence validity",
        Some(report.evidence_valid_percent),
        report.evidence_valid_count,
        report.positive_total,
        report.evidence_valid_percent >= 95.0,
    );
    let recommendations = eval_recommendations(
        &recall,
        &precision,
        &one_off_false_positive,
        &duplicate_cluster_risk,
        &evidence_validity,
        provider_evidence_validity.as_ref(),
    );
    let status = if recommendations.is_empty() {
        "passing"
    } else {
        "attention"
    };
    Ok(ProjectEvalRunView {
        project_path: fsutil::path_to_slash(&root),
        status: status.to_string(),
        provider: Some(run.provider),
        pipeline_version: Some(run.pipeline_version),
        timestamp: Some(run.timestamp),
        recall: Some(recall),
        precision: Some(precision),
        one_off_false_positive: Some(one_off_false_positive),
        duplicate_cluster_risk: Some(duplicate_cluster_risk),
        evidence_validity: Some(evidence_validity),
        provider_evidence_validity,
        recommendations,
    })
}

fn metric(
    label: &str,
    percent: Option<f32>,
    count: usize,
    total: usize,
    passed: bool,
) -> ProjectEvalMetricView {
    ProjectEvalMetricView {
        label: label.to_string(),
        percent,
        count,
        total,
        status: if passed { "pass" } else { "fail" }.to_string(),
    }
}

fn eval_recommendations(
    recall: &ProjectEvalMetricView,
    precision: &ProjectEvalMetricView,
    one_off_false_positive: &ProjectEvalMetricView,
    duplicate_cluster_risk: &ProjectEvalMetricView,
    evidence_validity: &ProjectEvalMetricView,
    provider_evidence_validity: Option<&ProjectEvalMetricView>,
) -> Vec<String> {
    let mut recommendations = Vec::new();
    if recall.status == "fail" {
        recommendations.push("Review missed positive golden cases before trusting new extraction changes.".to_string());
    }
    if precision.status == "fail" {
        recommendations.push("Tighten rejection rules for negative golden cases to reduce noisy suggestions.".to_string());
    }
    if one_off_false_positive.status == "fail" {
        recommendations.push("Inspect one-off leaks and raise recurrence requirements for temporary preferences.".to_string());
    }
    if duplicate_cluster_risk.status == "fail" {
        recommendations.push("Tune clustering/deduplication before approving duplicate-looking Memory Cards.".to_string());
    }
    if evidence_validity.status == "fail" {
        recommendations.push("Fix final evidence quote grounding before showing suggestions as review-ready.".to_string());
    }
    if provider_evidence_validity.is_some_and(|metric| metric.status == "fail") {
        recommendations.push("Rerun with provider evidence checks and inspect hallucinated or weak provider quotes.".to_string());
    }
    recommendations
}

pub fn start_evolve_project_job(
    task_store: DesktopTaskStore,
    project_path: String,
    home: PathBuf,
    targets: Vec<String>,
    dry_run: bool,
    engine: String,
) -> DesktopJobStart {
    let job_id = task_store.begin_job(
        "整理历史",
        "发现会话文件",
        "正在查找 Claude Code / Codex 的 JSONL 会话与历史文件",
        12,
    );
    task_store.attach_replay(
        &job_id,
        DesktopJobReplay {
            command: "evolve_project".to_string(),
            args: serde_json::json!({
                "projectPath": project_path,
                "home": fsutil::path_to_slash(&home),
                "targets": targets,
                "dryRun": dry_run,
                "engine": engine,
                "confirmedPolicy": kernel::KernelPolicy::agent_managed()
            }),
        },
    );
    let start = crate::job_start_from_status(&task_store.snapshot(), "已加入后台整理队列");
    let background_store = task_store.clone();
    let background_engine = engine.clone();
    let background_job_id = job_id.clone();
    let background_project_path = project_path.clone();
    let background_targets = targets.clone();
    thread::spawn(move || {
        background_store.job_step(
            &background_job_id,
            "导入增量观察",
            "正在按 observation-index 跳过已处理内容，只读取新增对话片段",
            34,
        );
        if background_store.cancel_requested(&background_job_id) {
            background_store.finish_job(&background_job_id, "整理已取消，未继续读取历史会话");
            return;
        }
        background_store.job_step(
            &background_job_id,
            "整理提炼材料",
            "正在脱敏、过滤噪声，并拼接可用于 Memory Card 合成的高价值材料；低价值片段不会进入推理阶段",
            55,
        );
        if background_store.cancel_requested(&background_job_id) {
            background_store.finish_job(&background_job_id, "整理已取消，未进入推理阶段");
            return;
        }
        let engine_description = if background_engine == "local" {
            "本地极速模式：只运行规则过滤与候选生成，不启动 Claude Code / Codex"
        } else {
            "深度模式：先完成本地高价值过滤，再只把少量候选交给外部 Agent 精炼"
        };
        background_store.job_step(&background_job_id, "运行整理引擎", engine_description, 72);
        let result = observation::evolve_local_conversations_with_engine(
            Path::new(&background_project_path),
            &home,
            background_targets,
            dry_run,
            &background_engine,
        );
        match result {
            Ok(report) => {
                background_store.job_step(
                    &background_job_id,
                    "写入候选建议",
                    &format!(
                        "已导入 {} 条增量观察，写入 {} 条候选建议",
                        report.imported, report.draft_candidates
                    ),
                    92,
                );
                background_store.finish_job(
                    &background_job_id,
                    &format!(
                        "历史对话整理完成：导入 {} 条观察，生成 {} 条候选",
                        report.imported, report.draft_candidates
                    ),
                );
            }
            Err(error) => {
                background_store.finish_job(&background_job_id, &format!("整理失败：{error}"));
            }
        }
    });
    start
}

pub fn start_scan_projects_job(
    task_store: DesktopTaskStore,
    home: PathBuf,
    roots: Vec<PathBuf>,
    max_depth: usize,
) -> DesktopJobStart {
    let job_id = task_store.begin_job("扫描", "扫描本地项目", "正在准备扫描根目录", 10);
    task_store.attach_replay(
        &job_id,
        DesktopJobReplay {
            command: "scan_projects".to_string(),
            args: serde_json::json!({
                "home": fsutil::path_to_slash(&home),
                "roots": roots
                    .iter()
                    .map(|root| fsutil::path_to_slash(root.as_path()))
                    .collect::<Vec<_>>(),
                "maxDepth": max_depth,
                "confirmedPolicy": kernel::KernelPolicy::agent_managed()
            }),
        },
    );
    let start = crate::job_start_from_status(&task_store.snapshot(), "已加入后台扫描队列");
    thread::spawn(move || {
        task_store.job_step(
            &job_id,
            "识别 Agent 项目",
            "正在查找 AGENTS.md、CLAUDE.md、.claude/skills 和 .agents/skills",
            35,
        );
        if task_store.cancel_requested(&job_id) {
            task_store.finish_job(&job_id, "扫描已取消，未继续读取本地项目");
            return;
        }
        task_store.job_step(
            &job_id,
            "读取本地目录",
            &format!(
                "正在扫描 {} 个根目录，最大深度 {}；完成后会自动刷新项目列表",
                roots.len(),
                max_depth
            ),
            58,
        );
        let result = project_registry::scan_and_register(&home, &roots, max_depth);
        match result {
            Ok(report) => {
                task_store.job_step(
                    &job_id,
                    "更新项目索引",
                    &format!("已发现 {} 个项目，正在写入本地 registry", report.discovered.len()),
                    88,
                );
                task_store.finish_job(
                    &job_id,
                    &format!("本地项目扫描完成，发现 {} 个项目", report.discovered.len()),
                );
            }
            Err(error) => {
                task_store.finish_job(&job_id, &format!("扫描失败：{error}"));
            }
        }
    });
    start
}

pub fn start_sync_project_job(
    task_store: DesktopTaskStore,
    project_path: String,
) -> DesktopJobStart {
    let job_id = task_store.begin_job(
        "同步",
        "编译 Agent 产物",
        "正在将已批准 Memory Card 编译到 Claude Code / Codex",
        18,
    );
    task_store.attach_replay(
        &job_id,
        DesktopJobReplay {
            command: "sync_project".to_string(),
            args: serde_json::json!({
                "projectPath": project_path,
                "confirmedPolicy": kernel::KernelPolicy::agent_managed()
            }),
        },
    );
    let start = crate::job_start_from_status(&task_store.snapshot(), "已加入后台同步队列");
    let background_store = task_store.clone();
    let background_project_path = project_path.clone();
    thread::spawn(move || {
        background_store.job_step(
            &job_id,
            "生成编译预览",
            "正在检查 CLAUDE.md / AGENTS.md 漂移和目标智能体配置",
            42,
        );
        if background_store.cancel_requested(&job_id) {
            background_store.finish_job(&job_id, "同步已取消，未写入 Agent 产物");
            return;
        }
        background_store.job_step(
            &job_id,
            "导入漂移草稿",
            "正在把 CLAUDE.md / AGENTS.md 手工改动导入 Draft，避免覆盖",
            58,
        );
        if background_store.cancel_requested(&job_id) {
            background_store.finish_job(&job_id, "同步已取消，未导入漂移或写入 Agent 产物");
            return;
        }
        background_store.job_step(
            &job_id,
            "写入 Agent 产物",
            "正在全量编译并写入 Claude Code / Codex 目标文件",
            76,
        );
        match sync_project_importing_artifact_drifts(Path::new(&background_project_path)) {
            Ok(report) => {
                let checkpoint = build::load_last_sync_checkpoint(Path::new(&background_project_path))
                    .ok()
                    .flatten();
                let rule_ci = rule_test::run_rule_tests(Path::new(&background_project_path)).ok();
                let verification_note = rule_ci
                    .map(|report| format!("Rule CI {} passed / {} failed", report.passed, report.failed))
                    .unwrap_or_else(|| "Rule CI 状态待刷新".to_string());
                if report.created > 0 {
                    background_store.finish_job(
                        &job_id,
                        &format!(
                            "已导入 {} 条漂移草稿并同步生成产物；{}；checkpoint {}",
                            report.created,
                            verification_note,
                            checkpoint
                                .map(|item| item.id)
                                .unwrap_or_else(|| "未记录".to_string())
                        ),
                    );
                } else {
                    background_store.finish_job(
                        &job_id,
                        &format!(
                            "已同步生成产物；{}；checkpoint {}",
                            verification_note,
                            checkpoint
                                .map(|item| item.id)
                                .unwrap_or_else(|| "未记录".to_string())
                        ),
                    );
                }
            }
            Err(error) => background_store.finish_job(&job_id, &format!("同步失败：{error}")),
        }
    });
    start
}

pub fn start_fuse_memory_cards_to_draft_job(
    task_store: DesktopTaskStore,
    project_path: String,
    input: MemoryCardMergeInput,
) -> DesktopJobStart {
    let job_id = task_store.begin_job(
        "融合 Memory Card",
        "创建融合草稿",
        "正在把多个 Memory Card 合成为一条可审阅草稿",
        15,
    );
    task_store.attach_replay(
        &job_id,
        DesktopJobReplay {
            command: "fuse_memory_cards_to_draft".to_string(),
            args: serde_json::json!({
                "projectPath": project_path,
                "input": input,
                "confirmedPolicy": kernel::KernelPolicy::agent_managed()
            }),
        },
    );
    let start = crate::job_start_from_status(&task_store.snapshot(), "已加入后台融合队列");
    let background_store = task_store.clone();
    let background_project_path = project_path.clone();
    let background_input = input.clone();
    thread::spawn(move || {
        background_store.job_step(
            &job_id,
            "读取源 Memory Card",
            &format!(
                "正在读取 {} 个源 Memory Card 并准备融合上下文",
                background_input.sources.len()
            ),
            38,
        );
        if background_store.cancel_requested(&job_id) {
            background_store.finish_job(&job_id, "融合已取消，未生成草稿");
            return;
        }
        background_store.job_step(
            &job_id,
            "写入融合草稿",
            "正在生成可编辑、可拒绝、可批准的 Draft Memory Card",
            76,
        );
        let result = draft::fuse_memory_cards_to_draft(
            Path::new(&background_project_path),
            &background_input.id,
            &background_input.title,
            background_input.sources,
            background_input.targets,
        );
        match result {
            Ok(_) => background_store.finish_job(&job_id, "Memory Card 融合草稿已生成"),
            Err(error) => background_store.finish_job(&job_id, &format!("融合失败：{error}")),
        }
    });
    start
}

#[cfg(test)]
#[path = "app_service_tests.rs"]
mod tests;
