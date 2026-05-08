use std::collections::{BTreeMap, BTreeSet};

use crate::candidate::MemoryTier;

use super::Candidate;

pub(super) fn select_diverse_candidates<T>(
    items: &[T],
    limit: usize,
    confidence: impl Fn(&T) -> f32,
    cluster_key: impl Fn(&T) -> String,
) -> Vec<usize> {
    if items.len() <= limit {
        let mut indices = (0..items.len()).collect::<Vec<_>>();
        indices.sort_by(|left, right| {
            confidence(&items[*right])
                .total_cmp(&confidence(&items[*left]))
                .then_with(|| left.cmp(right))
        });
        return indices;
    }

    let mut clusters: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, item) in items.iter().enumerate() {
        clusters.entry(cluster_key(item)).or_default().push(index);
    }

    for indices in clusters.values_mut() {
        indices.sort_by(|left, right| {
            confidence(&items[*right])
                .total_cmp(&confidence(&items[*left]))
                .then_with(|| left.cmp(right))
        });
    }

    let mut selected = Vec::new();
    let mut seen = BTreeSet::new();
    for indices in clusters.values() {
        if selected.len() >= limit {
            break;
        }
        if let Some(index) = indices.first()
            && seen.insert(*index)
        {
            selected.push(*index);
        }
    }

    let mut remaining = (0..items.len())
        .filter(|index| !seen.contains(index))
        .collect::<Vec<_>>();
    remaining.sort_by(|left, right| {
        confidence(&items[*right])
            .total_cmp(&confidence(&items[*left]))
            .then_with(|| left.cmp(right))
    });
    for index in remaining {
        if selected.len() >= limit {
            break;
        }
        selected.push(index);
    }

    selected.sort_by(|left, right| {
        confidence(&items[*right])
            .total_cmp(&confidence(&items[*left]))
            .then_with(|| left.cmp(right))
    });
    selected
}

pub(super) fn select_balanced_candidates<T>(
    items: &[T],
    limit: usize,
    confidence: impl Fn(&T) -> f32,
    cluster_key: impl Fn(&T) -> String,
    memory_tier: impl Fn(&T) -> MemoryTier,
) -> Vec<usize> {
    let mut selected = select_diverse_candidates(items, limit, &confidence, &cluster_key);
    if selected.is_empty() {
        return selected;
    }

    let selected_set = selected.iter().copied().collect::<BTreeSet<_>>();
    let available_collaboration = items
        .iter()
        .enumerate()
        .filter(|(_, item)| memory_tier(item) == MemoryTier::CollaborationPreference)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let available_cross_project = items
        .iter()
        .enumerate()
        .filter(|(_, item)| memory_tier(item) == MemoryTier::CrossProjectPrinciple)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let available_non_project = items
        .iter()
        .enumerate()
        .filter(|(_, item)| memory_tier(item) != MemoryTier::ProjectRule)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let available_project = items
        .iter()
        .enumerate()
        .filter(|(_, item)| memory_tier(item) == MemoryTier::ProjectRule)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();

    ensure_tier_presence(
        items,
        &mut selected,
        &selected_set,
        &confidence,
        &memory_tier,
        &available_collaboration,
    );
    let selected_set = selected.iter().copied().collect::<BTreeSet<_>>();
    ensure_tier_presence(
        items,
        &mut selected,
        &selected_set,
        &confidence,
        &memory_tier,
        &available_cross_project,
    );

    let target_non_project = available_non_project
        .len()
        .min(7)
        .min(limit.saturating_sub(1));
    while selected
        .iter()
        .filter(|index| memory_tier(&items[**index]) != MemoryTier::ProjectRule)
        .count()
        < target_non_project
    {
        let selected_set = selected.iter().copied().collect::<BTreeSet<_>>();
        let Some(replacement) = available_non_project
            .iter()
            .filter(|index| !selected_set.contains(index))
            .max_by(|left, right| {
                confidence(&items[**left]).total_cmp(&confidence(&items[**right]))
            })
            .copied()
        else {
            break;
        };
        let Some((drop_position, _)) = selected
            .iter()
            .enumerate()
            .filter(|(_, index)| memory_tier(&items[**index]) == MemoryTier::ProjectRule)
            .min_by(|(_, left), (_, right)| {
                confidence(&items[**left]).total_cmp(&confidence(&items[**right]))
            })
        else {
            break;
        };
        selected[drop_position] = replacement;
    }

    let max_non_project = available_non_project
        .len()
        .min(7)
        .min(limit.saturating_sub(1));
    while !available_project.is_empty()
        && selected
            .iter()
            .filter(|index| memory_tier(&items[**index]) != MemoryTier::ProjectRule)
            .count()
            > max_non_project
    {
        let selected_set = selected.iter().copied().collect::<BTreeSet<_>>();
        let Some(project_replacement) = available_project
            .iter()
            .filter(|index| !selected_set.contains(index))
            .max_by(|left, right| {
                confidence(&items[**left]).total_cmp(&confidence(&items[**right]))
            })
            .copied()
        else {
            break;
        };
        let Some((drop_position, _)) = selected
            .iter()
            .enumerate()
            .filter(|(_, index)| memory_tier(&items[**index]) != MemoryTier::ProjectRule)
            .min_by(|(_, left), (_, right)| {
                confidence(&items[**left]).total_cmp(&confidence(&items[**right]))
            })
        else {
            break;
        };
        selected[drop_position] = project_replacement;
    }

    selected.sort_by(|left, right| {
        confidence(&items[*right])
            .total_cmp(&confidence(&items[*left]))
            .then_with(|| left.cmp(right))
    });
    selected.dedup();
    selected
}

fn ensure_tier_presence<T>(
    items: &[T],
    selected: &mut [usize],
    selected_set: &BTreeSet<usize>,
    confidence: &impl Fn(&T) -> f32,
    memory_tier: &impl Fn(&T) -> MemoryTier,
    available: &[usize],
) {
    if available.is_empty()
        || selected
            .iter()
            .any(|index| available.iter().any(|candidate| candidate == index))
    {
        return;
    }
    let Some(replacement) = available
        .iter()
        .filter(|index| !selected_set.contains(index))
        .max_by(|left, right| confidence(&items[**left]).total_cmp(&confidence(&items[**right])))
        .copied()
    else {
        return;
    };
    if let Some((drop_position, _)) = selected
        .iter()
        .enumerate()
        .filter(|(_, index)| memory_tier(&items[**index]) == MemoryTier::ProjectRule)
        .min_by(|(_, left), (_, right)| {
            confidence(&items[**left]).total_cmp(&confidence(&items[**right]))
        })
    {
        selected[drop_position] = replacement;
    }
}

pub(super) fn candidate_cluster_key(candidate: &Candidate) -> String {
    let lower = candidate.body.to_lowercase();
    let domain = body_domain(&lower);
    format!(
        "{}:{}:{domain}",
        candidate.memory_tier.as_str(),
        candidate.kind
    )
}

pub(super) fn body_domain(body: &str) -> &'static str {
    let lower = body.to_lowercase();
    if contains_any(&lower, &["真实历史", "real history", "静态样例"]) {
        "real-history"
    } else if contains_any(&lower, &["候选质量", "候选数量"]) {
        "candidate-quality"
    } else if contains_any(&lower, &["持续自我修正", "自我修正", "真实反馈"]) {
        "self-correction"
    } else if contains_any(&lower, &["review", "merge", "审阅边界", "固化规则"]) {
        "review-boundary"
    } else if contains_any(&lower, &["小改快测", "大改重测", "完整回归", "快测"]) {
        "test-strategy"
    } else if contains_any(
        &lower,
        &[
            "真实结果",
            "推理引擎",
            "检查有没有问题",
            "自检",
            "dry-run",
            "dry run",
        ],
    ) {
        "self-verification"
    } else if contains_any(&lower, &["axios", "fetch", "ky", "ofetch", "http", "请求"]) {
        "http"
    } else if contains_any(&lower, &["cargo", "clippy", "rust", "test", "测试"]) {
        "validation"
    } else if contains_any(&lower, &["clarifying", "澄清", "先提问", "先规划", "plan"]) {
        "planning"
    } else if contains_any(&lower, &["agents.md", "claude.md", "memory_card", "draft"]) {
        "governance"
    } else if contains_any(&lower, &["用户", "ux", "体验", "responsive", "卡顿"]) {
        "user-experience"
    } else if contains_any(&lower, &["ui", "组件", "交互", "visual"]) {
        "ui"
    } else {
        "general"
    }
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Item {
        body: &'static str,
        confidence: f32,
        cluster: &'static str,
    }

    #[test]
    fn keeps_cluster_diversity_before_filling_by_confidence() {
        let items = [
            Item {
                body: "bun one",
                confidence: 0.99,
                cluster: "pm",
            },
            Item {
                body: "bun two",
                confidence: 0.98,
                cluster: "pm",
            },
            Item {
                body: "architecture",
                confidence: 0.70,
                cluster: "arch",
            },
        ];

        let selected = select_diverse_candidates(
            &items,
            2,
            |item| item.confidence,
            |item| item.cluster.to_string(),
        );

        assert_eq!(selected.len(), 2);
        assert!(selected.iter().any(|index| items[*index].body == "bun one"));
        assert!(
            selected
                .iter()
                .any(|index| items[*index].body == "architecture")
        );
    }
}
