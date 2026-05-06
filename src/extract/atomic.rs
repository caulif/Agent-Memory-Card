pub(super) fn split_atomic_sentences(sentence: &str) -> Vec<String> {
    let trimmed = sentence.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let connectors = [
        "additionally",
        "moreover",
        "同时",
        "另外",
        "而且",
        "此外",
        "also",
        "only when",
        "however",
        "although",
        "unless",
        "except",
        "while",
        "但是",
        "不过",
        "除非",
        "仅当",
        "只有",
        "if",
        "then",
        "如果",
        "那么",
        "但",
    ];
    for connector in connectors {
        let Some((left, right)) = split_once_case_insensitive(trimmed, connector) else {
            continue;
        };
        let left = left.trim_matches(['，', ',', ' ', '。', '.']).trim();
        let right = right.trim_matches(['，', ',', ' ', '。', '.']).trim();
        if left.len() < 8 || right.len() < 8 {
            continue;
        }
        let mut parts = vec![normalize_clause(left, connector)];
        parts.extend(
            split_atomic_sentences(&normalize_clause(right, connector))
                .into_iter()
                .filter(|part| !part.trim().is_empty()),
        );
        return parts;
    }

    vec![trimmed.to_string()]
}

fn split_once_case_insensitive<'a>(text: &'a str, connector: &str) -> Option<(&'a str, &'a str)> {
    if connector.is_ascii() {
        let lower = text.to_lowercase();
        for (index, _) in lower.match_indices(connector) {
            let before = lower[..index].chars().next_back();
            let after = lower[index + connector.len()..].chars().next();
            if before.is_some_and(|ch| ch.is_ascii_alphanumeric())
                || after.is_some_and(|ch| ch.is_ascii_alphanumeric())
            {
                continue;
            }
            return Some((&text[..index], &text[index + connector.len()..]));
        }
        None
    } else {
        text.split_once(connector)
    }
}

fn normalize_clause(clause: &str, connector: &str) -> String {
    let trimmed = clause.trim();
    let lower = trimmed.to_lowercase();

    let exception_connector = [
        "但是",
        "但",
        "不过",
        "除非",
        "仅当",
        "unless",
        "except",
        "however",
        "although",
        "only when",
        "while",
    ]
    .iter()
    .any(|item| connector.eq_ignore_ascii_case(item));

    if !exception_connector
        || lower.contains("保留")
        || lower.contains("keep")
        || lower.contains("use ")
        || lower.contains("使用")
        || lower.contains("用")
        || lower.contains("必须")
        || lower.contains("must")
    {
        trimmed.to_string()
    } else {
        format!("保留例外：{trimmed}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_default_rule_and_exception() {
        let parts = split_atomic_sentences(
            "以后 HTTP 请求默认用 Axios，但上传大文件保留 fetch，因为需要 streaming。",
        );

        assert_eq!(parts.len(), 2);
        assert!(parts[0].contains("Axios"));
        assert!(parts[1].contains("fetch"));
    }

    #[test]
    fn splits_additive_and_conditional_clauses() {
        let parts = split_atomic_sentences(
            "以后前端请求默认用 ky，同时 Node 包管理统一走 pnpm，另外只有上传大文件时保留 fetch。",
        );

        assert_eq!(parts.len(), 3);
        assert!(parts[0].contains("ky"));
        assert!(parts[1].contains("pnpm"));
        assert!(parts[2].contains("fetch"));
    }
}
