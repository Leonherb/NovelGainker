use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt;

pub const STORY_CONTEXT_MANIFEST_VERSION: u16 = 2;
pub const STORY_CONTEXT_WORKFLOW_VERSION: u16 = 1;
pub const STORY_REQUEST_LIMIT_BYTES: usize = 96 * 1024;
pub const STORY_PROTOCOL_BUDGET_BYTES: usize = 24 * 1024;
pub const STORY_PROJECT_CONTEXT_BUDGET_BYTES: usize = 48 * 1024;
pub const STORY_SAFETY_RESERVE_BYTES: usize = 24 * 1024;
pub const STORY_SINGLE_SOURCE_LIMIT_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StoryActionV2 {
    Create,
    Revise,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StoryContextSourceKindV2 {
    BuiltinProtocol,
    BuiltinSchema,
    BuiltinTemplate,
    ProjectRules,
    Plot,
    Outline,
    Style,
    Ledger,
    ChapterDraft,
    Character,
    World,
    Material,
}

impl StoryContextSourceKindV2 {
    fn uses_protocol_budget(self) -> bool {
        matches!(
            self,
            Self::BuiltinProtocol | Self::BuiltinSchema | Self::BuiltinTemplate
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StoryContextSelectionReasonV2 {
    Required,
    Explicit,
    DefaultPlot,
    DefaultOutline,
    DefaultStyle,
    DefaultLedger,
    RecentChapter,
    Available,
}

#[derive(Clone, PartialEq, Eq)]
pub struct StoryContextSourceInputV2 {
    pub source_id: String,
    pub kind: StoryContextSourceKindV2,
    pub relative_path: Option<String>,
    pub builtin_resource_id: Option<String>,
    pub display_name: String,
    pub selection_reason: StoryContextSelectionReasonV2,
    pub required: bool,
    pub user_deselectable: bool,
    pub priority: u16,
    pub content: String,
    pub content_hash: String,
    pub source_byte_count: u64,
    pub modified_time_ms: u64,
    pub included: bool,
}

impl fmt::Debug for StoryContextSourceInputV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoryContextSourceInputV2")
            .field("source_id", &self.source_id)
            .field("kind", &self.kind)
            .field("relative_path", &self.relative_path)
            .field("builtin_resource_id", &self.builtin_resource_id)
            .field("display_name", &self.display_name)
            .field("selection_reason", &self.selection_reason)
            .field("required", &self.required)
            .field("user_deselectable", &self.user_deselectable)
            .field("priority", &self.priority)
            .field("content", &"[REDACTED]")
            .field("content_hash", &self.content_hash)
            .field("source_byte_count", &self.source_byte_count)
            .field("modified_time_ms", &self.modified_time_ms)
            .field("included", &self.included)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildStoryContextInputV2 {
    pub workflow_id: String,
    pub project_root_identity: String,
    pub action: StoryActionV2,
    pub chapter_number: u32,
    pub chapter_title: String,
    pub sources: Vec<StoryContextSourceInputV2>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryTargetChapterV2 {
    pub chapter_number: u32,
    pub chapter_title: String,
    pub relative_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StoryContextTruncationReasonV2 {
    SingleSourceLimit,
    ProtocolBudget,
    ContextBudget,
    NotSelected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryContextTruncationV2 {
    pub reason: StoryContextTruncationReasonV2,
    pub original_byte_length: u64,
    pub retained_byte_length: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryContextManifestSourceV2 {
    pub source_id: String,
    pub kind: StoryContextSourceKindV2,
    pub relative_path: Option<String>,
    pub builtin_resource_id: Option<String>,
    pub display_name: String,
    pub selection_reason: StoryContextSelectionReasonV2,
    pub required: bool,
    pub user_deselectable: bool,
    pub priority: u16,
    pub source_byte_count: u64,
    pub original_byte_length: u64,
    pub included_byte_length: u64,
    pub content_hash: String,
    pub included_content_hash: String,
    pub truncation: Option<StoryContextTruncationV2>,
    pub included: bool,
    pub modified_time_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryContextBudgetV2 {
    pub request_limit_bytes: u64,
    pub protocol_budget_bytes: u64,
    pub project_context_budget_bytes: u64,
    pub safety_reserve_bytes: u64,
    pub single_source_limit_bytes: u64,
    pub protocol_bytes_used: u64,
    pub project_context_bytes_used: u64,
    pub total_included_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryContextManifestV2 {
    pub manifest_version: u16,
    pub workflow_id: String,
    pub workflow_version: u16,
    pub action: StoryActionV2,
    pub project_root_identity: String,
    pub target_chapter: StoryTargetChapterV2,
    pub selected_sources: Vec<StoryContextManifestSourceV2>,
    pub excluded_sources: Vec<StoryContextManifestSourceV2>,
    pub source_order: Vec<String>,
    pub budget: StoryContextBudgetV2,
    pub aggregate_hash: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoryContextBuildError {
    InvalidInput,
    InvalidSource,
    DuplicateSource,
    RequiredSourceExcluded,
    BudgetExceeded,
    Serialization,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ManifestHashMaterial<'a> {
    manifest_version: u16,
    workflow_id: &'a str,
    workflow_version: u16,
    action: StoryActionV2,
    project_root_identity: &'a str,
    target_chapter: &'a StoryTargetChapterV2,
    selected_sources: &'a [StoryContextManifestSourceV2],
    excluded_sources: &'a [StoryContextManifestSourceV2],
    source_order: &'a [String],
    budget: &'a StoryContextBudgetV2,
    created_at: u64,
}

pub fn build_story_context_manifest(
    input: BuildStoryContextInputV2,
) -> Result<StoryContextManifestV2, StoryContextBuildError> {
    validate_build_input(&input)?;
    let target_chapter = StoryTargetChapterV2 {
        chapter_number: input.chapter_number,
        chapter_title: input.chapter_title.trim().to_owned(),
        relative_path: format!("chapters/{:03}.md", input.chapter_number),
    };
    let created_at = input
        .sources
        .iter()
        .map(|source| source.modified_time_ms)
        .max()
        .unwrap_or(0);
    let mut sources = input.sources;
    sources.sort_by(|left, right| {
        left.priority
            .cmp(&right.priority)
            .then_with(|| left.source_id.cmp(&right.source_id))
    });

    let mut protocol_remaining = STORY_PROTOCOL_BUDGET_BYTES;
    let mut context_remaining = STORY_PROJECT_CONTEXT_BUDGET_BYTES;
    let mut selected_sources = Vec::new();
    let mut excluded_sources = Vec::new();

    for source in sources {
        if !source.included {
            if source.required {
                return Err(StoryContextBuildError::RequiredSourceExcluded);
            }
            excluded_sources.push(manifest_source(
                source,
                0,
                Some(StoryContextTruncationReasonV2::NotSelected),
            ));
            continue;
        }

        let original = source.content.len();
        let uses_protocol_budget = source.kind.uses_protocol_budget();
        let remaining = if uses_protocol_budget {
            protocol_remaining
        } else {
            context_remaining
        };
        let per_source_limit = if uses_protocol_budget {
            original
        } else {
            STORY_SINGLE_SOURCE_LIMIT_BYTES
        };
        let retained_limit = original.min(per_source_limit).min(remaining);
        if retained_limit == 0 && source.required {
            return Err(StoryContextBuildError::BudgetExceeded);
        }
        if retained_limit == 0 {
            let reason = if uses_protocol_budget {
                StoryContextTruncationReasonV2::ProtocolBudget
            } else {
                StoryContextTruncationReasonV2::ContextBudget
            };
            excluded_sources.push(manifest_source(source, 0, Some(reason)));
            continue;
        }
        let retained = utf8_prefix(&source.content, retained_limit);
        let retained_bytes = retained.len();
        let included_content_hash = sha256_hex(retained.as_bytes());
        if uses_protocol_budget {
            protocol_remaining = protocol_remaining.saturating_sub(retained_bytes);
        } else {
            context_remaining = context_remaining.saturating_sub(retained_bytes);
        }
        let reason = if retained_bytes == original {
            None
        } else if !uses_protocol_budget && original > STORY_SINGLE_SOURCE_LIMIT_BYTES {
            Some(StoryContextTruncationReasonV2::SingleSourceLimit)
        } else if uses_protocol_budget {
            Some(StoryContextTruncationReasonV2::ProtocolBudget)
        } else {
            Some(StoryContextTruncationReasonV2::ContextBudget)
        };
        selected_sources.push(manifest_source_with_hash(
            source,
            retained_bytes,
            reason,
            included_content_hash,
        ));
    }

    let source_order = selected_sources
        .iter()
        .map(|source| source.source_id.clone())
        .collect::<Vec<_>>();
    let protocol_bytes_used = STORY_PROTOCOL_BUDGET_BYTES - protocol_remaining;
    let project_context_bytes_used = STORY_PROJECT_CONTEXT_BUDGET_BYTES - context_remaining;
    let budget = StoryContextBudgetV2 {
        request_limit_bytes: STORY_REQUEST_LIMIT_BYTES as u64,
        protocol_budget_bytes: STORY_PROTOCOL_BUDGET_BYTES as u64,
        project_context_budget_bytes: STORY_PROJECT_CONTEXT_BUDGET_BYTES as u64,
        safety_reserve_bytes: STORY_SAFETY_RESERVE_BYTES as u64,
        single_source_limit_bytes: STORY_SINGLE_SOURCE_LIMIT_BYTES as u64,
        protocol_bytes_used: protocol_bytes_used as u64,
        project_context_bytes_used: project_context_bytes_used as u64,
        total_included_bytes: (protocol_bytes_used + project_context_bytes_used) as u64,
    };
    let material = ManifestHashMaterial {
        manifest_version: STORY_CONTEXT_MANIFEST_VERSION,
        workflow_id: &input.workflow_id,
        workflow_version: STORY_CONTEXT_WORKFLOW_VERSION,
        action: input.action,
        project_root_identity: &input.project_root_identity,
        target_chapter: &target_chapter,
        selected_sources: &selected_sources,
        excluded_sources: &excluded_sources,
        source_order: &source_order,
        budget: &budget,
        created_at,
    };
    let serialized =
        serde_json::to_vec(&material).map_err(|_| StoryContextBuildError::Serialization)?;
    let aggregate_hash = sha256_hex(&serialized);
    Ok(StoryContextManifestV2 {
        manifest_version: STORY_CONTEXT_MANIFEST_VERSION,
        workflow_id: input.workflow_id,
        workflow_version: STORY_CONTEXT_WORKFLOW_VERSION,
        action: input.action,
        project_root_identity: input.project_root_identity,
        target_chapter,
        selected_sources,
        excluded_sources,
        source_order,
        budget,
        aggregate_hash,
        created_at,
    })
}

fn validate_build_input(input: &BuildStoryContextInputV2) -> Result<(), StoryContextBuildError> {
    let title = input.chapter_title.trim();
    if input.workflow_id != crate::STORYTELLING_WORKFLOW_ID
        || input.project_root_identity.is_empty()
        || input.project_root_identity.len() > 128
        || input.chapter_number == 0
        || input.chapter_number > 999_999
        || title.is_empty()
        || title.chars().count() > 120
        || title.chars().any(char::is_control)
        || input.sources.is_empty()
    {
        return Err(StoryContextBuildError::InvalidInput);
    }
    let mut ids = BTreeSet::new();
    for source in &input.sources {
        let exactly_one_location =
            source.relative_path.is_some() ^ source.builtin_resource_id.is_some();
        if !ids.insert(source.source_id.as_str()) {
            return Err(StoryContextBuildError::DuplicateSource);
        }
        if source.source_id.is_empty()
            || source.source_id.len() > 600
            || !exactly_one_location
            || source.display_name.trim().is_empty()
            || source.display_name.chars().count() > 160
            || !is_sha256(&source.content_hash)
            || source.source_byte_count < source.content.len() as u64
            || (source.required && source.user_deselectable)
        {
            return Err(StoryContextBuildError::InvalidSource);
        }
    }
    Ok(())
}

fn manifest_source(
    source: StoryContextSourceInputV2,
    included_byte_length: usize,
    reason: Option<StoryContextTruncationReasonV2>,
) -> StoryContextManifestSourceV2 {
    manifest_source_with_hash(source, included_byte_length, reason, sha256_hex(&[]))
}

fn manifest_source_with_hash(
    source: StoryContextSourceInputV2,
    included_byte_length: usize,
    reason: Option<StoryContextTruncationReasonV2>,
    included_content_hash: String,
) -> StoryContextManifestSourceV2 {
    let original_byte_length = source.content.len();
    StoryContextManifestSourceV2 {
        source_id: source.source_id,
        kind: source.kind,
        relative_path: source.relative_path,
        builtin_resource_id: source.builtin_resource_id,
        display_name: source.display_name,
        selection_reason: source.selection_reason,
        required: source.required,
        user_deselectable: source.user_deselectable,
        priority: source.priority,
        source_byte_count: source.source_byte_count,
        original_byte_length: original_byte_length as u64,
        included_byte_length: included_byte_length as u64,
        content_hash: source.content_hash,
        included_content_hash,
        truncation: reason.map(|reason| StoryContextTruncationV2 {
            reason,
            original_byte_length: original_byte_length as u64,
            retained_byte_length: included_byte_length as u64,
        }),
        included: included_byte_length > 0,
        modified_time_ms: source.modified_time_ms,
    }
}

fn utf8_prefix(value: &str, maximum_bytes: usize) -> &str {
    if value.len() <= maximum_bytes {
        return value;
    }
    let mut boundary = maximum_bytes;
    while boundary > 0 && !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    &value[..boundary]
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(
        source_id: &str,
        priority: u16,
        content: &str,
        required: bool,
        included: bool,
    ) -> StoryContextSourceInputV2 {
        StoryContextSourceInputV2 {
            source_id: source_id.to_owned(),
            kind: if source_id.starts_with("builtin:") {
                StoryContextSourceKindV2::BuiltinProtocol
            } else {
                StoryContextSourceKindV2::Material
            },
            relative_path: (!source_id.starts_with("builtin:"))
                .then(|| "materials/a.md".to_owned()),
            builtin_resource_id: source_id
                .starts_with("builtin:")
                .then(|| "storytelling/workflow.md".to_owned()),
            display_name: source_id.to_owned(),
            selection_reason: if required {
                StoryContextSelectionReasonV2::Required
            } else {
                StoryContextSelectionReasonV2::Explicit
            },
            required,
            user_deselectable: !required,
            priority,
            content: content.to_owned(),
            content_hash: sha256_hex(content.as_bytes()),
            source_byte_count: content.len() as u64,
            modified_time_ms: 42,
            included,
        }
    }

    fn input(sources: Vec<StoryContextSourceInputV2>) -> BuildStoryContextInputV2 {
        BuildStoryContextInputV2 {
            workflow_id: crate::STORYTELLING_WORKFLOW_ID.to_owned(),
            project_root_identity: "project_fixture".to_owned(),
            action: StoryActionV2::Create,
            chapter_number: 7,
            chapter_title: "潮痕".to_owned(),
            sources,
        }
    }

    #[test]
    fn manifest_is_stable_sorted_and_explains_exclusions() {
        let sources = vec![
            source("project:z", 40, "可选", false, false),
            source("project:a", 20, "正文", false, true),
            source("builtin:workflow", 1, "协议", true, true),
        ];
        let first = build_story_context_manifest(input(sources.clone())).unwrap();
        let second = build_story_context_manifest(input(sources)).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.source_order, ["builtin:workflow", "project:a"]);
        assert_eq!(first.excluded_sources.len(), 1);
        assert_eq!(
            first.excluded_sources[0]
                .truncation
                .as_ref()
                .unwrap()
                .reason,
            StoryContextTruncationReasonV2::NotSelected
        );
        assert_eq!(first.aggregate_hash.len(), 64);
        assert_eq!(first.target_chapter.relative_path, "chapters/007.md");
    }

    #[test]
    fn utf8_source_is_truncated_without_splitting_a_character() {
        let content = "潮".repeat(4_000);
        let manifest = build_story_context_manifest(input(vec![
            source("builtin:workflow", 1, "协议", true, true),
            source("project:large", 20, &content, false, true),
        ]))
        .unwrap();
        let retained = &manifest.selected_sources[1];
        assert!(retained.included_byte_length <= STORY_SINGLE_SOURCE_LIMIT_BYTES as u64);
        assert_eq!(retained.included_byte_length % 3, 0);
        assert_eq!(
            retained.truncation.as_ref().unwrap().reason,
            StoryContextTruncationReasonV2::SingleSourceLimit
        );
    }

    #[test]
    fn a_changed_source_hash_changes_the_aggregate_hash() {
        let first = build_story_context_manifest(input(vec![source(
            "builtin:workflow",
            1,
            "协议一",
            true,
            true,
        )]))
        .unwrap();
        let second = build_story_context_manifest(input(vec![source(
            "builtin:workflow",
            1,
            "协议二",
            true,
            true,
        )]))
        .unwrap();
        assert_ne!(first.aggregate_hash, second.aggregate_hash);
    }

    #[test]
    fn context_budget_excludes_lower_priority_sources_and_explains_it() {
        let body = "x".repeat(STORY_SINGLE_SOURCE_LIMIT_BYTES);
        let mut sources = vec![source("builtin:workflow", 1, "协议", true, true)];
        for index in 0..7 {
            sources.push(source(
                &format!("project:{index}"),
                20 + index,
                &body,
                false,
                true,
            ));
        }
        let manifest = build_story_context_manifest(input(sources)).unwrap();
        assert_eq!(manifest.budget.project_context_bytes_used, 48 * 1024);
        assert_eq!(manifest.selected_sources.len(), 7);
        assert_eq!(manifest.excluded_sources.len(), 1);
        assert_eq!(manifest.excluded_sources[0].source_id, "project:6");
        assert_eq!(
            manifest.excluded_sources[0]
                .truncation
                .as_ref()
                .unwrap()
                .reason,
            StoryContextTruncationReasonV2::ContextBudget
        );
    }

    #[test]
    fn changing_an_optional_selection_changes_the_aggregate_hash() {
        let included = build_story_context_manifest(input(vec![
            source("builtin:workflow", 1, "协议", true, true),
            source("project:a", 20, "正文", false, true),
        ]))
        .unwrap();
        let excluded = build_story_context_manifest(input(vec![
            source("builtin:workflow", 1, "协议", true, true),
            source("project:a", 20, "正文", false, false),
        ]))
        .unwrap();
        assert_ne!(included.aggregate_hash, excluded.aggregate_hash);
    }

    #[test]
    fn required_sources_cannot_be_excluded() {
        let error = build_story_context_manifest(input(vec![source(
            "builtin:workflow",
            1,
            "协议",
            true,
            false,
        )]))
        .unwrap_err();
        assert_eq!(error, StoryContextBuildError::RequiredSourceExcluded);
    }
}
