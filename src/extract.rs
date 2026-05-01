use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use regex::Regex;

use crate::draft::{self, NewDraft};

#[derive(Debug)]
pub struct ExtractReport {
    pub created: Vec<String>,
    pub skipped: Vec<String>,
}

impl ExtractReport {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Agent-Kernel extract report\n\n");
        if self.created.is_empty() {
            out.push_str("No drafts created.\n");
        } else {
            out.push_str("Drafts created:\n");
            for id in &self.created {
                out.push_str(&format!("- {id}\n"));
            }
        }
        if !self.skipped.is_empty() {
            out.push_str("\nSkipped:\n");
            for item in &self.skipped {
                out.push_str(&format!("- {item}\n"));
            }
        }
        out
    }
}

#[derive(Debug, Clone)]
struct Candidate {
    title: String,
    body: String,
    kind: String,
    scope: String,
    evidence: String,
}

pub fn extract_to_drafts(
    project_root: &Path,
    text: Option<String>,
    file: Option<PathBuf>,
    targets: Vec<String>,
) -> Result<ExtractReport> {
    let (input, source) = match (text, file) {
        (Some(text), None) => (text, "inline text".to_string()),
        (None, Some(path)) => (
            fs::read_to_string(&path)?,
            format!("file:{}", path.to_string_lossy()),
        ),
        _ => return Err(anyhow!("provide exactly one of --text or --file")),
    };

    extract_text_to_drafts(project_root, &input, targets, &source)
}

pub fn extract_text_to_drafts(
    project_root: &Path,
    input: &str,
    targets: Vec<String>,
    source: &str,
) -> Result<ExtractReport> {
    let candidates = extract_candidates(input);
    let mut created = Vec::new();
    let mut skipped = Vec::new();
    for candidate in candidates {
        let id = draft_id(&candidate);
        let result = draft::add_draft(
            project_root,
            NewDraft {
                id: id.clone(),
                title: candidate.title,
                kind: candidate.kind,
                scope: candidate.scope,
                body: candidate.body,
                targets: targets.clone(),
                evidence: format!("{source}: {}", candidate.evidence),
            },
        );
        match result {
            Ok(()) => created.push(id),
            Err(error) => skipped.push(format!("{id}: {error}")),
        }
    }
    Ok(ExtractReport { created, skipped })
}

fn extract_candidates(input: &str) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    for sentence in split_sentences(input) {
        if !looks_like_rule(sentence) {
            continue;
        }
        let body = normalize_body(sentence);
        if body.len() < 12 {
            continue;
        }
        let title = title_from_body(&body);
        candidates.push(Candidate {
            title,
            body,
            kind: classify_kind(sentence).to_string(),
            scope: "project".to_string(),
            evidence: sentence.to_string(),
        });
    }
    dedupe_candidates(candidates)
}

fn split_sentences(input: &str) -> Vec<&str> {
    input
        .split(['\n', '。', '！', '？', '.', '!', '?'])
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect()
}

fn looks_like_rule(sentence: &str) -> bool {
    let lower = sentence.to_lowercase();
    let markers = [
        "以后",
        "必须",
        "不要",
        "禁止",
        "统一",
        "默认",
        "优先",
        "改用",
        "不再",
        "always",
        "never",
        "must",
        "prefer",
        "use ",
        "don't",
        "do not",
        "default to",
    ];
    markers.iter().any(|marker| lower.contains(marker))
}

fn classify_kind(sentence: &str) -> &'static str {
    let lower = sentence.to_lowercase();
    if lower.contains("不要")
        || lower.contains("禁止")
        || lower.contains("never")
        || lower.contains("do not")
        || lower.contains("don't")
    {
        "constraint"
    } else {
        "preference"
    }
}

fn normalize_body(sentence: &str) -> String {
    let mut body = sentence.trim().to_string();
    let replacements = [
        ("我再说最后一次，", ""),
        ("我再说最后一次", ""),
        ("以后", ""),
        ("请", ""),
        ("记住：", ""),
        ("记住:", ""),
    ];
    for (from, to) in replacements {
        body = body.replace(from, to);
    }
    body = body.trim_matches(['，', ',', ' ']).trim().to_string();
    if body.ends_with(';') {
        body.pop();
    }
    body
}

fn title_from_body(body: &str) -> String {
    let words = body.split_whitespace().collect::<Vec<_>>();
    if words.len() >= 3 {
        return words.iter().take(6).copied().collect::<Vec<_>>().join(" ");
    }
    body.chars().take(24).collect()
}

fn draft_id(candidate: &Candidate) -> String {
    let slug = slug(&candidate.title);
    format!("project:{slug}")
}

fn slug(input: &str) -> String {
    let re = Regex::new(r"[^a-zA-Z0-9\u4e00-\u9fff]+").expect("valid regex");
    let slug = re
        .replace_all(&input.to_lowercase(), "-")
        .trim_matches('-')
        .to_string();
    if slug.is_empty() {
        "draft".to_string()
    } else {
        slug.chars().take(48).collect()
    }
}

fn dedupe_candidates(candidates: Vec<Candidate>) -> Vec<Candidate> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for candidate in candidates {
        let key = candidate.body.to_lowercase();
        if seen.insert(key) {
            out.push(candidate);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_chinese_rule_candidate() {
        let candidates = extract_candidates("以后前端请求统一使用 Axios，不要再用 Fetch。");

        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].body.contains("Axios"));
    }

    #[test]
    fn extract_command_creates_draft() {
        let temp = tempfile::tempdir().expect("tempdir");

        let report = extract_to_drafts(
            temp.path(),
            Some("Always use pnpm for package management.".to_string()),
            None,
            vec!["codex".to_string()],
        )
        .expect("extract");

        assert_eq!(report.created.len(), 1);
        let drafts = draft::load_drafts(temp.path()).expect("drafts");
        assert_eq!(drafts[0].targets, vec!["codex"]);
    }
}
