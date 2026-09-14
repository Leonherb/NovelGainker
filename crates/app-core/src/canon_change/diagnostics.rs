//! Fixed-field Provider diagnostics; no source text, model keys or filesystem paths.
use super::*;
use crate::tool_runtime::ArgumentIssue;

impl ChangeEvidence {
    pub(crate) fn candidate_issues(&self, arguments: &Value, error: &str) -> Vec<ArgumentIssue> {
        if !matches!(
            error,
            "canon.change_candidate_invalid" | "canon.change_target_invalid"
        ) {
            return Vec::new();
        }
        let mut normalized = arguments.clone();
        normalize_new_canon_operations(&mut normalized);
        let Some(operations) = normalized["operations"].as_array() else {
            return Vec::new();
        };
        let mut issues = Vec::new();
        for (index, value) in operations.iter().take(MAX_CHANGE_TARGETS).enumerate() {
            let Ok(op) = serde_json::from_value::<CanonMutation>(value.clone()) else {
                continue;
            };
            let mut add = |field, expectation| {
                if issues.len() < 4 {
                    issues.push(ArgumentIssue::canon(index, field, expectation));
                }
            };
            let text = &op.proposed_markdown;
            if error == "canon.change_candidate_invalid"
                && op.entity_kind != CanonEntityKind::Chapter
                && validate_candidate(op.entity_kind, text).is_err()
            {
                if text.is_empty() || text.len() > MAX_CHANGE_FILE_BYTES {
                    add(
                        "proposedMarkdown",
                        json!({"code":"candidate.size","minBytes":1,"maxBytes":MAX_CHANGE_FILE_BYTES}),
                    );
                }
                if text.contains(['\r', '\0'])
                    || text.starts_with('\u{feff}')
                    || !text.ends_with('\n')
                {
                    add(
                        "proposedMarkdown",
                        json!({"code":"candidate.encoding","format":"UTF-8, LF, final LF, no BOM or NUL"}),
                    );
                }
                for diagnostic in validate_source(
                    &format!("{}/validation.md", op.entity_kind.directory()),
                    text,
                )
                .diagnostics
                .iter()
                .take(4)
                {
                    add(
                        "proposedMarkdown",
                        json!({"code":diagnostic.code,"line":diagnostic.line,"recovery":diagnostic.recovery}),
                    );
                }
                for (field, expected) in [
                    ("kind", op.entity_kind.metadata_kind()),
                    ("schemaVersion", op.entity_kind.metadata_version()),
                ] {
                    if metadata_field(text, field).as_deref() != Some(expected) {
                        add(
                            "proposedMarkdown",
                            json!({"frontmatterField":field,"value":expected}),
                        );
                    }
                }
                if !metadata_field(text, "id").is_some_and(|id| valid_identity(&id)) {
                    add(
                        "proposedMarkdown",
                        json!({"frontmatterField":"id","format":"1..80 ASCII letters, digits, hyphen or underscore"}),
                    );
                }
            }
            if error == "canon.change_target_invalid" {
                if op.kind == MutationKind::Create && op.target_id.is_some() {
                    add(
                        "targetId",
                        json!({"value":null,"reason":"create_has_no_existing_target"}),
                    );
                } else if op.kind == MutationKind::Update && op.target_id.is_none() {
                    add("targetId", json!({"from":"current full read receipt"}));
                }
                if op.target_id.is_none() {
                    if op.read_receipt_id.is_some() {
                        add("readReceiptId", json!({"value":null}));
                    }
                    if op.expected_source_hash.is_some() {
                        add("expectedSourceHash", json!({"value":null}));
                    }
                    if metadata_field(text, "id").as_deref() != Some("new") {
                        add(
                            "proposedMarkdown",
                            json!({"frontmatterField":"id","value":"new"}),
                        );
                    }
                } else if let Some(read) = op
                    .read_receipt_id
                    .as_ref()
                    .and_then(|id| self.reads.get(id))
                {
                    if op.target_id.as_deref() != Some(&read.target_id) {
                        add(
                            "targetId",
                            json!({"from":"same full read receipt as readReceiptId"}),
                        );
                    }
                    if op.entity_kind != read.entity_kind {
                        add("entityKind", json!({"value":read.entity_kind}));
                    }
                    if metadata_field(text, "id") != metadata_field(&read.content, "id") {
                        add(
                            "proposedMarkdown",
                            json!({"frontmatterField":"id","from":"unchanged id in fully read original Markdown"}),
                        );
                    }
                }
            }
        }
        while serde_json::to_vec(&issues).map_or(usize::MAX, |bytes| bytes.len()) > 2048 {
            issues.pop();
        }
        issues
    }
}
