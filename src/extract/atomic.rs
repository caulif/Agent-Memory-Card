pub(super) fn split_atomic_sentences(sentence: &str) -> Vec<String> {
    let trimmed = sentence.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let connectors = ["但是", "但", "除非", "except", "unless", "however"];
    for connector in connectors {
        let Some((left, right)) = split_once_case_insensitive(trimmed, connector) else {
            continue;
        };
        let left = left.trim_matches(['，', ',', ' ', '。', '.']).trim();
        let right = right.trim_matches(['，', ',', ' ', '。', '.']).trim();
        if left.len() < 8 || right.len() < 8 {
            continue;
        }
        return vec![left.to_string(), normalize_exception_clause(right)];
    }

    vec![trimmed.to_string()]
}

fn split_once_case_insensitive<'a>(text: &'a str, connector: &str) -> Option<(&'a str, &'a str)> {
    if connector.is_ascii() {
        let lower = text.to_lowercase();
        let index = lower.find(connector)?;
        Some((&text[..index], &text[index + connector.len()..]))
    } else {
        text.split_once(connector)
    }
}

fn normalize_exception_clause(clause: &str) -> String {
    let trimmed = clause.trim();
    let lower = trimmed.to_lowercase();
    if lower.contains("保留")
        || lower.contains("keep")
        || lower.contains("use ")
        || lower.contains("使用")
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
}
