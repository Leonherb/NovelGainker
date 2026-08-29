use crate::action_contracts::{
    AgentActionErrorCodeV1, AgentActionPayloadV1, ModelToolRequestV1, ProjectRelativePathV1,
    RegisteredToolIdV1, ToolArgumentsV1, ToolCallIdV1, AGENT_ACTION_SCHEMA_VERSION,
};
use crate::contracts::{ContentHashV1, SkillIdV1, MAX_PROVIDER_REQUEST_BYTES};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKindV1 {
    OpenAi,
    DeepSeek,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderOutputModeV1 {
    NativeToolCalling,
    StrictJsonFallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedAgentActionV1 {
    pub provider: ProviderKindV1,
    pub mode: ProviderOutputModeV1,
    pub payload: AgentActionPayloadV1,
    pub repaired: bool,
    pub attempt_count: u8,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OpenAiEnvelopeV1 {
    schema_version: u16,
    skill_ids: Vec<SkillIdV1>,
    tool_calls: Vec<OpenAiToolCallV1>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OpenAiToolCallV1 {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: OpenAiFunctionV1,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenAiFunctionV1 {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeepSeekEnvelopeV1 {
    schema_version: u16,
    skill_ids: Vec<SkillIdV1>,
    tool_calls: Vec<DeepSeekToolCallV1>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeepSeekToolCallV1 {
    call_id: String,
    name: String,
    arguments: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NativeArgumentsV1 {
    purpose: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    directory: Option<String>,
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    patch: Option<String>,
    #[serde(default)]
    expected_source_hash: Option<String>,
}

impl NativeArgumentsV1 {
    fn into_request(
        self,
        call_id: String,
        tool_name: &str,
    ) -> Result<ModelToolRequestV1, AgentActionErrorCodeV1> {
        let tool_id = RegisteredToolIdV1::parse(tool_name)?;
        let arguments = ToolArgumentsV1 {
            path: self.path.map(ProjectRelativePathV1::parse).transpose()?,
            directory: self
                .directory
                .map(ProjectRelativePathV1::parse)
                .transpose()?,
            query: self.query,
            content: self.content,
            patch: self.patch,
            expected_source_hash: self
                .expected_source_hash
                .map(ContentHashV1::parse)
                .transpose()
                .map_err(|_| AgentActionErrorCodeV1::InvalidPayload)?,
        };
        let request = ModelToolRequestV1 {
            call_id: ToolCallIdV1::parse(call_id)?,
            tool_id,
            arguments,
            purpose: self.purpose,
        };
        request.validate()?;
        Ok(request)
    }
}

pub fn normalize_provider_output(
    provider: ProviderKindV1,
    mode: ProviderOutputModeV1,
    source: &[u8],
) -> Result<NormalizedAgentActionV1, AgentActionErrorCodeV1> {
    if source.len() > MAX_PROVIDER_REQUEST_BYTES as usize {
        return Err(AgentActionErrorCodeV1::ProviderRequestBudgetExceeded);
    }
    let value: Value =
        serde_json::from_slice(source).map_err(|_| AgentActionErrorCodeV1::InvalidPayload)?;
    if contains_forbidden_authority_field(&value) {
        return Err(AgentActionErrorCodeV1::ForbiddenAuthorityField);
    }

    let payload = match mode {
        ProviderOutputModeV1::StrictJsonFallback => {
            serde_json::from_value(value).map_err(|_| AgentActionErrorCodeV1::InvalidPayload)?
        }
        ProviderOutputModeV1::NativeToolCalling => match provider {
            ProviderKindV1::OpenAi => normalize_openai(value)?,
            ProviderKindV1::DeepSeek => normalize_deepseek(value)?,
        },
    };
    payload.validate()?;
    Ok(NormalizedAgentActionV1 {
        provider,
        mode,
        payload,
        repaired: false,
        attempt_count: 1,
    })
}

pub fn normalize_with_single_repair(
    provider: ProviderKindV1,
    first_mode: ProviderOutputModeV1,
    first_source: &[u8],
    repair: Option<(ProviderKindV1, ProviderOutputModeV1, &[u8])>,
) -> Result<NormalizedAgentActionV1, AgentActionErrorCodeV1> {
    if let Ok(normalized) = normalize_provider_output(provider, first_mode, first_source) {
        return Ok(normalized);
    }
    let Some((repair_provider, repair_mode, repair_source)) = repair else {
        return Err(AgentActionErrorCodeV1::RepairExhausted);
    };
    if repair_provider != provider {
        return Err(AgentActionErrorCodeV1::CrossProviderFallbackForbidden);
    }
    let mut normalized = normalize_provider_output(provider, repair_mode, repair_source)
        .map_err(|_| AgentActionErrorCodeV1::RepairExhausted)?;
    normalized.repaired = true;
    normalized.attempt_count = 2;
    Ok(normalized)
}

fn normalize_openai(value: Value) -> Result<AgentActionPayloadV1, AgentActionErrorCodeV1> {
    let envelope: OpenAiEnvelopeV1 =
        serde_json::from_value(value).map_err(|_| AgentActionErrorCodeV1::InvalidPayload)?;
    if envelope.schema_version != AGENT_ACTION_SCHEMA_VERSION {
        return Err(AgentActionErrorCodeV1::UnsupportedVersion);
    }
    let mut requests = Vec::with_capacity(envelope.tool_calls.len());
    for call in envelope.tool_calls {
        if call.call_type != "function" {
            return Err(AgentActionErrorCodeV1::InvalidPayload);
        }
        let arguments: NativeArgumentsV1 = serde_json::from_str(&call.function.arguments)
            .map_err(|_| AgentActionErrorCodeV1::InvalidPayload)?;
        requests.push(arguments.into_request(call.id, &call.function.name)?);
    }
    Ok(AgentActionPayloadV1::RequestTools {
        schema_version: envelope.schema_version,
        skill_ids: envelope.skill_ids,
        tool_requests: requests,
    })
}

fn normalize_deepseek(value: Value) -> Result<AgentActionPayloadV1, AgentActionErrorCodeV1> {
    let envelope: DeepSeekEnvelopeV1 =
        serde_json::from_value(value).map_err(|_| AgentActionErrorCodeV1::InvalidPayload)?;
    if envelope.schema_version != AGENT_ACTION_SCHEMA_VERSION {
        return Err(AgentActionErrorCodeV1::UnsupportedVersion);
    }
    let mut requests = Vec::with_capacity(envelope.tool_calls.len());
    for call in envelope.tool_calls {
        let arguments: NativeArgumentsV1 = serde_json::from_value(call.arguments)
            .map_err(|_| AgentActionErrorCodeV1::InvalidPayload)?;
        requests.push(arguments.into_request(call.call_id, &call.name)?);
    }
    Ok(AgentActionPayloadV1::RequestTools {
        schema_version: envelope.schema_version,
        skill_ids: envelope.skill_ids,
        tool_requests: requests,
    })
}

fn contains_forbidden_authority_field(value: &Value) -> bool {
    const FORBIDDEN: [&str; 8] = [
        "projectId",
        "sessionId",
        "turnId",
        "authorizationPolicy",
        "policyDisposition",
        "writerLease",
        "writerPermission",
        "highRiskConfirmed",
    ];
    match value {
        Value::Object(entries) => entries.iter().any(|(key, child)| {
            FORBIDDEN.contains(&key.as_str()) || contains_forbidden_authority_field(child)
        }),
        Value::Array(values) => values.iter().any(contains_forbidden_authority_field),
        _ => false,
    }
}
