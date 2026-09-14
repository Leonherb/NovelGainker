//! Host-owned formatting for newly created data documents only. Existing
//! identities, source/read receipts, chapter candidates and story text are not repaired.
use super::*;

pub(crate) fn normalize_new_canon_operations(value: &mut Value) {
    let Some(operations) = value.get_mut("operations").and_then(Value::as_array_mut) else {
        return;
    };
    if operations.len() > MAX_CHANGE_TARGETS {
        return;
    }
    for operation in operations {
        if operation["kind"] != "create"
            || !["targetId", "readReceiptId", "expectedSourceHash"]
                .iter()
                .all(|key| operation.get(*key).is_some_and(Value::is_null))
        {
            continue;
        }
        let Ok(kind) = serde_json::from_value::<CanonEntityKind>(operation["entityKind"].clone())
        else {
            continue;
        };
        if kind == CanonEntityKind::Chapter {
            continue;
        }
        let Some(markdown) = operation["proposedMarkdown"].as_str() else {
            continue;
        };
        if markdown.len() > MAX_CHANGE_FILE_BYTES || markdown.contains('\0') {
            continue;
        }
        if let Some(normalized) = normalize(kind, markdown) {
            operation["proposedMarkdown"] = Value::String(normalized);
        }
    }
}

fn normalize(kind: CanonEntityKind, markdown: &str) -> Option<String> {
    let normalized = markdown
        .strip_prefix('\u{feff}')
        .unwrap_or(markdown)
        .replace("\r\n", "\n");
    if normalized.contains('\r') {
        return None;
    }
    let mut lines: Vec<String> = normalized.split('\n').map(str::to_owned).collect();
    if lines.first().map(String::as_str) != Some("---") {
        return None;
    }
    let end = lines.iter().skip(1).position(|line| line == "---")? + 1;
    let fixed = [
        ("schemaVersion", kind.metadata_version()),
        ("kind", kind.metadata_kind()),
        ("writingLanguage", "zh-CN"),
        ("id", "new"),
    ];
    let mut missing = Vec::new();
    for (key, expected) in fixed {
        let prefix = format!("{key}:");
        let indices: Vec<usize> = (1..end)
            .filter(|i| lines[*i].starts_with(&prefix))
            .collect();
        if indices.len() > 1 {
            return None;
        }
        if let Some(index) = indices.first() {
            // Do not consume nested values, anchors, or malformed YAML as scalars.
            if metadata_field(&normalized, key).is_none()
                || lines.get(index + 1).is_some_and(|line| {
                    line.starts_with(char::is_whitespace) && !line.trim().is_empty()
                })
            {
                return None;
            }
            let value = lines[*index][prefix.len()..].trim();
            if value.starts_with(['&', '*', '!', '|', '>']) {
                return None;
            }
            lines[*index] = format!("{key}: {expected}");
        } else {
            missing.push(format!("{key}: {expected}"));
        }
    }
    lines.splice(end..end, missing);
    let mut output = lines.join("\n");
    if !output.ends_with('\n') {
        output.push('\n');
    }
    (output.len() <= MAX_CHANGE_FILE_BYTES).then_some(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(kind: CanonEntityKind, markdown: &str) -> Value {
        json!({"operations":[{"kind":"create","entityKind":kind,"targetId":null,"readReceiptId":null,"expectedSourceHash":null,"proposedMarkdown":markdown}]})
    }
    #[test]
    fn five_new_document_kinds_are_canonical_without_changing_story_text() {
        for kind in [
            CanonEntityKind::Character,
            CanonEntityKind::World,
            CanonEntityKind::Material,
            CanonEntityKind::Plot,
            CanonEntityKind::Outline,
        ] {
            let body = "\n# 合成资料\n\n铜钱上的字迹保留。\n";
            let text=format!("---\nschemaVersion: 99\nkind: wrong\nid: made_up\ntitle: 合成资料\nwritingLanguage: en-US\n---\n{body}");
            let mut value = request(kind, &text.replace('\n', "\r\n"));
            normalize_new_canon_operations(&mut value);
            let actual = value["operations"][0]["proposedMarkdown"].as_str().unwrap();
            validate_candidate(kind, actual).unwrap();
            assert_eq!(metadata_field(actual, "id").as_deref(), Some("new"));
            assert_eq!(actual.split_once("\n---\n").unwrap().1, body);
            let once = value.clone();
            normalize_new_canon_operations(&mut value);
            assert_eq!(value, once);
        }
        let mut missing = request(
            CanonEntityKind::Character,
            "---\ntitle: 合成资料\n---\n正文",
        );
        normalize_new_canon_operations(&mut missing);
        validate_candidate(
            CanonEntityKind::Character,
            missing["operations"][0]["proposedMarkdown"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
    }
    #[test]
    fn normalization_cannot_repair_target_authority_updates_or_chapter_candidates() {
        let text="---\nid: made_up\nschemaVersion: 99\nkind: wrong\ntitle: 测试\nwritingLanguage: en-US\n---\n正文";
        for field in ["targetId", "readReceiptId", "expectedSourceHash"] {
            let mut value = request(CanonEntityKind::Character, text);
            value["operations"][0][field] = json!("invented");
            let before = value.clone();
            normalize_new_canon_operations(&mut value);
            assert_eq!(value, before);
        }
        for mutation in ["update", "upsert"] {
            let mut value = request(CanonEntityKind::Character, text);
            value["operations"][0]["kind"] = json!(mutation);
            let before = value.clone();
            normalize_new_canon_operations(&mut value);
            assert_eq!(value, before);
        }
        let mut chapter = request(CanonEntityKind::Chapter, text);
        let before = chapter.clone();
        normalize_new_canon_operations(&mut chapter);
        assert_eq!(chapter, before);
        for text in [
            "---\nid: x\nid: y\ntitle: t\n---\nbody\n",
            "---\nid: *anchor\ntitle: t\n---\nbody\n",
            "---\nid:\n  nested: x\ntitle: t\n---\nbody\n",
            "```markdown\n---\ntitle: t\n---\nbody\n```",
        ] {
            let mut value = request(CanonEntityKind::Character, text);
            let before = value.clone();
            normalize_new_canon_operations(&mut value);
            assert_eq!(value, before);
            assert!(validate_candidate(CanonEntityKind::Character, text).is_err());
        }
    }
}
