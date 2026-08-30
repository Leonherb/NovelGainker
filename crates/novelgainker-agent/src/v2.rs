use crate::SKILL_IDS;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const AGENT_V2_SCHEMA_VERSION: u16 = 2;
pub const MAX_SELECTED_SKILLS_V2: usize = 4;
pub const MAX_CONTEXT_QUERIES_V2: usize = 8;
pub const MAX_CONTEXT_QUERY_TERMS_V2: usize = 16;
pub const MAX_VISIBLE_REASON_BYTES_V2: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderModeV2 {
    Control,
    Conversation,
    Composition,
    Reconciliation,
    MemoryCompaction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionContractV2 {
    SkillSelectionV2,
    AgentActionV2,
    ConversationTextV1,
    CompositionMarkdownV1,
    CanonProposalV1,
    MemoryCompactionV1,
}

impl CompletionContractV2 {
    pub fn is_valid_for(self, mode: ProviderModeV2) -> bool {
        matches!(
            (mode, self),
            (
                ProviderModeV2::Control,
                Self::SkillSelectionV2 | Self::AgentActionV2
            ) | (ProviderModeV2::Conversation, Self::ConversationTextV1)
                | (ProviderModeV2::Composition, Self::CompositionMarkdownV1)
                | (ProviderModeV2::Reconciliation, Self::CanonProposalV1)
                | (ProviderModeV2::MemoryCompaction, Self::MemoryCompactionV1)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderInvocationV2 {
    pub request_id: String,
    pub mode: ProviderModeV2,
    pub completion_contract: CompletionContractV2,
    pub budget_lease_id: String,
    pub cancellation_id: String,
}

impl ProviderInvocationV2 {
    pub fn validate(&self) -> Result<(), AgentV2ContractError> {
        validate_opaque_id(&self.request_id, "req_")?;
        validate_opaque_id(&self.budget_lease_id, "bud_")?;
        validate_opaque_id(&self.cancellation_id, "can_")?;
        if !self.completion_contract.is_valid_for(self.mode) {
            return Err(AgentV2ContractError::CompletionContractMismatch);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillSelectionV2 {
    pub schema_version: u16,
    pub request_id: String,
    pub selected_skill_ids: Vec<String>,
    pub intent_class: SkillIntentClassV2,
    pub visible_reason: String,
}

impl SkillSelectionV2 {
    pub fn validate(&self, expected_request_id: &str) -> Result<(), AgentV2ContractError> {
        if self.schema_version != AGENT_V2_SCHEMA_VERSION {
            return Err(AgentV2ContractError::UnsupportedVersion);
        }
        if self.request_id != expected_request_id {
            return Err(AgentV2ContractError::RequestMismatch);
        }
        validate_skill_ids(&self.selected_skill_ids, true)?;
        validate_visible_text(&self.visible_reason, MAX_VISIBLE_REASON_BYTES_V2)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillIntentClassV2 {
    ProductHelp,
    ProjectDiscussion,
    CanonUpdate,
    Composition,
    Revision,
    Review,
    Visual,
    Publish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextQueryKindV2 {
    EntityOrRule,
    Relationship,
    PlotThread,
    ChapterOrScene,
    Timeline,
    ClueOrMaterial,
    FullText,
    MarkedContext,
    SkillResource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextQueryV2 {
    pub kind: ContextQueryKindV2,
    pub terms: Vec<String>,
    pub known_entity_ids: Vec<String>,
    pub known_source_ids: Vec<String>,
    pub purpose: String,
}

impl ContextQueryV2 {
    fn validate(&self) -> Result<(), AgentV2ContractError> {
        if self.terms.is_empty()
            || self.terms.len() > MAX_CONTEXT_QUERY_TERMS_V2
            || self.known_entity_ids.len() > 16
            || self.known_source_ids.len() > 16
        {
            return Err(AgentV2ContractError::InvalidContextQuery);
        }
        for term in &self.terms {
            validate_visible_text(term, 128)?;
        }
        for entity_id in &self.known_entity_ids {
            validate_semantic_entity_id(entity_id)?;
        }
        for source_id in &self.known_source_ids {
            validate_reference_id(source_id, "src_")?;
        }
        validate_visible_text(&self.purpose, 256)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MarkSuggestionV2 {
    pub source_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanonTargetKindV1 {
    Character,
    Setting,
    Material,
    Plot,
    Chapter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanonProposalOperationV1 {
    Create,
    Extend,
    AppendChapter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanonFactSourceKindV1 {
    VisibleMessage,
    DraftArtifact,
    CanonSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonFactSourceV1 {
    pub kind: CanonFactSourceKindV1,
    pub source_id: String,
    pub source_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonFactV1 {
    pub field: String,
    pub value: String,
    pub sources: Vec<CanonFactSourceV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonProposalV1 {
    pub schema_version: u16,
    pub request_id: String,
    pub proposal_id: String,
    pub target_kind: CanonTargetKindV1,
    pub display_name: String,
    pub operation: CanonProposalOperationV1,
    pub facts: Vec<CanonFactV1>,
}

impl CanonProposalV1 {
    pub fn validate(
        &self,
        expected_request_id: &str,
        allowed_sources: &[(String, String)],
    ) -> Result<(), AgentV2ContractError> {
        if self.schema_version != 1
            || self.request_id != expected_request_id
            || validate_opaque_id(&self.proposal_id, "prop_").is_err()
            || self.facts.is_empty()
            || self.facts.len() > 32
        {
            return Err(AgentV2ContractError::InvalidCanonProposal);
        }
        validate_visible_text(&self.display_name, 256)?;
        let allowed = allowed_sources
            .iter()
            .map(|(id, hash)| (id.as_str(), hash.as_str()))
            .collect::<BTreeSet<_>>();
        let mut total_value_bytes = 0usize;
        for fact in &self.facts {
            validate_visible_text(&fact.field, 128)?;
            validate_visible_text(&fact.value, 64 * 1024)?;
            total_value_bytes = total_value_bytes.saturating_add(fact.value.len());
            if fact.sources.is_empty()
                || fact.sources.len() > 8
                || fact.sources.iter().any(|source| {
                    validate_hash(&source.source_hash).is_err()
                        || !allowed
                            .contains(&(source.source_id.as_str(), source.source_hash.as_str()))
                })
            {
                return Err(AgentV2ContractError::InvalidCanonProposal);
            }
        }
        if total_value_bytes > 128 * 1024 {
            return Err(AgentV2ContractError::InvalidCanonProposal);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "action",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum AgentControlActionV2 {
    RequestContext {
        schema_version: u16,
        request_id: String,
        selected_skill_ids: Vec<String>,
        visible_reason: String,
        queries: Vec<ContextQueryV2>,
    },
    StartResponse {
        schema_version: u16,
        request_id: String,
        selected_skill_ids: Vec<String>,
        visible_reason: String,
        response_brief: String,
        source_ids: Vec<String>,
        mark_suggestions: Vec<MarkSuggestionV2>,
    },
    SubmitCanonProposal {
        schema_version: u16,
        request_id: String,
        selected_skill_ids: Vec<String>,
        visible_reason: String,
        target_kind: CanonTargetKindV1,
        display_name: Option<String>,
        proposal_brief: String,
        source_ids: Vec<String>,
    },
}

impl AgentControlActionV2 {
    pub fn validate(
        &self,
        expected_request_id: &str,
        selected_skills: &[String],
    ) -> Result<(), AgentV2ContractError> {
        let (schema_version, request_id, skill_ids, visible_reason) = match self {
            Self::RequestContext {
                schema_version,
                request_id,
                selected_skill_ids,
                visible_reason,
                queries,
            } => {
                if queries.is_empty() || queries.len() > MAX_CONTEXT_QUERIES_V2 {
                    return Err(AgentV2ContractError::InvalidContextQuery);
                }
                for query in queries {
                    query.validate()?;
                }
                (
                    schema_version,
                    request_id,
                    selected_skill_ids,
                    visible_reason,
                )
            }
            Self::StartResponse {
                schema_version,
                request_id,
                selected_skill_ids,
                visible_reason,
                response_brief,
                source_ids,
                mark_suggestions,
            } => {
                validate_visible_text(response_brief, 1024)?;
                if source_ids.len() > 48 || mark_suggestions.len() > 8 {
                    return Err(AgentV2ContractError::InvalidContextQuery);
                }
                for source_id in source_ids {
                    validate_reference_id(source_id, "src_")?;
                }
                for suggestion in mark_suggestions {
                    validate_reference_id(&suggestion.source_id, "src_")?;
                    validate_visible_text(&suggestion.reason, 256)?;
                }
                (
                    schema_version,
                    request_id,
                    selected_skill_ids,
                    visible_reason,
                )
            }
            Self::SubmitCanonProposal {
                schema_version,
                request_id,
                selected_skill_ids,
                visible_reason,
                display_name,
                proposal_brief,
                source_ids,
                ..
            } => {
                if let Some(display_name) = display_name {
                    validate_visible_text(display_name, 256)?;
                }
                validate_visible_text(proposal_brief, 2_048)?;
                if source_ids.len() > 48 {
                    return Err(AgentV2ContractError::InvalidCanonProposal);
                }
                for source_id in source_ids {
                    validate_reference_id(source_id, "src_")?;
                }
                (
                    schema_version,
                    request_id,
                    selected_skill_ids,
                    visible_reason,
                )
            }
        };
        if *schema_version != AGENT_V2_SCHEMA_VERSION {
            return Err(AgentV2ContractError::UnsupportedVersion);
        }
        if request_id != expected_request_id {
            return Err(AgentV2ContractError::RequestMismatch);
        }
        validate_skill_ids(skill_ids, true)?;
        if skill_ids
            .iter()
            .any(|skill_id| !selected_skills.contains(skill_id))
        {
            return Err(AgentV2ContractError::SkillSelectionMismatch);
        }
        validate_visible_text(visible_reason, MAX_VISIBLE_REASON_BYTES_V2)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BudgetProfileV2 {
    pub schema_version: u16,
    pub scope: String,
    pub max_provider_calls: u16,
    pub max_total_provider_input_bytes: u64,
    pub max_total_provider_output_bytes: u64,
    pub max_accumulated_context_result_bytes: u64,
    pub max_structured_repair_calls: u16,
    pub max_turn_elapsed_ms: u64,
    pub max_total_provider_input_tokens: u64,
    pub max_total_provider_output_tokens: u64,
    pub max_composition_segments: u16,
    pub max_canon_writes: u16,
}

impl BudgetProfileV2 {
    pub fn gar1_read_only() -> Self {
        Self {
            schema_version: AGENT_V2_SCHEMA_VERSION,
            scope: "gar1_read_only".to_owned(),
            max_provider_calls: 16,
            max_total_provider_input_bytes: 1_572_864,
            max_total_provider_output_bytes: 1_048_576,
            max_accumulated_context_result_bytes: 524_288,
            max_structured_repair_calls: 3,
            max_turn_elapsed_ms: 120_000,
            max_total_provider_input_tokens: 384_000,
            max_total_provider_output_tokens: 256_000,
            max_composition_segments: 0,
            max_canon_writes: 0,
        }
    }

    pub fn gar2_canon() -> Self {
        Self {
            schema_version: AGENT_V2_SCHEMA_VERSION,
            scope: "gar2_canon".to_owned(),
            max_provider_calls: 16,
            max_total_provider_input_bytes: 1_572_864,
            max_total_provider_output_bytes: 1_048_576,
            max_accumulated_context_result_bytes: 524_288,
            max_structured_repair_calls: 3,
            max_turn_elapsed_ms: 120_000,
            max_total_provider_input_tokens: 384_000,
            max_total_provider_output_tokens: 256_000,
            max_composition_segments: 0,
            // One Canon transaction may contain at most eight file mutations.
            max_canon_writes: 8,
        }
    }

    pub fn validate_gar1(&self) -> Result<(), AgentV2ContractError> {
        if self.scope != "gar1_read_only" || self.max_canon_writes != 0 {
            return Err(AgentV2ContractError::InvalidBudgetProfile);
        }
        self.validate_common()
    }

    pub fn validate_gar2(&self) -> Result<(), AgentV2ContractError> {
        if self.scope != "gar2_canon" || self.max_canon_writes != 8 {
            return Err(AgentV2ContractError::InvalidBudgetProfile);
        }
        self.validate_common()
    }

    fn validate_common(&self) -> Result<(), AgentV2ContractError> {
        if self.schema_version != AGENT_V2_SCHEMA_VERSION
            || self.max_provider_calls == 0
            || self.max_total_provider_input_bytes == 0
            || self.max_total_provider_output_bytes == 0
            || self.max_accumulated_context_result_bytes == 0
            || self.max_turn_elapsed_ms == 0
            || self.max_total_provider_input_tokens == 0
            || self.max_total_provider_output_tokens == 0
            || self.max_composition_segments != 0
        {
            return Err(AgentV2ContractError::InvalidBudgetProfile);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TurnBudgetUsageV2 {
    pub provider_calls: u16,
    pub total_provider_input_bytes: u64,
    pub total_provider_output_bytes: u64,
    pub accumulated_context_result_bytes: u64,
    pub structured_repair_calls: u16,
    pub total_provider_input_tokens: u64,
    pub total_provider_output_tokens: u64,
}

#[derive(Debug, Clone)]
pub struct TurnBudgetTrackerV2 {
    profile: BudgetProfileV2,
    usage: TurnBudgetUsageV2,
    canon_writes: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemoryCompactionV1 {
    pub schema_version: u16,
    pub request_id: String,
    pub base_memory_hash: String,
    pub summary_text: String,
    pub compacted_through_turn_id: String,
}

impl MemoryCompactionV1 {
    pub fn validate(
        &self,
        expected_request_id: &str,
        expected_memory_hash: &str,
    ) -> Result<(), AgentV2ContractError> {
        if self.schema_version != 1
            || self.request_id != expected_request_id
            || self.base_memory_hash != expected_memory_hash
            || self.base_memory_hash.len() != 64
            || !self
                .base_memory_hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(AgentV2ContractError::MemoryCompactionMismatch);
        }
        validate_visible_text(&self.summary_text, 16_000)?;
        validate_opaque_id(&self.compacted_through_turn_id, "trn_")
    }
}

impl TurnBudgetTrackerV2 {
    pub fn new(profile: BudgetProfileV2) -> Result<Self, AgentV2ContractError> {
        match profile.scope.as_str() {
            "gar1_read_only" => profile.validate_gar1()?,
            "gar2_canon" => profile.validate_gar2()?,
            _ => return Err(AgentV2ContractError::InvalidBudgetProfile),
        }
        Ok(Self {
            profile,
            usage: TurnBudgetUsageV2::default(),
            canon_writes: 0,
        })
    }

    pub fn usage(&self) -> TurnBudgetUsageV2 {
        self.usage
    }

    pub fn reserve_provider_call(
        &mut self,
        input_bytes: u64,
        estimated_input_tokens: u64,
    ) -> Result<(), AgentV2ContractError> {
        let next_calls = self.usage.provider_calls.saturating_add(1);
        let next_bytes = self
            .usage
            .total_provider_input_bytes
            .saturating_add(input_bytes);
        let next_tokens = self
            .usage
            .total_provider_input_tokens
            .saturating_add(estimated_input_tokens);
        if next_calls > self.profile.max_provider_calls
            || next_bytes > self.profile.max_total_provider_input_bytes
            || next_tokens > self.profile.max_total_provider_input_tokens
        {
            return Err(AgentV2ContractError::BudgetExceeded);
        }
        self.usage.provider_calls = next_calls;
        self.usage.total_provider_input_bytes = next_bytes;
        self.usage.total_provider_input_tokens = next_tokens;
        Ok(())
    }

    pub fn record_provider_output(
        &mut self,
        output_bytes: u64,
        output_tokens: u64,
    ) -> Result<(), AgentV2ContractError> {
        let next_bytes = self
            .usage
            .total_provider_output_bytes
            .saturating_add(output_bytes);
        let next_tokens = self
            .usage
            .total_provider_output_tokens
            .saturating_add(output_tokens);
        if next_bytes > self.profile.max_total_provider_output_bytes
            || next_tokens > self.profile.max_total_provider_output_tokens
        {
            return Err(AgentV2ContractError::BudgetExceeded);
        }
        self.usage.total_provider_output_bytes = next_bytes;
        self.usage.total_provider_output_tokens = next_tokens;
        Ok(())
    }

    pub fn record_context_result(&mut self, bytes: u64) -> Result<(), AgentV2ContractError> {
        let next = self
            .usage
            .accumulated_context_result_bytes
            .saturating_add(bytes);
        if next > self.profile.max_accumulated_context_result_bytes {
            return Err(AgentV2ContractError::BudgetExceeded);
        }
        self.usage.accumulated_context_result_bytes = next;
        Ok(())
    }

    pub fn record_structured_repair(&mut self) -> Result<(), AgentV2ContractError> {
        let next = self.usage.structured_repair_calls.saturating_add(1);
        if next > self.profile.max_structured_repair_calls {
            return Err(AgentV2ContractError::BudgetExceeded);
        }
        self.usage.structured_repair_calls = next;
        Ok(())
    }

    pub fn reserve_canon_writes(&mut self, count: u16) -> Result<(), AgentV2ContractError> {
        if count == 0 {
            return Ok(());
        }
        let next = self.canon_writes.saturating_add(count);
        if next > self.profile.max_canon_writes {
            return Err(AgentV2ContractError::BudgetExceeded);
        }
        self.canon_writes = next;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentV2ContractError {
    UnsupportedVersion,
    InvalidIdentifier,
    RequestMismatch,
    CompletionContractMismatch,
    InvalidSkillSelection,
    SkillSelectionMismatch,
    InvalidContextQuery,
    InvalidVisibleText,
    InvalidBudgetProfile,
    BudgetExceeded,
    MemoryCompactionMismatch,
    InvalidCanonProposal,
}

fn validate_hash(value: &str) -> Result<(), AgentV2ContractError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(AgentV2ContractError::InvalidCanonProposal)
    }
}

fn validate_skill_ids(ids: &[String], allow_empty: bool) -> Result<(), AgentV2ContractError> {
    if (!allow_empty && ids.is_empty()) || ids.len() > MAX_SELECTED_SKILLS_V2 {
        return Err(AgentV2ContractError::InvalidSkillSelection);
    }
    let mut unique = BTreeSet::new();
    for id in ids {
        if !SKILL_IDS.contains(&id.as_str()) || !unique.insert(id) {
            return Err(AgentV2ContractError::InvalidSkillSelection);
        }
    }
    Ok(())
}

fn validate_opaque_id(value: &str, prefix: &str) -> Result<(), AgentV2ContractError> {
    let suffix = value
        .strip_prefix(prefix)
        .ok_or(AgentV2ContractError::InvalidIdentifier)?;
    if suffix.len() < 8
        || suffix.len() > 80
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(AgentV2ContractError::InvalidIdentifier);
    }
    Ok(())
}

fn validate_reference_id(value: &str, prefix: &str) -> Result<(), AgentV2ContractError> {
    validate_opaque_id(value, prefix).map_err(|_| AgentV2ContractError::InvalidContextQuery)
}

fn validate_semantic_entity_id(value: &str) -> Result<(), AgentV2ContractError> {
    if value.len() < 8
        || value.len() > 81
        || !value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphabetic())
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(AgentV2ContractError::InvalidContextQuery);
    }
    Ok(())
}

fn validate_visible_text(value: &str, maximum_bytes: usize) -> Result<(), AgentV2ContractError> {
    if value.trim().is_empty()
        || value.len() > maximum_bytes
        || value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
    {
        return Err(AgentV2ContractError::InvalidVisibleText);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_contract_is_host_fixed_per_mode() {
        let valid = ProviderInvocationV2 {
            request_id: "req_12345678".to_owned(),
            mode: ProviderModeV2::Control,
            completion_contract: CompletionContractV2::SkillSelectionV2,
            budget_lease_id: "bud_12345678".to_owned(),
            cancellation_id: "can_12345678".to_owned(),
        };
        assert_eq!(valid.validate(), Ok(()));
        let mut invalid = valid;
        invalid.completion_contract = CompletionContractV2::ConversationTextV1;
        assert_eq!(
            invalid.validate(),
            Err(AgentV2ContractError::CompletionContractMismatch)
        );
    }

    #[test]
    fn skill_selection_rejects_unknown_duplicate_and_request_drift() {
        let valid = SkillSelectionV2 {
            schema_version: 2,
            request_id: "req_12345678".to_owned(),
            selected_skill_ids: vec!["create-character".to_owned(), "create-setting".to_owned()],
            intent_class: SkillIntentClassV2::ProjectDiscussion,
            visible_reason: "需要结合人物与设定。".to_owned(),
        };
        assert_eq!(valid.validate("req_12345678"), Ok(()));
        assert_eq!(
            valid.validate("req_87654321"),
            Err(AgentV2ContractError::RequestMismatch)
        );
        let mut duplicate = valid;
        duplicate
            .selected_skill_ids
            .push("create-character".to_owned());
        assert_eq!(
            duplicate.validate("req_12345678"),
            Err(AgentV2ContractError::InvalidSkillSelection)
        );
    }

    #[test]
    fn start_response_has_no_body_and_cannot_change_selected_skills() {
        let action = AgentControlActionV2::StartResponse {
            schema_version: 2,
            request_id: "req_12345678".to_owned(),
            selected_skill_ids: vec!["novel-story-telling".to_owned()],
            visible_reason: "直接回复作者。".to_owned(),
            response_brief: "回答作者关于当前章节的问题。".to_owned(),
            source_ids: Vec::new(),
            mark_suggestions: Vec::new(),
        };
        assert_eq!(
            action.validate("req_12345678", &["novel-story-telling".to_owned()]),
            Ok(())
        );
        let serialized = serde_json::to_value(action).unwrap();
        assert!(serialized.get("body").is_none());
        assert!(serialized.get("text").is_none());
    }

    #[test]
    fn gar1_budget_is_cumulative_and_write_disabled() {
        let profile = BudgetProfileV2::gar1_read_only();
        assert_eq!(profile.validate_gar1(), Ok(()));
        assert_eq!(profile.max_composition_segments, 0);
        assert_eq!(profile.max_canon_writes, 0);
        let mut tracker = TurnBudgetTrackerV2::new(profile).unwrap();
        tracker.reserve_provider_call(100, 25).unwrap();
        tracker.record_provider_output(40, 10).unwrap();
        tracker.record_context_result(80).unwrap();
        let usage = tracker.usage();
        assert_eq!(usage.provider_calls, 1);
        assert_eq!(usage.total_provider_input_bytes, 100);
        assert_eq!(usage.total_provider_output_bytes, 40);
        assert_eq!(usage.accumulated_context_result_bytes, 80);
    }

    #[test]
    fn gar2_budget_allows_one_eight_file_transaction_and_rejects_more() {
        let profile = BudgetProfileV2::gar2_canon();
        assert_eq!(profile.validate_gar2(), Ok(()));
        assert_eq!(profile.max_composition_segments, 0);
        assert_eq!(profile.max_canon_writes, 8);
        let mut tracker = TurnBudgetTrackerV2::new(profile).unwrap();
        tracker.reserve_canon_writes(8).unwrap();
        assert_eq!(
            tracker.reserve_canon_writes(1),
            Err(AgentV2ContractError::BudgetExceeded)
        );
    }
}
