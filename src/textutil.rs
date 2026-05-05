use std::collections::BTreeSet;

pub fn slug(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() || is_cjk(ch) {
            out.push(ch);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let slug = out.trim_matches('-').chars().take(48).collect::<String>();
    if slug.is_empty() {
        "draft".to_string()
    } else {
        slug
    }
}

pub fn normalize_string_list(values: Vec<String>) -> Vec<String> {
    let mut values = values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

pub fn tokenize(input: &str) -> BTreeSet<String> {
    let mut tokens = BTreeSet::new();
    let mut current = String::new();
    let mut current_kind = TokenKind::None;

    for ch in input.to_lowercase().chars() {
        let kind = if ch.is_ascii_alphanumeric() {
            TokenKind::Ascii
        } else if is_cjk(ch) {
            TokenKind::Cjk
        } else {
            TokenKind::None
        };

        if kind == TokenKind::None {
            push_token(&mut tokens, &mut current);
            current_kind = TokenKind::None;
            continue;
        }

        if current_kind != TokenKind::None && current_kind != kind {
            push_token(&mut tokens, &mut current);
        }
        current.push(ch);
        current_kind = kind;
    }
    push_token(&mut tokens, &mut current);
    tokens
}

pub fn jaccard_similarity(left: &str, right: &str) -> f32 {
    let left = tokenize(left);
    let right = tokenize(right);
    if left.is_empty() && right.is_empty() {
        return 1.0;
    }
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let intersection = left.intersection(&right).count() as f32;
    let union = left.union(&right).count() as f32;
    intersection / union
}

fn push_token(tokens: &mut BTreeSet<String>, current: &mut String) {
    if current.len() >= 2 || current.chars().any(is_cjk) {
        tokens.insert(std::mem::take(current));
    } else {
        current.clear();
    }
}

fn is_cjk(ch: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&ch)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenKind {
    None,
    Ascii,
    Cjk,
}
