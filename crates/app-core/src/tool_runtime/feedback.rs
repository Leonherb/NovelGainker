//! Bounded Provider-only diagnostics. Never echo argument values or unknown keys.
use super::{schema_type, validate_value, ToolDefinition};
use serde::Serialize;
use serde_json::Value;

#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct ArgumentIssue {
    path: String,
    issue: IssueKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected: Option<Value>,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum IssueKind {
    MissingField,
    UnexpectedFields,
    InvalidTypeOrValue,
}

impl ArgumentIssue {
    pub(crate) fn canon(index: usize, field: &'static str, expected: Value) -> Self {
        Self {
            path: format!("$.operations[{index}].{field}"),
            issue: IssueKind::InvalidTypeOrValue,
            expected: Some(expected),
        }
    }
}

impl ToolDefinition {
    pub(crate) fn argument_issues(&self, arguments: &Value) -> Vec<ArgumentIssue> {
        let mut issues = Vec::new();
        // Paths come exclusively from the validated Host schema, never model keys.
        if self.validate().is_ok() {
            collect(arguments, &self.input_schema, "$", &mut issues);
        }
        issues
    }
}

fn collect(value: &Value, schema: &Value, path: &str, issues: &mut Vec<ArgumentIssue>) {
    if issues.len() >= 4 || path.len() > 240 || validate_value(value, schema).is_ok() {
        return;
    }
    let push = |issues: &mut Vec<ArgumentIssue>, issue| {
        if issues.len() < 4 {
            issues.push(ArgumentIssue {
                path: path.to_owned(),
                issue,
                expected: expectation(schema),
            });
        }
    };
    let object = schema.as_object().expect("validated schema");
    let (kind, _) = schema_type(object).expect("validated schema type");
    if kind == "object" && value.is_object() {
        let map = value.as_object().expect("object");
        let properties = schema["properties"]
            .as_object()
            .expect("validated properties");
        for (key, child_schema) in properties {
            if issues.len() >= 4 {
                break;
            }
            let child_path = format!("{path}.{key}");
            if child_path.len() > 240 {
                continue;
            }
            if let Some(child) = map.get(key) {
                collect(child, child_schema, &child_path, issues);
            } else {
                issues.push(ArgumentIssue {
                    path: child_path,
                    issue: IssueKind::MissingField,
                    expected: expectation(child_schema),
                });
            }
        }
        if map.keys().any(|key| !properties.contains_key(key)) {
            push(issues, IssueKind::UnexpectedFields);
        }
    } else if kind == "array" && value.is_array() {
        for (index, item) in value.as_array().expect("array").iter().enumerate() {
            if issues.len() >= 4 {
                break;
            }
            collect(item, &schema["items"], &format!("{path}[{index}]"), issues);
        }
    } else {
        push(issues, IssueKind::InvalidTypeOrValue);
    }
}

// Only bounded type/enum data from the trusted schema; never values, keys or descriptions
// supplied by the model. Large enums are omitted rather than returned as a false subset.
fn expectation(schema: &Value) -> Option<Value> {
    let mut expected = serde_json::json!({"type": schema["type"]});
    if let Some(allowed) = schema.get("enum") {
        if serde_json::to_vec(allowed).is_ok_and(|bytes| bytes.len() <= 512) {
            expected["enum"] = allowed.clone();
        }
    }
    Some(expected)
}

pub(super) fn failure_guidance(code: Option<&str>) -> Option<&'static str> {
    Some(match code? {
        "agent.tool_arguments_invalid" => "工具参数校验失败。按当前 tools 中该函数的 parameters 逐项修正 argumentIssues（expected 给出类型及可选值）；所有 required 字段都必须存在，可空字段也必须显式传 null。不要原样重复，不要改用未注册工具。propose_change 的五类资料要求 chapterIntent/finalize/candidateId 均为 null；不支持修改 project.md。参数错误不表示缺少保存权限。",
        "canon.change_invalid" => "变更参数未满足语义约束。五类资料的 chapterIntent/finalize/candidateId 必须为 null；遵循当前工具 schema、方法说明和批次大小限制。不要原样重复，也不要声称写入工具不存在。",
        "canon.change_method_not_read" => "先通过 novelist.read_context 读取对应 capabilityId 的方法，使用本轮返回的 methodReceiptId；不要猜测或复用其他轮次的回执。",
        "canon.change_source_invalid" => "sources 必须引用 Host 消息来源目录中真实的 messageId/contentHash，并与 sourceKind 及用户授权对应；不要自行编造来源。",
        "canon.change_incomplete_read" | "canon.change_source_changed" => "更新前通过 novelist.read_context 完整重读目标，使用该次返回的 targetId/readReceiptId/expectedSourceHash；不要沿用旧版本或不完整读取的回执。",
        "canon.change_target_invalid" => "优先按 argumentIssues 指出的具体字段纠正，不要猜测失败原因。核对变更目标：新建三个目标字段必须显式为 null；更新使用 Host 完整读取回执。仅支持 character/world/material/plot/outline/chapter，不支持修改 project.md 或自行提供路径。",
        "canon.change_candidate_invalid" => "候选 Markdown 不符合方法模板或章节候选约束。优先按 argumentIssues 中的字段、行号和 recovery 纠正，不要凭通用提示猜测原因。先按 read_context.requiredTemplates 中对应模板修正 proposedMarkdown：必须是实际换行的完整 Markdown，不能用代码围栏包裹；填写完整 YAML，人物 schemaVersion: 2，其余资料和章节为 1，writingLanguage: zh-CN；entityKind=world 对应 kind: setting，其余 kind 与 entityKind 一致，新建用 id: new。替换模板占位符并遵守当前工具大小限制。接受章节候选须匹配 Host 已展示的完整版本。未取得 committed 回执不得声称已保存。",
        "canon.change_budget_exceeded" => "Host 本轮工具调用或安全预留上限已触发，本次请求未执行。不是 API 余额耗尽，也不是应用没有保存能力。仅使用当前仍开放的工具；没有工具时继续文字说明，列明实际 committed 回执和未保存内容，请用户下一轮继续。",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn issues_use_only_schema_paths_and_are_bounded() {
        let definition = crate::canon_change::propose_change_definition();
        let mut value = json!({"schemaVersion":99,"sourceKind":"user_decision","risk":"low","sources":[],"operations":[]});
        value["secret-or-manuscript-as-key"] = json!("private-body");
        let issues = serde_json::to_value(definition.argument_issues(&value)).unwrap();
        assert!(issues.to_string().contains("$.schemaVersion"));
        assert!(issues.to_string().contains("unexpected_fields"));
        assert!(!issues.to_string().contains("secret-or-manuscript"));
        assert!(!issues.to_string().contains("private-body"));
        assert_eq!(definition.argument_issues(&json!({})).len(), 4);
        value["schemaVersion"] = json!(1);
        value
            .as_object_mut()
            .unwrap()
            .remove("secret-or-manuscript-as-key");
        value["operations"] = json!([{"kind":"create"}]);
        assert!(serde_json::to_string(&definition.argument_issues(&value))
            .unwrap()
            .contains("$.operations[0]."));
    }

    #[test]
    fn invalid_enum_and_missing_nullable_field_include_expected_contract() {
        let definition = crate::canon_context::novelist_read_context_definition();
        let value = serde_json::json!({
            "capabilityId":"create-character", "capabilityResources":[],
            "canonOperations":[{"kind":"private-invalid-operation", "scopes":[]}]
        });
        let issues = serde_json::to_value(definition.argument_issues(&value)).unwrap();
        let issues = issues.as_array().unwrap();
        let kind = issues
            .iter()
            .find(|issue| issue["path"] == "$.canonOperations[0].kind")
            .unwrap();
        assert_eq!(
            kind["expected"]["enum"],
            serde_json::json!(["list", "search", "read"])
        );
        let nullable = issues
            .iter()
            .find(|issue| issue["path"] == "$.canonOperations[0].cursor")
            .unwrap();
        assert_eq!(
            nullable["expected"]["type"],
            serde_json::json!(["string", "null"])
        );
        assert!(!serde_json::to_string(issues)
            .unwrap()
            .contains("private-invalid-operation"));
    }

    #[test]
    fn diagnostics_cannot_turn_a_failed_call_into_a_success_or_unbounded_result() {
        use crate::tool_runtime::{ToolResult, ToolResultStatus};
        let definition = crate::canon_change::propose_change_definition();
        let mut result = ToolResult {
            argument_issues: definition.argument_issues(&json!({})),
            call_id: "invalid_call".to_owned(),
            name: definition.name,
            version: definition.version,
            status: ToolResultStatus::Failed,
            output_json: None,
            error_code: Some("agent.tool_arguments_invalid".to_owned()),
        };
        assert!(result.validate().is_ok());
        assert_eq!(result.provider_payload()["status"], "failed");
        result
            .argument_issues
            .push(result.argument_issues[0].clone());
        assert!(result.validate().is_err());
        result.argument_issues.pop();
        result.status = ToolResultStatus::Succeeded;
        result.error_code = None;
        result.output_json = Some(json!({}));
        assert!(result.validate().is_err());
    }
}
