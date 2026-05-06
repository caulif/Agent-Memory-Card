use std::collections::{BTreeMap, BTreeSet};

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

pub(super) fn candidate_cluster_key(candidate: &Candidate) -> String {
    let lower = candidate.body.to_lowercase();
    let domain = body_domain(&lower);
    format!("{}:{domain}", candidate.kind)
}

pub(super) fn body_domain(body: &str) -> &'static str {
    let lower = body.to_lowercase();
    if contains_any(&lower, &["axios", "fetch", "ky", "ofetch", "http", "请求"]) {
        "http"
    } else if contains_any(
        &lower,
        &["bun", "pnpm", "npm", "yarn", "package", "依赖", "脚本"],
    ) {
        "package-manager"
    } else if contains_any(&lower, &["cargo", "clippy", "rust", "test", "测试"]) {
        "validation"
    } else if contains_any(&lower, &["agents.md", "claude.md", "skilllet", "draft"]) {
        "governance"
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
