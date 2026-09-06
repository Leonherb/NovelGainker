use super::*;
use crate::canon_change::{ChangeEvidence, SourceReference};
use crate::canon_change_store::CanonChangeCoordinator;
use crate::provider_core::CancellationToken;
use crate::tool_runtime::{
    ToolDefinition, ToolExecutionContext, ToolExecutionOutput, ToolExecutor, ToolResultStatus,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationAction {
    Preflight,
    Build,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublicationRequest {
    pub schema_version: u16,
    pub action: PublicationAction,
    pub method_receipt_id: String,
    pub source: SourceReference,
}

pub fn definition() -> ToolDefinition {
    ToolDefinition { name: "novelist.publish".to_owned(), version: 2,
        display_label: "本地 EPUB 预检 / 构建".to_owned(),
        description: "依据本轮用户明确要求和已读取的 publish-novel 方法预检或构建本地 EPUB。build 自带预检；每轮最多一次，预检后要构建需新的用户轮次。只导出当前有效定稿章 Draft 正文和已存在的受控 PNG/JPEG，缺作者 authorDisplayName 等元数据请用户编辑 project.md；缺封面允许无封面。Host 决定 published 下的新 UUID 路径，不覆盖旧书，不下载、不上架、不改 Canon。失败不抹掉聊天或已有产物。".to_owned(),
        input_schema: json!({"type":"object","additionalProperties":false,
            "required":["schemaVersion","action","methodReceiptId","source"],
            "properties":{
                "schemaVersion":{"type":"integer","enum":[2]},
                "action":{"type":"string","enum":["preflight","build"]},
                "methodReceiptId":{"type":"string"},
                "source":{"type":"object","additionalProperties":false,"required":["messageId","contentHash"],
                    "properties":{"messageId":{"type":"string"},
                        "contentHash":{"type":"string"}}}
            }}) }
}

pub struct NovelistPublishExecutorV2 {
    coordinator: Arc<CanonChangeCoordinator>,
    evidence: Arc<Mutex<ChangeEvidence>>,
}
impl NovelistPublishExecutorV2 {
    pub(crate) fn new(
        coordinator: Arc<CanonChangeCoordinator>,
        evidence: Arc<Mutex<ChangeEvidence>>,
    ) -> Self {
        Self {
            coordinator,
            evidence,
        }
    }
    pub fn definition(&self) -> ToolDefinition {
        definition()
    }
    fn decode(&self, arguments: &Value) -> Result<PublicationRequest, PublicationIssue> {
        definition()
            .validate_arguments(arguments)
            .map_err(|_| PublicationIssue::SourceRequired)?;
        let request: PublicationRequest = serde_json::from_value(arguments.clone())
            .map_err(|_| PublicationIssue::SourceRequired)?;
        let evidence = self
            .evidence
            .try_lock()
            .map_err(|_| PublicationIssue::SourceRequired)?;
        if request.schema_version != 2
            || !evidence.has_publication_method(&request.method_receipt_id)
            || !evidence.current_user_source(&request.source)
        {
            return Err(PublicationIssue::SourceRequired);
        }
        Ok(request)
    }
}
impl ToolExecutor for NovelistPublishExecutorV2 {
    fn validate_arguments(&self, arguments: &Value) -> Result<(), &'static str> {
        self.decode(arguments)
            .map(|_| ())
            .map_err(|_| "agent.tool_arguments_invalid")
    }
    fn execute(
        &self,
        context: &ToolExecutionContext,
        arguments: &Value,
        cancellation: &CancellationToken,
    ) -> ToolExecutionOutput {
        let result = self.decode(arguments).and_then(|request| {
            let evidence = self
                .evidence
                .try_lock()
                .map_err(|_| PublicationIssue::SourceRequired)?;
            self.coordinator
                .publish(&evidence, &request, context, cancellation)
        });
        match result {
            Ok(receipt)
                if matches!(
                    receipt.status,
                    PublicationStatus::Generated | PublicationStatus::PreflightPassed
                ) =>
            {
                ToolExecutionOutput {
                    status: ToolResultStatus::Succeeded,
                    output_json: serde_json::to_value(receipt).ok(),
                    error_code: None,
                }
            }
            Ok(receipt) => failed(receipt.issue.unwrap_or(PublicationIssue::StoreUnavailable)),
            Err(issue) => failed(issue),
        }
    }
}
fn failed(issue: PublicationIssue) -> ToolExecutionOutput {
    ToolExecutionOutput {
        status: if issue == PublicationIssue::Cancelled {
            ToolResultStatus::Cancelled
        } else {
            ToolResultStatus::Failed
        },
        output_json: None,
        error_code: Some(issue.code()),
    }
}
