use super::*;

pub(super) fn extract_llm_text_to_drafts(
    project_root: &Path,
    input: &str,
    targets: Vec<String>,
    source: &str,
    provider_name: &str,
    dry_run: bool,
    max_candidates: Option<usize>,
) -> Result<ExtractReport> {
    let provider_cfg = provider::load_or_default_provider_config(project_root)?;
    let redacted_input = if provider_cfg.privacy.redact_secrets {
        provider::redact_secrets(input)
    } else {
        input.to_string()
    };
    let redacted = redacted_input != input;

    let paragraphs = signals::split_into_paragraphs(&redacted_input);
    let candidate_paragraphs = signals::detect_candidate_paragraphs(&paragraphs);

    if candidate_paragraphs.is_empty() {
        return Ok(ExtractReport {
            created: Vec::new(),
            skipped: vec!["No candidate paragraphs detected.".to_string()],
            candidates: Vec::new(),
            dry_run,
            provider: provider_name.to_string(),
            redacted,
        });
    }

    let max_per_batch = provider_cfg.max_candidates_per_batch;
    let min_confidence = provider_cfg.min_confidence;
    let mut all_knowledge = Vec::new();

    for batch in candidate_paragraphs.chunks(max_per_batch) {
        match llm::run_llm_extraction(project_root, batch, 2048) {
            Ok(items) => all_knowledge.extend(items),
            Err(e) => {
                return Ok(ExtractReport {
                    created: Vec::new(),
                    skipped: vec![format!("LLM extraction failed: {e}")],
                    candidates: Vec::new(),
                    dry_run,
                    provider: provider_name.to_string(),
                    redacted,
                });
            }
        }
    }

    let usable = llm::filter_usable_knowledge(all_knowledge, min_confidence);

    let skilllets = skilllet::load_skilllets(project_root)?;
    let deduper = embedding::SemanticDeduper::new(0.75, 0.65);
    let mut deduped_items: Vec<embedding::LlmKnowledgeItem> = Vec::new();
    let mut skipped = Vec::new();
    for item in usable {
        if item.is_noise {
            continue;
        }
        let memory_tier = memory_tier_from_guess(item.memory_tier_guess.as_deref(), &item.body);
        let scope = if memory_tier == MemoryTier::ProjectRule {
            "project".to_string()
        } else {
            "global".to_string()
        };
        let scope_skilllets = scoped_skilllets(&skilllets, &scope);
        let mut suggested_action =
            match deduper.dedup_against_existing(&item.body, &scope_skilllets) {
                embedding::DedupResult::Duplicate {
                    similar_id,
                    similarity,
                } => candidate::ExtractionAction::merge_into_existing(similar_id, similarity),
                embedding::DedupResult::Unique => candidate::ExtractionAction::new_candidate(),
            };
        let similar_skilllets =
            deduper.top_similar_skilllets(&item.body, &scope_skilllets, 5, 0.35);
        if !similar_skilllets.is_empty()
            && let Ok(decision) =
                llm::run_update_decision(project_root, &item.body, &similar_skilllets)
        {
            suggested_action = llm::action_from_update_decision(&decision, suggested_action);
        }
        let mut base_item = embedding::LlmKnowledgeItem {
            title: item.title.clone(),
            body: item.body.clone(),
            kind: llm::knowledge_kind_to_str(&item.kind).to_string(),
            scope: scope.clone(),
            memory_tier: memory_tier.clone(),
            abstraction_of: None,
            abstracted_from: None,
            confidence: item.confidence,
            evidence: item
                .evidence_quote
                .as_deref()
                .map(|quote| format!("{source}: {quote}"))
                .unwrap_or_else(|| format!("{source}: LLM extraction")),
            reason: item.rationale.clone(),
            matched_signal: format!("{:?}", item.kind),
            is_noise: item.is_noise,
            suggested_action,
            durability_score: item.durability_score,
            reusability_score: item.reusability_score,
            specificity_score: item.specificity_score,
            source_trust_score: item.source_trust_score,
        };
        let should_abstract = memory_tier == MemoryTier::ProjectRule
            && (item.reusability_score.unwrap_or(0.0) >= 0.6
                || has_methodology_signal(&item.body)
                || item
                    .evidence_quote
                    .as_deref()
                    .is_some_and(has_methodology_signal));
        let mut abstracted_items = Vec::new();
        if should_abstract {
            let project_candidate = Candidate {
                title: base_item.title.clone(),
                body: base_item.body.clone(),
                kind: base_item.kind.clone(),
                scope: base_item.scope.clone(),
                memory_tier: base_item.memory_tier.clone(),
                abstraction_of: None,
                abstracted_from: None,
                evidence: base_item.evidence.clone(),
                confidence: Some(base_item.confidence),
                reason: Some(base_item.reason.clone()),
                matched_template: Some(base_item.matched_signal.clone()),
            };
            if let Ok(Some(spec)) =
                r#abstract::run_candidate_abstraction(project_root, &project_candidate)
            {
                abstracted_items.push(abstracted_item_from_spec(&base_item, spec));
            }
            if abstracted_items.is_empty() && provider_cfg.fallback_methodology_templates {
                for abstracted in methodology_pair_candidates(&item.body)
                    .into_iter()
                    .filter(|candidate| candidate.scope == "global")
                {
                    abstracted_items.push(embedding::LlmKnowledgeItem {
                    title: abstracted.title.clone(),
                    body: abstracted.body.clone(),
                    kind: abstracted.kind.clone(),
                    scope: abstracted.scope.clone(),
                    memory_tier: abstracted.memory_tier.clone(),
                    abstraction_of: abstracted.abstraction_of.clone(),
                    abstracted_from: Some(candidate_id_from_scope_and_title(
                        &base_item.scope,
                        &base_item.title,
                    )),
                    confidence: item.confidence.max(0.88),
                    evidence: item
                        .evidence_quote
                        .as_deref()
                        .map(|quote| format!("{source}: {quote}"))
                        .unwrap_or_else(|| format!("{source}: LLM extraction")),
                    reason: format!(
                        "{} Abstracted into a reusable principle because the extracted item had high reusability.",
                        item.rationale
                    ),
                    matched_signal: "fallback-methodology-template".to_string(),
                    is_noise: false,
                    suggested_action: candidate::ExtractionAction::new_candidate_for_route(
                        "review_only",
                    ),
                    durability_score: Some(0.86),
                    reusability_score: Some(0.90),
                    specificity_score: Some(0.78),
                    source_trust_score: item.source_trust_score,
                });
                }
            }
            if let Some(first) = abstracted_items.first() {
                base_item.abstraction_of = Some(candidate_id_from_scope_and_title(
                    &first.scope,
                    &first.title,
                ));
            }
        }
        deduped_items.push(base_item);
        deduped_items.extend(abstracted_items);
    }

    let mut feedback_filtered_items = Vec::new();
    for mut item in deduped_items {
        let candidate_id = candidate_id_from_scope_and_title(&item.scope, &item.title);
        match feedback_gate::apply_llm_item_feedback(project_root, &candidate_id, &mut item) {
            Ok(Some(message)) => skipped.push(message),
            Ok(None) => feedback_filtered_items.push(item),
            Err(error) => {
                skipped.push(format!("feedback: {error}"));
                feedback_filtered_items.push(item);
            }
        }
    }
    let mut deduped_items = feedback_filtered_items;
    let retained = deduper.dedup_within_batch(&mut deduped_items);

    let final_items: Vec<&embedding::LlmKnowledgeItem> =
        retained.iter().map(|&i| &deduped_items[i]).collect();
    let final_limit = max_candidates.unwrap_or(usize::MAX);
    let mut scored_items: Vec<(
        &embedding::LlmKnowledgeItem,
        scoring::ExtractionScore,
        quality_gate::QualityGateDecision,
        candidate::ExtractionAction,
    )> = Vec::new();
    for item in final_items {
        let candidate = Candidate {
            title: item.title.clone(),
            body: item.body.clone(),
            kind: item.kind.clone(),
            scope: item.scope.clone(),
            memory_tier: item.memory_tier.clone(),
            abstraction_of: item.abstraction_of.clone(),
            abstracted_from: item.abstracted_from.clone(),
            evidence: item.evidence.clone(),
            confidence: Some(item.confidence),
            reason: Some(item.reason.clone()),
            matched_template: Some(item.matched_signal.clone()),
        };
        let candidate_id = candidate_id_from_scope_and_title(&item.scope, &item.title);
        if let Some(message) =
            feedback_gate::skip_body_from_feedback(project_root, &candidate_id, &candidate.body)?
        {
            skipped.push(message);
            continue;
        }
        let quality_decision = evaluate_candidate_quality(&candidate, &item.suggested_action);
        if quality_decision.disposition == QualityDisposition::Skip {
            skipped.push(quality_skip_message(
                &format!("project:{}", textutil::slug(&item.title)),
                &quality_decision,
            ));
            continue;
        }
        let memory_decision = memory_gate::evaluate_memory_candidate(
            &candidate.title,
            &candidate.body,
            &candidate.evidence,
            &candidate.kind,
            &candidate.scope,
        );
        if memory_decision.disposition == memory_gate::MemoryGateDisposition::Reject {
            skipped.push(format!(
                "{candidate_id}: memory-gate ({})",
                memory_decision.flags.join(",")
            ));
            continue;
        }
        let suggested_action =
            if memory_decision.disposition == memory_gate::MemoryGateDisposition::ReviewOnly {
                item.suggested_action
                    .clone()
                    .with_route(memory_decision.route)
            } else {
                item.suggested_action.clone()
            };
        let decision = quality_decision;
        let scope_skilllets = scoped_skilllets(&skilllets, &item.scope);
        let similar_skilllets =
            deduper.top_similar_skilllets(&item.body, &scope_skilllets, 5, 0.35);
        if let Ok(judgment) =
            llm::run_quality_judge(project_root, &item.body, &item.evidence, &similar_skilllets)
        {
            let weak_grounding = item.memory_tier != MemoryTier::ProjectRule
                && judgment.evidence_grounded.unwrap_or(1.0)
                    < provider_cfg.judge.evidence_grounded_min_non_project;
            let noisy = judgment.noise_risk.unwrap_or(0.0) > provider_cfg.judge.noise_risk_reject;
            if judgment.decision == llm::JudgeDecision::Reject || weak_grounding || noisy {
                skipped.push(format!(
                    "project:{}: llm-judge ({})",
                    textutil::slug(&item.title),
                    judgment.reason
                ));
                continue;
            }
        }
        let chunk = chunk::EvidenceChunk {
            id: item.title.clone(),
            text: item.body.clone(),
            origin: chunk::ChunkOrigin::Assistant,
            source_kind: source.to_string(),
            source_observations: Vec::new(),
        };
        let score = scoring::score_chunk(&chunk);
        if score.disposition == scoring::ExtractionDisposition::Candidate
            || item.matched_signal.starts_with("methodology-")
        {
            scored_items.push((item, score, decision, suggested_action));
        }
    }
    let selected = ranking::select_balanced_candidates(
        &scored_items,
        10.min(final_limit),
        |(item, _, _, _)| llm_item_selection_score(item),
        |(item, _, _, _)| {
            format!(
                "{}:{}",
                item.memory_tier.as_str(),
                ranking::body_domain(&item.body)
            )
        },
        |(item, _, _, _)| item.memory_tier.clone(),
    );
    let mut slots = scored_items
        .into_iter()
        .map(Some)
        .collect::<Vec<Option<_>>>();
    let scored_items = selected
        .into_iter()
        .filter_map(|index| slots.get_mut(index).and_then(Option::take))
        .collect::<Vec<_>>();
    let scored_items = if should_refine_final_memory(source) {
        let originals = scored_items
            .iter()
            .map(|(item, _, _, _)| Candidate {
                title: item.title.clone(),
                body: item.body.clone(),
                kind: item.kind.clone(),
                scope: item.scope.clone(),
                memory_tier: item.memory_tier.clone(),
                abstraction_of: item.abstraction_of.clone(),
                abstracted_from: item.abstracted_from.clone(),
                evidence: item.evidence.clone(),
                confidence: Some(item.confidence),
                reason: Some(item.reason.clone()),
                matched_template: Some(item.matched_signal.clone()),
            })
            .collect::<Vec<_>>();
        let (refined, refine_messages) = refine::refine_candidates(project_root, &originals);
        skipped.extend(refine_messages);
        scored_items
            .into_iter()
            .zip(refined)
            .filter_map(|((item, score, decision, action), refined)| {
                refined.map(|candidate| (item, candidate, score, decision, action))
            })
            .collect::<Vec<_>>()
    } else {
        scored_items
            .into_iter()
            .map(|(item, score, decision, action)| {
                let candidate = Candidate {
                    title: item.title.clone(),
                    body: item.body.clone(),
                    kind: item.kind.clone(),
                    scope: item.scope.clone(),
                    memory_tier: item.memory_tier.clone(),
                    abstraction_of: item.abstraction_of.clone(),
                    abstracted_from: item.abstracted_from.clone(),
                    evidence: item.evidence.clone(),
                    confidence: Some(item.confidence),
                    reason: Some(item.reason.clone()),
                    matched_template: Some(item.matched_signal.clone()),
                };
                (item, candidate, score, decision, action)
            })
            .collect::<Vec<_>>()
    };

    let previews = scored_items
        .iter()
        .map(|(_item, candidate, score, decision, action)| {
            let chunk = chunk::EvidenceChunk {
                id: candidate.title.clone(),
                text: candidate.body.clone(),
                origin: chunk::ChunkOrigin::Assistant,
                source_kind: source.to_string(),
                source_observations: Vec::new(),
            };
            let classification = classification_for_candidate(candidate, &chunk);
            let routed_action = route_action_for_classification(action, &classification);
            ExtractCandidatePreview {
                id: candidate_id_from_scope_and_title(&candidate.scope, &candidate.title),
                title: candidate.title.clone(),
                body: candidate.body.clone(),
                kind: candidate.kind.clone(),
                scope: candidate.scope.clone(),
                memory_tier: candidate.memory_tier.clone(),
                abstraction_of: candidate.abstraction_of.clone(),
                abstracted_from: candidate.abstracted_from.clone(),
                evidence: candidate.evidence.clone(),
                confidence: candidate.confidence,
                reason: Some(score.reason.clone()),
                matched_template: candidate.matched_template.clone(),
                classification: Some(classification.clone()),
                tags: classification.tags.clone(),
                suggested_action: Some(routed_action),
                operation: decision.operation.clone(),
                quality_flags: decision.flags.clone(),
            }
        })
        .collect::<Vec<_>>();

    if dry_run {
        return Ok(ExtractReport {
            created: Vec::new(),
            skipped,
            candidates: previews,
            dry_run,
            provider: provider_name.to_string(),
            redacted,
        });
    }

    let mut created = Vec::new();
    for (item, refined_candidate, score, _decision, action) in scored_items {
        let id =
            candidate_id_from_scope_and_title(&refined_candidate.scope, &refined_candidate.title);
        let chunk = chunk::EvidenceChunk {
            id: refined_candidate.title.clone(),
            text: refined_candidate.body.clone(),
            origin: chunk::ChunkOrigin::Assistant,
            source_kind: source.to_string(),
            source_observations: Vec::new(),
        };
        let mut extraction =
            extraction_metadata_for_chunk(&chunk, &score, item.suggested_action.record_id.clone());
        let route = extraction
            .classification
            .as_ref()
            .map(|classification| classification.artifact_kind.as_str())
            .unwrap_or("review_only");
        extraction.suggested_action = Some(route_action(action, route));
        extraction.memory_tier = item.memory_tier.clone();
        extraction.value_scores = llm_item_value_scores(item);
        extraction.abstraction_of = refined_candidate.abstraction_of.clone();
        extraction.abstracted_from = refined_candidate.abstracted_from.clone();
        let result = candidate::add_candidate(
            project_root,
            candidate::NewCandidate {
                id: id.clone(),
                title: refined_candidate.title.clone(),
                kind: refined_candidate.kind.clone(),
                scope: refined_candidate.scope.clone(),
                body: refined_candidate.body.clone(),
                brief: None,
                tags: Vec::new(),
                language: None,
                targets: targets.clone(),
                evidence: refined_candidate.evidence.clone(),
                confidence: refined_candidate.confidence,
                reason: refined_candidate.reason.clone(),
                matched_template: refined_candidate.matched_template.clone(),
                source_observations: Vec::new(),
                extraction,
            },
        );
        match result {
            Ok(()) => created.push(id),
            Err(error) => skipped.push(format!("{id}: {error}")),
        }
    }

    Ok(ExtractReport {
        created,
        skipped,
        candidates: previews,
        dry_run,
        provider: provider_name.to_string(),
        redacted,
    })
}
