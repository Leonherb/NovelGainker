//! Bounded local EPUB building blocks. The coordinator owns authorization and disk effects.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(crate) mod assets;
pub(crate) mod epub;
pub(crate) mod markdown;
pub(crate) mod store;
pub(crate) mod tool;
pub use store::{PublicationDetail, PublicationPage, PublicationReceipt, PublicationStatus};
pub use tool::{NovelistPublishExecutorV2, PublicationAction, PublicationRequest};

/// Reconciles only a durable local result; it never rebuilds or calls a Provider.
pub(crate) fn task_recovery_result(
    handle: &crate::project::ProjectHandle,
    session_id: &str,
    turn_id: &str,
    call_id_hash: &str,
) -> Result<crate::tool_runtime::ToolResult, PublicationIssue> {
    use crate::tool_runtime::{ToolResult, ToolResultStatus};
    let records = store::load_all(handle)?;
    let mut matching = records.into_iter().filter(|record| {
        record.receipt.session_id == session_id
            && record.receipt.turn_id == turn_id
            && crate::workspace::sha256_hex(record.call_id.as_bytes()) == call_id_hash
    });
    let mut record = matching.next().ok_or(PublicationIssue::StoreUnavailable)?;
    if matching.next().is_some() || record.stage == store::Stage::Building {
        return Err(PublicationIssue::StoreUnavailable);
    }
    if record.stage == store::Stage::ReadyToPromote
        || record.receipt.status == PublicationStatus::Generated
    {
        let path = record
            .receipt
            .relative_path
            .as_ref()
            .ok_or(PublicationIssue::StoreUnavailable)?;
        let (hash, size) = store::file_hash(handle, path, MAX_PUBLICATION_OUTPUT)?;
        if Some(hash) != record.receipt.output_hash || Some(size) != record.receipt.size_bytes {
            return Err(PublicationIssue::StoreUnavailable);
        }
        record.receipt.status = PublicationStatus::Generated;
        record.receipt.issue = None;
    }
    let issue = record.receipt.issue;
    Ok(ToolResult {
        argument_issues: Vec::new(),
        call_id: record.call_id,
        name: "novelist.publish".to_owned(),
        version: 2,
        status: if issue == Some(PublicationIssue::Cancelled) {
            ToolResultStatus::Cancelled
        } else if issue.is_some() {
            ToolResultStatus::Failed
        } else {
            ToolResultStatus::Succeeded
        },
        output_json: if issue.is_none() {
            Some(
                serde_json::to_value(record.receipt)
                    .map_err(|_| PublicationIssue::StoreUnavailable)?,
            )
        } else {
            None
        },
        error_code: issue.map(|issue| issue.code().to_owned()),
    })
}
#[cfg(test)]
mod tests;

pub const MAX_PUBLICATION_CHAPTERS: usize = 500;
pub const MAX_PUBLICATION_TEXT: usize = 8 * 1024 * 1024;
pub const MAX_PUBLICATION_ASSETS: usize = 200;
pub const MAX_PUBLICATION_ASSET_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_PUBLICATION_OUTPUT: u64 = 112 * 1024 * 1024;
pub const MAX_PUBLICATION_RENDERED: usize = 8 * 1024 * 1024;
pub const MAX_PUBLICATION_RECORDS: usize = 256;
pub const MAX_PUBLICATION_RECORD_BYTES: u64 = 512 * 1024;
pub const MAX_PUBLICATION_STORE_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationIssue {
    MetadataRequired,
    NoFinalChapters,
    FinalConfirmationRequired,
    InvalidChapter,
    UnsupportedMarkdown,
    InvalidAsset,
    UnsafePath,
    LimitExceeded,
    SourceChanged,
    LocalValidationFailed,
    StoreUnavailable,
    SourceRequired,
    Cancelled,
}

impl PublicationIssue {
    pub const fn code(self) -> &'static str {
        match self {
            Self::MetadataRequired => "publication.metadata_required",
            Self::NoFinalChapters => "publication.no_final_chapters",
            Self::FinalConfirmationRequired => "publication.final_confirmation_required",
            Self::InvalidChapter => "publication.invalid_chapter",
            Self::UnsupportedMarkdown => "publication.unsupported_markdown",
            Self::InvalidAsset => "publication.invalid_asset",
            Self::UnsafePath => "publication.unsafe_path",
            Self::LimitExceeded => "publication.limit_exceeded",
            Self::SourceChanged => "publication.source_changed",
            Self::LocalValidationFailed => "publication.local_validation_failed",
            Self::StoreUnavailable => "publication.store_unavailable",
            Self::SourceRequired => "publication.source_required",
            Self::Cancelled => "agent.tool_cancelled",
        }
    }
    pub const fn message(self) -> &'static str {
        match self {
            Self::MetadataRequired => "请在 project.md 中补齐有效的 title、authorDisplayName 和 writingLanguage（zh-CN）。",
            Self::NoFinalChapters => "没有可出版的定稿章节；保存草稿不等于确认定稿。",
            Self::FinalConfirmationRequired => "存在没有当前有效定稿确认的 final 章节，请核对正文并重新明确确认定稿。",
            Self::InvalidChapter => "章节编码、身份、章号或 Draft 正文不合格，请修正后重新出版。",
            Self::UnsupportedMarkdown => "正文包含暂不支持的 Markdown（如表格、链接、嵌套标记或原始 HTML），本次未删减正文生成书籍。",
            Self::InvalidAsset => "图片缺失或其格式、尺寸、结构不合格；仅支持工程 assets 内已存在的 PNG/JPEG。",
            Self::UnsafePath => "输入或产物路径不安全，未执行本次出版。",
            Self::LimitExceeded => "本次出版超过已声明的输入、输出或存储上限，旧产物未被删除。",
            Self::SourceChanged => "书稿、资产或定稿确认已变化，本产物不能作为当前版本。",
            Self::LocalValidationFailed => "本地产物结构验证失败，不能报告为已生成。",
            Self::StoreUnavailable => "无法安全记录或读取出版结果；旧书稿和已有产物保持不变。",
            Self::SourceRequired => "出版需要本轮用户来源和实际读取的内置出版方法回执。",
            Self::Cancelled => "本次出版已取消，已有产物保留。",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BookMetadata {
    pub title: String,
    pub author: String,
    pub language: String,
    pub cover: Option<String>,
}

impl BookMetadata {
    pub fn parse(bytes: &[u8]) -> Result<Self, PublicationIssue> {
        if bytes.len() > 32 * 1024 {
            return Err(PublicationIssue::LimitExceeded);
        }
        let text = std::str::from_utf8(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes))
            .map_err(|_| PublicationIssue::MetadataRequired)?;
        let fields = novelgainker_validation::validated_frontmatter("project.md", text)
            .ok_or(PublicationIssue::MetadataRequired)?;
        let required = |name: &str| -> Result<String, PublicationIssue> {
            let value = fields.get(name).ok_or(PublicationIssue::MetadataRequired)?;
            if value.trim().is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
                return Err(PublicationIssue::MetadataRequired);
            }
            Ok(value.clone())
        };
        if required("kind")? != "project"
            || required("schemaVersion")? != "1"
            || required("writingLanguage")? != "zh-CN"
        {
            return Err(PublicationIssue::MetadataRequired);
        }
        let cover = fields
            .get("coverImage")
            .filter(|value| !value.trim().is_empty())
            .map(|value| assets::asset_path(value))
            .transpose()?;
        Ok(Self {
            title: required("title")?,
            author: required("authorDisplayName")?,
            language: required("writingLanguage")?,
            cover,
        })
    }
}

#[derive(Debug)]
pub(crate) struct BookChapter {
    pub number: u32,
    pub title: String,
    pub body: String,
}

#[derive(Debug)]
pub(crate) struct BookAsset {
    pub bytes: Vec<u8>,
    pub media_type: &'static str,
    pub extension: &'static str,
}

#[derive(Debug)]
pub(crate) struct BookSnapshot {
    pub metadata: BookMetadata,
    pub chapters: Vec<BookChapter>,
    pub assets: BTreeMap<String, BookAsset>,
}

pub(crate) fn xml(value: &str) -> Result<String, PublicationIssue> {
    if value.chars().any(|c| {
        !(c == '\t'
            || c == '\n'
            || c == '\r'
            || ('\u{20}'..='\u{d7ff}').contains(&c)
            || ('\u{e000}'..='\u{fffd}').contains(&c)
            || ('\u{10000}'..='\u{10ffff}').contains(&c))
    }) {
        return Err(PublicationIssue::LocalValidationFailed);
    }
    Ok(value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;"))
}
