use crate::credential_vault::ProviderId;
use crate::tool_runtime::{
    validate_definitions, ProviderToolCall, ToolDefinition, ToolResult,
    MAX_TOOL_CONTINUATION_BYTES, MAX_TOOL_DEFINITIONS_BYTES,
};
use reqwest::blocking::{Client as BlockingClient, Response as BlockingResponse};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE, RETRY_AFTER};
use reqwest::{redirect, Client as AsyncClient, Response as AsyncResponse, StatusCode, Url};
use serde::Serialize;
use serde_json::{json, Value};
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

pub const PROVIDER_PRESET_VERSION: u16 = 1;
pub const MAX_MESSAGE_COUNT: usize = 64;
pub const MAX_MESSAGE_BYTES: usize = 32 * 1024;
pub const MAX_REQUEST_BYTES: usize = 96 * 1024;
pub const MAX_OUTPUT_TOKENS: u32 = 4_096;
pub const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_EVENT_BYTES: usize = 64 * 1024;
pub const MAX_STREAM_EVENTS: usize = 8_192;
pub const MAX_RETRIES: u8 = 2;
const CONNECT_TIMEOUT_SECONDS: u64 = 15;
#[cfg(test)]
const TEST_REQUEST_TIMEOUT_MILLIS: u64 = 250;
pub const CONVERSATION_SYSTEM_INSTRUCTION: &str = "你是 NovelGainker 的中文写作对话助手。自然回应用户，可以普通交流、共同构思或直接创作；只输出用户可见的自然语言文本。仅当当前请求提供工具且确有必要时才可调用；没有工具时继续正常回复。不得声称已经修改或保存文件。";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderProtocol {
    DeepseekChat,
    OpenaiResponses,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPreset {
    pub provider_id: ProviderId,
    pub preset_version: u16,
    pub base_url: &'static str,
    pub endpoint: &'static str,
    pub protocol: ProviderProtocol,
    pub model: &'static str,
    pub timeout_seconds: u64,
}

impl ProviderPreset {
    pub fn request_url(self) -> String {
        format!("{}{}", self.base_url, self.endpoint)
    }
}

pub const DEEPSEEK_PRESET: ProviderPreset = ProviderPreset {
    provider_id: ProviderId::Deepseek,
    preset_version: PROVIDER_PRESET_VERSION,
    base_url: "https://api.deepseek.com",
    endpoint: "/chat/completions",
    protocol: ProviderProtocol::DeepseekChat,
    model: "deepseek-v4-pro",
    timeout_seconds: 120,
};

pub const OPENAI_PRESET: ProviderPreset = ProviderPreset {
    provider_id: ProviderId::Openai,
    preset_version: PROVIDER_PRESET_VERSION,
    base_url: "https://api.openai.com/v1",
    endpoint: "/responses",
    protocol: ProviderProtocol::OpenaiResponses,
    model: "gpt-5.6-terra",
    timeout_seconds: 120,
};

pub fn preset(provider: ProviderId) -> &'static ProviderPreset {
    match provider {
        ProviderId::Deepseek => &DEEPSEEK_PRESET,
        ProviderId::Openai => &OPENAI_PRESET,
    }
}

pub fn preset_manifest_hash() -> &'static str {
    env!("NOVELGAINKER_PROVIDER_PRESET_SHA256")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

#[derive(Clone, PartialEq, Eq)]
pub struct NormalizedMessage {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Clone, PartialEq, Eq)]
pub enum OpenAiMessageContent {
    OutputText { text: String },
    Refusal { refusal: String },
}

#[derive(Clone, PartialEq, Eq)]
pub enum OpenAiContinuationItem {
    OutputMessage {
        item_id: String,
        status: String,
        role: String,
        content: Vec<OpenAiMessageContent>,
        phase: Option<String>,
    },
    FunctionCall {
        item_id: String,
        call_id: String,
        name: String,
        arguments: String,
        status: String,
        phase: Option<String>,
    },
    EncryptedReasoning {
        item_id: String,
        encrypted_content: String,
        phase: Option<String>,
    },
}

#[derive(Clone, PartialEq, Eq)]
pub enum ProviderContinuation {
    Deepseek {
        assistant_text: String,
        call: ProviderToolCall,
    },
    Openai {
        items: Vec<OpenAiContinuationItem>,
    },
}

#[derive(Clone, PartialEq, Eq)]
pub struct NormalizedRequest {
    pub messages: Vec<NormalizedMessage>,
    pub max_output_tokens: u32,
    pub tool_definitions: Vec<ToolDefinition>,
    pub continuation: Option<ProviderContinuation>,
    pub tool_result: Option<ToolResult>,
}

impl NormalizedRequest {
    pub fn user_text(text: String, max_output_tokens: u32) -> Result<Self, ProviderError> {
        let request = Self {
            messages: vec![
                NormalizedMessage {
                    role: MessageRole::System,
                    content: CONVERSATION_SYSTEM_INSTRUCTION.to_owned(),
                },
                NormalizedMessage {
                    role: MessageRole::User,
                    content: text,
                },
            ],
            max_output_tokens,
            tool_definitions: Vec::new(),
            continuation: None,
            tool_result: None,
        };
        request.validate()?;
        Ok(request)
    }

    pub fn connection_probe() -> Self {
        Self {
            messages: vec![
                NormalizedMessage {
                    role: MessageRole::System,
                    content: CONVERSATION_SYSTEM_INSTRUCTION.to_owned(),
                },
                NormalizedMessage {
                    role: MessageRole::User,
                    content: "仅返回 OK".to_owned(),
                },
            ],
            max_output_tokens: 8,
            tool_definitions: Vec::new(),
            continuation: None,
            tool_result: None,
        }
    }

    pub fn conversation(
        messages: Vec<NormalizedMessage>,
        max_output_tokens: u32,
    ) -> Result<Self, ProviderError> {
        Self::with_system(
            CONVERSATION_SYSTEM_INSTRUCTION.to_owned(),
            messages,
            max_output_tokens,
        )
    }

    pub fn with_system(
        system_instruction: String,
        messages: Vec<NormalizedMessage>,
        max_output_tokens: u32,
    ) -> Result<Self, ProviderError> {
        if system_instruction.is_empty()
            || messages.is_empty()
            || messages
                .iter()
                .any(|message| message.role == MessageRole::System)
            || messages.last().map(|message| message.role) != Some(MessageRole::User)
        {
            return Err(ProviderError::invalid_request());
        }
        let mut normalized = Vec::with_capacity(messages.len() + 1);
        normalized.push(NormalizedMessage {
            role: MessageRole::System,
            content: system_instruction,
        });
        normalized.extend(messages);
        let request = Self {
            messages: normalized,
            max_output_tokens,
            tool_definitions: Vec::new(),
            continuation: None,
            tool_result: None,
        };
        request.validate()?;
        Ok(request)
    }

    pub fn with_tool_definitions(
        mut self,
        definitions: Vec<ToolDefinition>,
    ) -> Result<Self, ProviderError> {
        if definitions.is_empty() {
            return Ok(self);
        }
        validate_definitions(&definitions).map_err(|_| ProviderError::invalid_request())?;
        self.tool_definitions = definitions;
        self.validate()?;
        Ok(self)
    }

    pub fn continue_after_tool(
        &self,
        continuation: ProviderContinuation,
        result: ToolResult,
        max_output_tokens: u32,
    ) -> Result<Self, ProviderError> {
        result
            .validate()
            .map_err(|_| ProviderError::invalid_request())?;
        let call = continuation_tool_call(&continuation)?;
        if call.call_id != result.call_id || call.name != result.name {
            return Err(ProviderError::invalid_request());
        }
        let request = Self {
            messages: self.messages.clone(),
            max_output_tokens,
            tool_definitions: Vec::new(),
            continuation: Some(continuation),
            tool_result: Some(result),
        };
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> Result<(), ProviderError> {
        if self.messages.is_empty()
            || self.messages.len() > MAX_MESSAGE_COUNT
            || self.max_output_tokens == 0
            || self.max_output_tokens > MAX_OUTPUT_TOKENS
            || self.messages[0].role != MessageRole::System
            || self.messages[0].content.trim().is_empty()
            || self
                .messages
                .iter()
                .filter(|message| message.role == MessageRole::System)
                .count()
                != 1
            || validate_definitions(&self.tool_definitions).is_err()
            || self
                .tool_definitions
                .iter()
                .fold(0usize, |total, definition| {
                    total
                        .saturating_add(definition.description.len())
                        .saturating_add(
                            serde_json::to_vec(&definition.input_schema)
                                .map_or(usize::MAX, |bytes| bytes.len()),
                        )
                })
                > MAX_TOOL_DEFINITIONS_BYTES
            || self.continuation.is_some() != self.tool_result.is_some()
            || (self.continuation.is_some() && !self.tool_definitions.is_empty())
            || self
                .continuation
                .as_ref()
                .is_some_and(|continuation| validate_continuation_bounds(continuation).is_err())
        {
            return Err(ProviderError::invalid_request());
        }
        let mut total = 0usize;
        for message in &self.messages {
            let length = message.content.len();
            if length == 0 || length > MAX_MESSAGE_BYTES {
                return Err(ProviderError::invalid_request());
            }
            total = total
                .checked_add(length)
                .ok_or_else(ProviderError::invalid_request)?;
        }
        if total > MAX_REQUEST_BYTES {
            return Err(ProviderError::invalid_request());
        }
        Ok(())
    }

    pub fn validate_for(&self, provider: ProviderId) -> Result<(), ProviderError> {
        self.validate()?;
        match (&self.continuation, provider) {
            (Some(ProviderContinuation::Deepseek { .. }), ProviderId::Deepseek)
            | (Some(ProviderContinuation::Openai { .. }), ProviderId::Openai)
            | (None, _) => Ok(()),
            _ => Err(ProviderError::invalid_request()),
        }
    }
}

fn validate_continuation_bounds(continuation: &ProviderContinuation) -> Result<(), ProviderError> {
    match continuation {
        ProviderContinuation::Deepseek {
            assistant_text,
            call,
        } => {
            if assistant_text.len() > MAX_MESSAGE_BYTES {
                return Err(ProviderError::response_too_large());
            }
            call.validate_envelope()
                .map_err(|_| ProviderError::protocol())?;
            let bytes = serde_json::to_vec(&json!({
                "assistantText": assistant_text,
                "callId": call.call_id,
                "name": call.name,
                "arguments": call.arguments_json
            }))
            .map_err(|_| ProviderError::protocol())?;
            if bytes.len() > MAX_TOOL_CONTINUATION_BYTES {
                return Err(ProviderError::response_too_large());
            }
        }
        ProviderContinuation::Openai { items } => {
            if items.is_empty() {
                return Err(ProviderError::protocol());
            }
            let mut total = 0usize;
            for item in items {
                let bytes = serde_json::to_vec(&openai_continuation_value(item))
                    .map_err(|_| ProviderError::protocol())?;
                let item_limit = match item {
                    OpenAiContinuationItem::FunctionCall { .. } => MAX_TOOL_CONTINUATION_BYTES,
                    _ => 32 * 1024,
                };
                if bytes.len() > item_limit {
                    return Err(ProviderError::response_too_large());
                }
                total = total.saturating_add(bytes.len());
                if total > MAX_TOOL_CONTINUATION_BYTES {
                    return Err(ProviderError::response_too_large());
                }
            }
            continuation_tool_call(continuation)?;
        }
    }
    Ok(())
}

fn continuation_tool_call(
    continuation: &ProviderContinuation,
) -> Result<ProviderToolCall, ProviderError> {
    match continuation {
        ProviderContinuation::Deepseek { call, .. } => Ok(call.clone()),
        ProviderContinuation::Openai { items } => {
            let mut calls = items.iter().filter_map(|item| match item {
                OpenAiContinuationItem::FunctionCall {
                    call_id,
                    name,
                    arguments,
                    ..
                } => Some((call_id, name, arguments)),
                _ => None,
            });
            let (call_id, name, arguments) = calls.next().ok_or_else(ProviderError::protocol)?;
            if calls.next().is_some() {
                return Err(ProviderError::protocol());
            }
            let call = ProviderToolCall {
                call_id: call_id.clone(),
                name: name.clone(),
                arguments_json: arguments.clone(),
            };
            call.validate_envelope()
                .map_err(|_| ProviderError::protocol())?;
            Ok(call)
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FinishReason {
    Stop,
    Length,
    ContentFilter,
    ToolCall,
    ResourceLimit,
    Unknown,
}

#[derive(Clone, PartialEq, Eq)]
pub struct NormalizedResponse {
    pub text: String,
    pub usage: Option<Usage>,
    pub finish_reason: FinishReason,
    pub provider_id: ProviderId,
    pub model: &'static str,
    pub tool_call: Option<ProviderToolCall>,
    pub continuation: Option<ProviderContinuation>,
}

impl std::fmt::Debug for NormalizedResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NormalizedResponse")
            .field("text_bytes", &self.text.len())
            .field("usage", &self.usage)
            .field("finish_reason", &self.finish_reason)
            .field("provider_id", &self.provider_id)
            .field("model", &self.model)
            .field("has_tool_call", &self.tool_call.is_some())
            .field("has_continuation", &self.continuation.is_some())
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum NormalizedEvent {
    TextDelta {
        sequence: u32,
        delta: String,
    },
    Usage {
        sequence: u32,
        usage: Usage,
    },
    Completed {
        sequence: u32,
        finish_reason: FinishReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderErrorKind {
    InvalidRequest,
    Authentication,
    RateLimited,
    Timeout,
    Unreachable,
    ModelUnavailable,
    Protocol,
    ResponseTooLarge,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderError {
    pub kind: ProviderErrorKind,
    pub code: &'static str,
    pub retryable: bool,
}

impl ProviderError {
    pub const fn invalid_request() -> Self {
        Self::new(
            ProviderErrorKind::InvalidRequest,
            "provider.invalid_request",
            false,
        )
    }

    pub const fn authentication() -> Self {
        Self::new(
            ProviderErrorKind::Authentication,
            "provider.authentication_failed",
            false,
        )
    }

    pub const fn rate_limited() -> Self {
        Self::new(
            ProviderErrorKind::RateLimited,
            "provider.rate_limited",
            true,
        )
    }

    pub const fn timeout() -> Self {
        Self::new(ProviderErrorKind::Timeout, "provider.timeout", true)
    }

    pub const fn unreachable() -> Self {
        Self::new(ProviderErrorKind::Unreachable, "provider.unreachable", true)
    }

    pub const fn model_unavailable() -> Self {
        Self::new(
            ProviderErrorKind::ModelUnavailable,
            "provider.model_unavailable",
            false,
        )
    }

    pub const fn protocol() -> Self {
        Self::new(ProviderErrorKind::Protocol, "provider.protocol", false)
    }

    pub const fn agent_tool_round_limit() -> Self {
        Self::new(ProviderErrorKind::Protocol, "agent.tool_round_limit", false)
    }

    pub const fn agent_tool_unknown() -> Self {
        Self::new(ProviderErrorKind::Protocol, "agent.tool_unknown", false)
    }

    pub const fn response_too_large() -> Self {
        Self::new(
            ProviderErrorKind::ResponseTooLarge,
            "provider.response_too_large",
            false,
        )
    }

    pub const fn cancelled() -> Self {
        Self::new(ProviderErrorKind::Cancelled, "provider.cancelled", false)
    }

    const fn new(kind: ProviderErrorKind, code: &'static str, retryable: bool) -> Self {
        Self {
            kind,
            code,
            retryable,
        }
    }
}

#[derive(Clone)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
    signal: watch::Sender<bool>,
}

impl Default for CancellationToken {
    fn default() -> Self {
        let (signal, _) = watch::channel(false);
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            signal,
        }
    }
}

impl CancellationToken {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.signal.send_replace(true);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub async fn cancelled(&self) {
        if self.is_cancelled() {
            return;
        }
        let mut receiver = self.signal.subscribe();
        loop {
            if *receiver.borrow() {
                return;
            }
            if receiver.changed().await.is_err() {
                return;
            }
        }
    }
}

enum AdapterStreamEvent {
    Delta(String),
    ToolCallDelta {
        index: u32,
        call_id: Option<String>,
        name: Option<String>,
        arguments_delta: String,
    },
    ToolEnvelope {
        call: ProviderToolCall,
        continuation: ProviderContinuation,
    },
    Usage(Usage),
    Finished(FinishReason),
    Done,
}

#[derive(Default)]
struct ToolCallAccumulator {
    index: Option<u32>,
    call_id: Option<String>,
    name: Option<String>,
    arguments: String,
    envelope: Option<(ProviderToolCall, ProviderContinuation)>,
}

impl ToolCallAccumulator {
    fn push_delta(
        &mut self,
        index: u32,
        call_id: Option<String>,
        name: Option<String>,
        arguments_delta: String,
    ) -> Result<(), ProviderError> {
        if self.envelope.is_some()
            || self.index.is_some_and(|current| current != index)
            || index != 0
            || call_id
                .as_ref()
                .is_some_and(|next| self.call_id.as_ref().is_some_and(|current| current != next))
            || name
                .as_ref()
                .is_some_and(|next| self.name.as_ref().is_some_and(|current| current != next))
            || self.arguments.len().saturating_add(arguments_delta.len())
                > MAX_TOOL_CONTINUATION_BYTES
        {
            return Err(ProviderError::protocol());
        }
        self.index = Some(index);
        if call_id.is_some() {
            self.call_id = call_id;
        }
        if name.is_some() {
            self.name = name;
        }
        self.arguments.push_str(&arguments_delta);
        Ok(())
    }

    fn set_envelope(
        &mut self,
        call: ProviderToolCall,
        continuation: ProviderContinuation,
    ) -> Result<(), ProviderError> {
        if self.index.is_some() || self.envelope.is_some() {
            return Err(ProviderError::protocol());
        }
        self.envelope = Some((call, continuation));
        Ok(())
    }

    fn finish(
        self,
        provider: ProviderId,
        assistant_text: &str,
    ) -> Result<(Option<ProviderToolCall>, Option<ProviderContinuation>), ProviderError> {
        if let Some((call, continuation)) = self.envelope {
            call.validate_envelope()
                .map_err(|_| ProviderError::protocol())?;
            validate_continuation_bounds(&continuation)?;
            return Ok((Some(call), Some(continuation)));
        }
        if self.index.is_none() {
            return Ok((None, None));
        }
        let call = ProviderToolCall {
            call_id: self.call_id.ok_or_else(ProviderError::protocol)?,
            name: self.name.ok_or_else(ProviderError::protocol)?,
            arguments_json: self.arguments,
        };
        call.validate_envelope()
            .map_err(|_| ProviderError::protocol())?;
        if provider != ProviderId::Deepseek {
            return Err(ProviderError::protocol());
        }
        let continuation = ProviderContinuation::Deepseek {
            assistant_text: assistant_text.to_owned(),
            call: call.clone(),
        };
        validate_continuation_bounds(&continuation)?;
        Ok((Some(call), Some(continuation)))
    }
}

trait ProviderAdapter: Sync {
    fn preset(&self) -> &'static ProviderPreset;
    fn streaming_body(&self, request: &NormalizedRequest) -> Value;
    fn completion_body(&self, request: &NormalizedRequest) -> Value;
    fn parse_stream_data(&self, data: &str) -> Result<Vec<AdapterStreamEvent>, ProviderError>;
    fn parse_completion(&self, value: &Value) -> Result<NormalizedResponse, ProviderError>;
}

struct DeepseekAdapter;
struct OpenaiAdapter;

static DEEPSEEK_ADAPTER: DeepseekAdapter = DeepseekAdapter;
static OPENAI_ADAPTER: OpenaiAdapter = OpenaiAdapter;

fn adapter(provider: ProviderId) -> &'static dyn ProviderAdapter {
    match provider {
        ProviderId::Deepseek => &DEEPSEEK_ADAPTER,
        ProviderId::Openai => &OPENAI_ADAPTER,
    }
}

fn message_values(request: &NormalizedRequest) -> Vec<Value> {
    request
        .messages
        .iter()
        .map(|message| {
            json!({
                "role": match message.role {
                    MessageRole::System => "system",
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                },
                "content": message.content,
            })
        })
        .collect()
}

fn deepseek_messages(request: &NormalizedRequest) -> Result<Vec<Value>, ProviderError> {
    let mut messages = message_values(request);
    if let (
        Some(ProviderContinuation::Deepseek {
            assistant_text,
            call,
        }),
        Some(result),
    ) = (&request.continuation, &request.tool_result)
    {
        messages.push(json!({
            "role": "assistant",
            "content": assistant_text,
            "tool_calls": [{
                "id": call.call_id,
                "type": "function",
                "function": {
                    "name": call.name,
                    "arguments": call.arguments_json
                }
            }]
        }));
        messages.push(json!({
            "role": "tool",
            "tool_call_id": result.call_id,
            "content": serde_json::to_string(&result.provider_payload())
                .map_err(|_| ProviderError::protocol())?
        }));
    } else if request.continuation.is_some() {
        return Err(ProviderError::invalid_request());
    }
    Ok(messages)
}

fn deepseek_tool_values(definitions: &[ToolDefinition]) -> Vec<Value> {
    definitions
        .iter()
        .map(|definition| {
            json!({
                "type": "function",
                "function": {
                    "name": definition.name,
                    "description": definition.description,
                    "parameters": definition.input_schema
                }
            })
        })
        .collect()
}

fn openai_tool_values(definitions: &[ToolDefinition]) -> Vec<Value> {
    definitions
        .iter()
        .map(|definition| {
            json!({
                "type": "function",
                "name": definition.name,
                "description": definition.description,
                "parameters": definition.input_schema,
                "strict": true
            })
        })
        .collect()
}

fn openai_continuation_value(item: &OpenAiContinuationItem) -> Value {
    match item {
        OpenAiContinuationItem::OutputMessage {
            item_id,
            status,
            role,
            content,
            phase,
        } => {
            let mut value = json!({
                "type": "message",
                "id": item_id,
                "status": status,
                "role": role,
                "content": content.iter().map(|part| match part {
                    OpenAiMessageContent::OutputText { text } =>
                        json!({"type": "output_text", "text": text}),
                    OpenAiMessageContent::Refusal { refusal } =>
                        json!({"type": "refusal", "refusal": refusal}),
                }).collect::<Vec<_>>()
            });
            if let Some(phase) = phase {
                value["phase"] = Value::String(phase.clone());
            }
            value
        }
        OpenAiContinuationItem::FunctionCall {
            item_id,
            call_id,
            name,
            arguments,
            status,
            phase,
        } => {
            let mut value = json!({
                "type": "function_call",
                "id": item_id,
                "call_id": call_id,
                "name": name,
                "arguments": arguments,
                "status": status
            });
            if let Some(phase) = phase {
                value["phase"] = Value::String(phase.clone());
            }
            value
        }
        OpenAiContinuationItem::EncryptedReasoning {
            item_id,
            encrypted_content,
            phase,
        } => {
            let mut value = json!({
                "type": "reasoning",
                "id": item_id,
                "encrypted_content": encrypted_content
            });
            if let Some(phase) = phase {
                value["phase"] = Value::String(phase.clone());
            }
            value
        }
    }
}

fn openai_input(request: &NormalizedRequest) -> Result<Vec<Value>, ProviderError> {
    let mut input = message_values(request)
        .into_iter()
        .skip(1)
        .collect::<Vec<_>>();
    if let (Some(ProviderContinuation::Openai { items }), Some(result)) =
        (&request.continuation, &request.tool_result)
    {
        input.extend(items.iter().map(openai_continuation_value));
        input.push(json!({
            "type": "function_call_output",
            "call_id": result.call_id,
            "output": serde_json::to_string(&result.provider_payload())
                .map_err(|_| ProviderError::protocol())?
        }));
    } else if request.continuation.is_some() {
        return Err(ProviderError::invalid_request());
    }
    Ok(input)
}

impl ProviderAdapter for DeepseekAdapter {
    fn preset(&self) -> &'static ProviderPreset {
        &DEEPSEEK_PRESET
    }

    fn streaming_body(&self, request: &NormalizedRequest) -> Value {
        let mut body = json!({
            "model": self.preset().model,
            "messages": deepseek_messages(request).unwrap_or_default(),
            "max_tokens": request.max_output_tokens,
            "stream": true,
            "stream_options": { "include_usage": true }
        });
        if !request.tool_definitions.is_empty() {
            body["tools"] = Value::Array(deepseek_tool_values(&request.tool_definitions));
            body["tool_choice"] = Value::String("auto".to_owned());
            body["thinking"] = json!({"type": "disabled"});
        } else if request.continuation.is_some() {
            body["tool_choice"] = Value::String("none".to_owned());
            body["thinking"] = json!({"type": "disabled"});
        }
        body
    }

    fn completion_body(&self, request: &NormalizedRequest) -> Value {
        let mut body = self.streaming_body(request);
        body["stream"] = Value::Bool(false);
        body.as_object_mut()
            .map(|object| object.remove("stream_options"));
        body
    }

    fn parse_stream_data(&self, data: &str) -> Result<Vec<AdapterStreamEvent>, ProviderError> {
        if data == "[DONE]" {
            return Ok(vec![AdapterStreamEvent::Done]);
        }
        let value: Value = serde_json::from_str(data).map_err(|_| ProviderError::protocol())?;
        if value
            .get("model")
            .and_then(Value::as_str)
            .is_some_and(|model| model != self.preset().model)
        {
            return Err(ProviderError::model_unavailable());
        }

        let mut events = Vec::new();
        if let Some(usage) = value.get("usage").and_then(parse_deepseek_usage) {
            events.push(AdapterStreamEvent::Usage(usage));
        }
        if let Some(choice) = value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
        {
            if let Some(delta) = choice
                .pointer("/delta/content")
                .and_then(Value::as_str)
                .filter(|delta| !delta.is_empty())
            {
                events.push(AdapterStreamEvent::Delta(delta.to_owned()));
            }
            if let Some(tool_calls) = choice
                .pointer("/delta/tool_calls")
                .and_then(Value::as_array)
            {
                if tool_calls.len() != 1 {
                    return Err(ProviderError::protocol());
                }
                let tool_call = &tool_calls[0];
                let index = tool_call
                    .get("index")
                    .and_then(Value::as_u64)
                    .and_then(|value| u32::try_from(value).ok())
                    .ok_or_else(ProviderError::protocol)?;
                events.push(AdapterStreamEvent::ToolCallDelta {
                    index,
                    call_id: tool_call
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    name: tool_call
                        .pointer("/function/name")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    arguments_delta: tool_call
                        .pointer("/function/arguments")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                });
            }
            if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
                events.push(AdapterStreamEvent::Finished(map_finish_reason(reason)));
            }
        }
        Ok(events)
    }

    fn parse_completion(&self, value: &Value) -> Result<NormalizedResponse, ProviderError> {
        if value.get("model").and_then(Value::as_str) != Some(self.preset().model) {
            return Err(ProviderError::model_unavailable());
        }
        let choice = value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .ok_or_else(ProviderError::protocol)?;
        let text = choice
            .pointer("/message/content")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let finish_reason = choice
            .get("finish_reason")
            .and_then(Value::as_str)
            .map(map_finish_reason)
            .unwrap_or(FinishReason::Unknown);
        let tool_call = parse_deepseek_tool_call(choice.pointer("/message/tool_calls"))?;
        if (finish_reason == FinishReason::ToolCall) != tool_call.is_some() {
            return Err(ProviderError::protocol());
        }
        let continuation = tool_call
            .as_ref()
            .map(|call| ProviderContinuation::Deepseek {
                assistant_text: text.to_owned(),
                call: call.clone(),
            });
        if let Some(continuation) = &continuation {
            validate_continuation_bounds(continuation)?;
        }
        Ok(NormalizedResponse {
            text: text.to_owned(),
            usage: value.get("usage").and_then(parse_deepseek_usage),
            finish_reason,
            provider_id: ProviderId::Deepseek,
            model: self.preset().model,
            tool_call,
            continuation,
        })
    }
}

impl ProviderAdapter for OpenaiAdapter {
    fn preset(&self) -> &'static ProviderPreset {
        &OPENAI_PRESET
    }

    fn streaming_body(&self, request: &NormalizedRequest) -> Value {
        let mut body = json!({
            "model": self.preset().model,
            "instructions": request.messages[0].content,
            "input": openai_input(request).unwrap_or_default(),
            "max_output_tokens": request.max_output_tokens,
            "store": false,
            "stream": true
        });
        if !request.tool_definitions.is_empty() {
            body["tools"] = Value::Array(openai_tool_values(&request.tool_definitions));
            body["tool_choice"] = Value::String("auto".to_owned());
            body["parallel_tool_calls"] = Value::Bool(false);
            body["include"] = json!(["reasoning.encrypted_content"]);
        } else if request.continuation.is_some() {
            body["tool_choice"] = Value::String("none".to_owned());
        }
        body
    }

    fn completion_body(&self, request: &NormalizedRequest) -> Value {
        let mut body = self.streaming_body(request);
        body["stream"] = Value::Bool(false);
        body
    }

    fn parse_stream_data(&self, data: &str) -> Result<Vec<AdapterStreamEvent>, ProviderError> {
        let value: Value = serde_json::from_str(data).map_err(|_| ProviderError::protocol())?;
        let event_type = value
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(ProviderError::protocol)?;
        let events = match event_type {
            "response.output_text.delta" | "response.refusal.delta" => {
                let delta = value
                    .get("delta")
                    .and_then(Value::as_str)
                    .ok_or_else(ProviderError::protocol)?;
                if delta.is_empty() {
                    Vec::new()
                } else {
                    vec![AdapterStreamEvent::Delta(delta.to_owned())]
                }
            }
            "response.completed" => {
                let response = value.get("response").ok_or_else(ProviderError::protocol)?;
                if response.get("model").and_then(Value::as_str) != Some(self.preset().model) {
                    return Err(ProviderError::model_unavailable());
                }
                let mut events = Vec::new();
                if let Some(usage) = response.get("usage").and_then(parse_openai_usage) {
                    events.push(AdapterStreamEvent::Usage(usage));
                }
                let (_, tool_call, continuation) = parse_openai_output(response)?;
                if let (Some(call), Some(continuation)) = (tool_call, continuation) {
                    events.push(AdapterStreamEvent::ToolEnvelope { call, continuation });
                    events.push(AdapterStreamEvent::Finished(FinishReason::ToolCall));
                } else {
                    events.push(AdapterStreamEvent::Finished(FinishReason::Stop));
                }
                events.push(AdapterStreamEvent::Done);
                events
            }
            "response.incomplete" => vec![
                AdapterStreamEvent::Finished(FinishReason::Length),
                AdapterStreamEvent::Done,
            ],
            "response.failed" | "error" => return Err(ProviderError::protocol()),
            _ => Vec::new(),
        };
        Ok(events)
    }

    fn parse_completion(&self, value: &Value) -> Result<NormalizedResponse, ProviderError> {
        if value.get("model").and_then(Value::as_str) != Some(self.preset().model) {
            return Err(ProviderError::model_unavailable());
        }
        let (text, tool_call, continuation) = parse_openai_output(value)?;
        let finish_reason = match (
            value.get("status").and_then(Value::as_str),
            tool_call.is_some(),
        ) {
            (Some("completed"), true) => FinishReason::ToolCall,
            (Some("completed"), false) => FinishReason::Stop,
            (Some("incomplete"), _) => FinishReason::Length,
            _ => FinishReason::Unknown,
        };
        Ok(NormalizedResponse {
            text,
            usage: value.get("usage").and_then(parse_openai_usage),
            finish_reason,
            provider_id: ProviderId::Openai,
            model: self.preset().model,
            tool_call,
            continuation,
        })
    }
}

fn parse_deepseek_usage(value: &Value) -> Option<Usage> {
    Some(Usage {
        input_tokens: value.get("prompt_tokens")?.as_u64()?,
        output_tokens: value.get("completion_tokens")?.as_u64()?,
        total_tokens: value.get("total_tokens")?.as_u64()?,
    })
}

fn parse_openai_usage(value: &Value) -> Option<Usage> {
    Some(Usage {
        input_tokens: value.get("input_tokens")?.as_u64()?,
        output_tokens: value.get("output_tokens")?.as_u64()?,
        total_tokens: value.get("total_tokens")?.as_u64()?,
    })
}

fn parse_openai_output(
    response: &Value,
) -> Result<
    (
        String,
        Option<ProviderToolCall>,
        Option<ProviderContinuation>,
    ),
    ProviderError,
> {
    let Some(output) = response.get("output").and_then(Value::as_array) else {
        return Ok((String::new(), None, None));
    };
    let mut text = String::new();
    for item in output {
        if item.get("type").and_then(Value::as_str) == Some("message") {
            for part in item
                .get("content")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                match part.get("type").and_then(Value::as_str) {
                    Some("output_text") => text.push_str(
                        part.get("text")
                            .and_then(Value::as_str)
                            .ok_or_else(ProviderError::protocol)?,
                    ),
                    Some("refusal") => text.push_str(
                        part.get("refusal")
                            .and_then(Value::as_str)
                            .ok_or_else(ProviderError::protocol)?,
                    ),
                    _ => {}
                }
            }
        }
    }
    let function_calls = output
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("function_call"))
        .collect::<Vec<_>>();
    if function_calls.is_empty() {
        return Ok((text, None, None));
    }
    if function_calls.len() != 1 {
        return Err(ProviderError::protocol());
    }

    let mut items = Vec::with_capacity(output.len());
    let mut total_bytes = 0usize;
    for item in output {
        let item_bytes = serde_json::to_vec(item)
            .map_err(|_| ProviderError::protocol())?
            .len();
        let item_limit = if item.get("type").and_then(Value::as_str) == Some("function_call") {
            MAX_TOOL_CONTINUATION_BYTES
        } else {
            32 * 1024
        };
        if item_bytes > item_limit {
            return Err(ProviderError::response_too_large());
        }
        total_bytes = total_bytes.saturating_add(item_bytes);
        if total_bytes > 48 * 1024 {
            return Err(ProviderError::response_too_large());
        }
        let item_id = item
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(ProviderError::protocol)?
            .to_owned();
        let phase = parse_optional_phase(item)?;
        match item.get("type").and_then(Value::as_str) {
            Some("message") => {
                let content = item
                    .get("content")
                    .and_then(Value::as_array)
                    .ok_or_else(ProviderError::protocol)?
                    .iter()
                    .map(|part| match part.get("type").and_then(Value::as_str) {
                        Some("output_text") => Ok(OpenAiMessageContent::OutputText {
                            text: part
                                .get("text")
                                .and_then(Value::as_str)
                                .ok_or_else(ProviderError::protocol)?
                                .to_owned(),
                        }),
                        Some("refusal") => Ok(OpenAiMessageContent::Refusal {
                            refusal: part
                                .get("refusal")
                                .and_then(Value::as_str)
                                .ok_or_else(ProviderError::protocol)?
                                .to_owned(),
                        }),
                        _ => Err(ProviderError::protocol()),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                items.push(OpenAiContinuationItem::OutputMessage {
                    item_id,
                    status: required_string(item, "status")?,
                    role: required_string(item, "role")?,
                    content,
                    phase,
                });
            }
            Some("function_call") => {
                let arguments = required_raw_string(item, "arguments")?;
                let call = ProviderToolCall {
                    call_id: required_string(item, "call_id")?,
                    name: required_string(item, "name")?,
                    arguments_json: arguments.clone(),
                };
                call.validate_envelope()
                    .map_err(|_| ProviderError::protocol())?;
                if item.get("status").and_then(Value::as_str) != Some("completed") {
                    return Err(ProviderError::protocol());
                }
                items.push(OpenAiContinuationItem::FunctionCall {
                    item_id,
                    call_id: call.call_id,
                    name: call.name,
                    arguments,
                    status: required_string(item, "status")?,
                    phase,
                });
            }
            Some("reasoning") => {
                items.push(OpenAiContinuationItem::EncryptedReasoning {
                    item_id,
                    encrypted_content: required_string(item, "encrypted_content")?,
                    phase,
                });
            }
            _ => return Err(ProviderError::protocol()),
        }
    }
    let call = continuation_tool_call(&ProviderContinuation::Openai {
        items: items.clone(),
    })?;
    Ok((
        text,
        Some(call),
        Some(ProviderContinuation::Openai { items }),
    ))
}

fn required_string(value: &Value, key: &str) -> Result<String, ProviderError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(ProviderError::protocol)
}

fn required_raw_string(value: &Value, key: &str) -> Result<String, ProviderError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(ProviderError::protocol)
}

fn parse_optional_phase(value: &Value) -> Result<Option<String>, ProviderError> {
    match value.get("phase") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(phase)) if !phase.is_empty() && phase.len() <= 64 => {
            Ok(Some(phase.clone()))
        }
        _ => Err(ProviderError::protocol()),
    }
}

fn parse_deepseek_tool_call(
    value: Option<&Value>,
) -> Result<Option<ProviderToolCall>, ProviderError> {
    let Some(calls) = value.and_then(Value::as_array) else {
        return Ok(None);
    };
    if calls.len() != 1 {
        return Err(ProviderError::protocol());
    }
    let call = &calls[0];
    if call.get("type").and_then(Value::as_str) != Some("function") {
        return Err(ProviderError::protocol());
    }
    let call = ProviderToolCall {
        call_id: call
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(ProviderError::protocol)?
            .to_owned(),
        name: call
            .pointer("/function/name")
            .and_then(Value::as_str)
            .ok_or_else(ProviderError::protocol)?
            .to_owned(),
        arguments_json: call
            .pointer("/function/arguments")
            .and_then(Value::as_str)
            .ok_or_else(ProviderError::protocol)?
            .to_owned(),
    };
    call.validate_envelope()
        .map_err(|_| ProviderError::protocol())?;
    Ok(Some(call))
}

fn map_finish_reason(reason: &str) -> FinishReason {
    match reason {
        "stop" => FinishReason::Stop,
        "length" => FinishReason::Length,
        "content_filter" => FinishReason::ContentFilter,
        "tool_calls" => FinishReason::ToolCall,
        "insufficient_system_resource" => FinishReason::ResourceLimit,
        _ => FinishReason::Unknown,
    }
}

pub struct ProviderHttpClient {
    blocking_client: BlockingClient,
    async_client: AsyncClient,
    #[cfg(test)]
    test_endpoint: Option<(ProviderId, String)>,
}

impl ProviderHttpClient {
    pub fn production() -> Result<Self, ProviderError> {
        validate_preset(&DEEPSEEK_PRESET)?;
        validate_preset(&OPENAI_PRESET)?;
        let blocking_client = BlockingClient::builder()
            .redirect(redirect::Policy::none())
            .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECONDS))
            .timeout(Duration::from_secs(DEEPSEEK_PRESET.timeout_seconds))
            .user_agent("NovelGainker/0.1")
            .build()
            .map_err(|_| ProviderError::unreachable())?;
        let async_client = AsyncClient::builder()
            .redirect(redirect::Policy::none())
            .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECONDS))
            .timeout(Duration::from_secs(DEEPSEEK_PRESET.timeout_seconds))
            .user_agent("NovelGainker/0.1")
            .build()
            .map_err(|_| ProviderError::unreachable())?;
        Ok(Self {
            blocking_client,
            async_client,
            #[cfg(test)]
            test_endpoint: None,
        })
    }

    #[cfg(test)]
    pub(crate) fn for_test(provider: ProviderId, endpoint: String) -> Result<Self, ProviderError> {
        let url = Url::parse(&endpoint).map_err(|_| ProviderError::invalid_request())?;
        if url.scheme() != "http"
            || !matches!(url.host_str(), Some("127.0.0.1") | Some("localhost"))
            || url.port().is_none()
        {
            return Err(ProviderError::invalid_request());
        }
        let blocking_client = BlockingClient::builder()
            .redirect(redirect::Policy::none())
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_millis(TEST_REQUEST_TIMEOUT_MILLIS))
            .no_proxy()
            .build()
            .map_err(|_| ProviderError::unreachable())?;
        let async_client = AsyncClient::builder()
            .redirect(redirect::Policy::none())
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_millis(TEST_REQUEST_TIMEOUT_MILLIS))
            .no_proxy()
            .build()
            .map_err(|_| ProviderError::unreachable())?;
        Ok(Self {
            blocking_client,
            async_client,
            test_endpoint: Some((provider, endpoint)),
        })
    }

    pub fn complete_once(
        &self,
        provider: ProviderId,
        key: &[u8],
        request: &NormalizedRequest,
        cancellation: &CancellationToken,
    ) -> Result<NormalizedResponse, ProviderError> {
        request.validate_for(provider)?;
        if cancellation.is_cancelled() {
            return Err(ProviderError::cancelled());
        }
        let adapter = adapter(provider);
        let body = serde_json::to_vec(&adapter.completion_body(request))
            .map_err(|_| ProviderError::protocol())?;
        let response = self.send(provider, key, &body, "application/json")?;
        if cancellation.is_cancelled() {
            return Err(ProviderError::cancelled());
        }
        if !response.status().is_success() {
            return Err(map_http_status(response.status()));
        }
        parse_bounded_completion(response, adapter)
    }
    pub fn complete(
        &self,
        provider: ProviderId,
        key: &[u8],
        request: &NormalizedRequest,
        cancellation: &CancellationToken,
    ) -> Result<NormalizedResponse, ProviderError> {
        request.validate_for(provider)?;
        let adapter = adapter(provider);
        let body = serde_json::to_vec(&adapter.completion_body(request))
            .map_err(|_| ProviderError::protocol())?;
        let mut last_error = ProviderError::unreachable();
        for attempt in 0..=MAX_RETRIES {
            if cancellation.is_cancelled() {
                return Err(ProviderError::cancelled());
            }
            match self.send(provider, key, &body, "application/json") {
                Ok(response) if response.status().is_success() => {
                    return parse_bounded_completion(response, adapter);
                }
                Ok(response) => {
                    let retry_after = bounded_retry_after(response.headers());
                    let error = map_http_status(response.status());
                    if !error.retryable || attempt == MAX_RETRIES {
                        return Err(error);
                    }
                    last_error = error;
                    sleep_retry(attempt, retry_after);
                }
                Err(error) => {
                    if !error.retryable || attempt == MAX_RETRIES {
                        return Err(error);
                    }
                    last_error = error;
                    sleep_retry(attempt, None);
                }
            }
        }
        Err(last_error)
    }

    pub async fn complete_cancellable(
        &self,
        provider: ProviderId,
        key: &[u8],
        request: &NormalizedRequest,
        cancellation: &CancellationToken,
    ) -> Result<NormalizedResponse, ProviderError> {
        request.validate_for(provider)?;
        let adapter = adapter(provider);
        let body = serde_json::to_vec(&adapter.completion_body(request))
            .map_err(|_| ProviderError::protocol())?;
        let mut last_error = ProviderError::unreachable();
        for attempt in 0..=MAX_RETRIES {
            if cancellation.is_cancelled() {
                return Err(ProviderError::cancelled());
            }
            let response = tokio::select! {
                _ = cancellation.cancelled() => return Err(ProviderError::cancelled()),
                response = self.send_async(provider, key, &body, "application/json") => response,
            };
            if cancellation.is_cancelled() {
                return Err(ProviderError::cancelled());
            }
            match response {
                Ok(response) if response.status().is_success() => {
                    return parse_bounded_completion_cancellable(response, adapter, cancellation)
                        .await;
                }
                Ok(response) => {
                    let retry_after = bounded_retry_after(response.headers());
                    let error = map_http_status(response.status());
                    if !error.retryable || attempt == MAX_RETRIES {
                        return Err(error);
                    }
                    last_error = error;
                    sleep_retry_cancellable(attempt, retry_after, cancellation).await?;
                }
                Err(error) => {
                    if !error.retryable || attempt == MAX_RETRIES {
                        return Err(error);
                    }
                    last_error = error;
                    sleep_retry_cancellable(attempt, None, cancellation).await?;
                }
            }
        }
        Err(last_error)
    }

    pub async fn complete_once_cancellable(
        &self,
        provider: ProviderId,
        key: &[u8],
        request: &NormalizedRequest,
        cancellation: &CancellationToken,
    ) -> Result<NormalizedResponse, ProviderError> {
        request.validate_for(provider)?;
        if cancellation.is_cancelled() {
            return Err(ProviderError::cancelled());
        }
        let adapter = adapter(provider);
        let body = serde_json::to_vec(&adapter.completion_body(request))
            .map_err(|_| ProviderError::protocol())?;
        let response = tokio::select! {
            _ = cancellation.cancelled() => return Err(ProviderError::cancelled()),
            response = self.send_async(provider, key, &body, "application/json") => response?,
        };
        if !response.status().is_success() {
            return Err(map_http_status(response.status()));
        }
        parse_bounded_completion_cancellable(response, adapter, cancellation).await
    }

    pub fn complete_once_cancellable_blocking(
        &self,
        provider: ProviderId,
        key: &[u8],
        request: &NormalizedRequest,
        cancellation: &CancellationToken,
    ) -> Result<NormalizedResponse, ProviderError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| ProviderError::unreachable())?;
        runtime.block_on(self.complete_once_cancellable(provider, key, request, cancellation))
    }

    pub fn stream(
        &self,
        provider: ProviderId,
        key: &[u8],
        request: &NormalizedRequest,
        cancellation: &CancellationToken,
        sink: &mut dyn FnMut(NormalizedEvent) -> Result<(), ProviderError>,
    ) -> Result<NormalizedResponse, ProviderError> {
        request.validate_for(provider)?;
        let adapter = adapter(provider);
        let body = serde_json::to_vec(&adapter.streaming_body(request))
            .map_err(|_| ProviderError::protocol())?;
        let mut response = None;
        let mut last_error = ProviderError::unreachable();

        for attempt in 0..=MAX_RETRIES {
            if cancellation.is_cancelled() {
                return Err(ProviderError::cancelled());
            }
            match self.send(provider, key, &body, "text/event-stream") {
                Ok(candidate) if candidate.status().is_success() => {
                    response = Some(candidate);
                    break;
                }
                Ok(candidate) => {
                    let retry_after = bounded_retry_after(candidate.headers());
                    let error = map_http_status(candidate.status());
                    if !error.retryable || attempt == MAX_RETRIES {
                        return Err(error);
                    }
                    last_error = error;
                    sleep_retry(attempt, retry_after);
                }
                Err(error) => {
                    if !error.retryable || attempt == MAX_RETRIES {
                        return Err(error);
                    }
                    last_error = error;
                    sleep_retry(attempt, None);
                }
            }
        }

        let response = response.ok_or(last_error)?;
        consume_stream(response, provider, adapter, cancellation, sink)
    }

    pub async fn stream_cancellable<F>(
        &self,
        provider: ProviderId,
        key: &[u8],
        request: &NormalizedRequest,
        cancellation: &CancellationToken,
        sink: &mut F,
    ) -> Result<NormalizedResponse, ProviderError>
    where
        F: FnMut(NormalizedEvent) -> Result<(), ProviderError> + Send,
    {
        request.validate_for(provider)?;
        let adapter = adapter(provider);
        let body = serde_json::to_vec(&adapter.streaming_body(request))
            .map_err(|_| ProviderError::protocol())?;
        let mut response = None;
        let mut last_error = ProviderError::unreachable();

        for attempt in 0..=MAX_RETRIES {
            if cancellation.is_cancelled() {
                return Err(ProviderError::cancelled());
            }
            let candidate = tokio::select! {
                _ = cancellation.cancelled() => return Err(ProviderError::cancelled()),
                candidate = self.send_async(provider, key, &body, "text/event-stream") => candidate,
            };
            match candidate {
                Ok(candidate) if candidate.status().is_success() => {
                    response = Some(candidate);
                    break;
                }
                Ok(candidate) => {
                    let retry_after = bounded_retry_after(candidate.headers());
                    let error = map_http_status(candidate.status());
                    if !error.retryable || attempt == MAX_RETRIES {
                        return Err(error);
                    }
                    last_error = error;
                    sleep_retry_cancellable(attempt, retry_after, cancellation).await?;
                }
                Err(error) => {
                    if !error.retryable || attempt == MAX_RETRIES {
                        return Err(error);
                    }
                    last_error = error;
                    sleep_retry_cancellable(attempt, None, cancellation).await?;
                }
            }
        }

        consume_stream_cancellable(
            response.ok_or(last_error)?,
            provider,
            adapter,
            cancellation,
            sink,
        )
        .await
    }

    pub fn stream_cancellable_blocking<F>(
        &self,
        provider: ProviderId,
        key: &[u8],
        request: &NormalizedRequest,
        cancellation: &CancellationToken,
        sink: &mut F,
    ) -> Result<NormalizedResponse, ProviderError>
    where
        F: FnMut(NormalizedEvent) -> Result<(), ProviderError> + Send,
    {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| ProviderError::unreachable())?;
        runtime.block_on(self.stream_cancellable(provider, key, request, cancellation, sink))
    }

    pub async fn stream_once_cancellable<F>(
        &self,
        provider: ProviderId,
        key: &[u8],
        request: &NormalizedRequest,
        cancellation: &CancellationToken,
        sink: &mut F,
    ) -> Result<NormalizedResponse, ProviderError>
    where
        F: FnMut(NormalizedEvent) -> Result<(), ProviderError> + Send,
    {
        request.validate_for(provider)?;
        if cancellation.is_cancelled() {
            return Err(ProviderError::cancelled());
        }
        let adapter = adapter(provider);
        let body = serde_json::to_vec(&adapter.streaming_body(request))
            .map_err(|_| ProviderError::protocol())?;
        let response = tokio::select! {
            _ = cancellation.cancelled() => return Err(ProviderError::cancelled()),
            response = self.send_async(provider, key, &body, "text/event-stream") => response?,
        };
        if !response.status().is_success() {
            return Err(map_http_status(response.status()));
        }
        consume_stream_cancellable(response, provider, adapter, cancellation, sink).await
    }

    pub fn stream_once_cancellable_blocking<F>(
        &self,
        provider: ProviderId,
        key: &[u8],
        request: &NormalizedRequest,
        cancellation: &CancellationToken,
        sink: &mut F,
    ) -> Result<NormalizedResponse, ProviderError>
    where
        F: FnMut(NormalizedEvent) -> Result<(), ProviderError> + Send,
    {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| ProviderError::unreachable())?;
        runtime.block_on(self.stream_once_cancellable(provider, key, request, cancellation, sink))
    }

    fn endpoint(&self, provider: ProviderId) -> Result<String, ProviderError> {
        #[cfg(test)]
        if let Some((allowed_provider, endpoint)) = &self.test_endpoint {
            if *allowed_provider != provider {
                return Err(ProviderError::invalid_request());
            }
            return Ok(endpoint.clone());
        }
        Ok(preset(provider).request_url())
    }

    fn send(
        &self,
        provider: ProviderId,
        key: &[u8],
        body: &[u8],
        accept: &'static str,
    ) -> Result<BlockingResponse, ProviderError> {
        if body.len() > MAX_REQUEST_BYTES {
            return Err(ProviderError::invalid_request());
        }
        let endpoint = self.endpoint(provider)?;
        let mut authorization = Vec::with_capacity(key.len() + 7);
        authorization.extend_from_slice(b"Bearer ");
        authorization.extend_from_slice(key);
        let authorization_result = HeaderValue::from_bytes(&authorization);
        wipe(&mut authorization);
        let mut authorization_value =
            authorization_result.map_err(|_| ProviderError::invalid_request())?;
        authorization_value.set_sensitive(true);

        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, authorization_value);
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(ACCEPT, HeaderValue::from_static(accept));
        self.blocking_client
            .post(endpoint)
            .headers(headers)
            .body(body.to_vec())
            .send()
            .map_err(|error| map_reqwest_error(&error))
    }

    async fn send_async(
        &self,
        provider: ProviderId,
        key: &[u8],
        body: &[u8],
        accept: &'static str,
    ) -> Result<AsyncResponse, ProviderError> {
        if body.len() > MAX_REQUEST_BYTES {
            return Err(ProviderError::invalid_request());
        }
        let endpoint = self.endpoint(provider)?;
        let mut authorization = Vec::with_capacity(key.len() + 7);
        authorization.extend_from_slice(b"Bearer ");
        authorization.extend_from_slice(key);
        let authorization_result = HeaderValue::from_bytes(&authorization);
        wipe(&mut authorization);
        let mut authorization_value =
            authorization_result.map_err(|_| ProviderError::invalid_request())?;
        authorization_value.set_sensitive(true);

        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, authorization_value);
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(ACCEPT, HeaderValue::from_static(accept));
        self.async_client
            .post(endpoint)
            .headers(headers)
            .body(body.to_vec())
            .send()
            .await
            .map_err(|error| map_reqwest_error(&error))
    }
}

fn validate_preset(preset: &ProviderPreset) -> Result<(), ProviderError> {
    let url = Url::parse(&preset.request_url()).map_err(|_| ProviderError::invalid_request())?;
    let expected_host = match preset.provider_id {
        ProviderId::Deepseek => "api.deepseek.com",
        ProviderId::Openai => "api.openai.com",
    };
    if url.scheme() != "https"
        || url.host_str() != Some(expected_host)
        || url.username() != ""
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || preset.preset_version != PROVIDER_PRESET_VERSION
        || preset.timeout_seconds != 120
    {
        return Err(ProviderError::invalid_request());
    }
    Ok(())
}

fn parse_bounded_completion(
    mut response: BlockingResponse,
    adapter: &dyn ProviderAdapter,
) -> Result<NormalizedResponse, ProviderError> {
    let bytes = read_bounded(&mut response)?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| ProviderError::protocol())?;
    adapter.parse_completion(&value)
}

async fn parse_bounded_completion_cancellable(
    response: AsyncResponse,
    adapter: &dyn ProviderAdapter,
    cancellation: &CancellationToken,
) -> Result<NormalizedResponse, ProviderError> {
    let bytes = read_bounded_cancellable(response, cancellation).await?;
    if cancellation.is_cancelled() {
        return Err(ProviderError::cancelled());
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| ProviderError::protocol())?;
    if cancellation.is_cancelled() {
        return Err(ProviderError::cancelled());
    }
    adapter.parse_completion(&value)
}

fn read_bounded(response: &mut BlockingResponse) -> Result<Vec<u8>, ProviderError> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(ProviderError::response_too_large());
    }
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 8 * 1024];
    loop {
        let read = response
            .read(&mut chunk)
            .map_err(|_| ProviderError::unreachable())?;
        if read == 0 {
            break;
        }
        if bytes.len().saturating_add(read) > MAX_RESPONSE_BYTES {
            wipe(&mut bytes);
            return Err(ProviderError::response_too_large());
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
    Ok(bytes)
}

async fn read_bounded_cancellable(
    mut response: AsyncResponse,
    cancellation: &CancellationToken,
) -> Result<Vec<u8>, ProviderError> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(ProviderError::response_too_large());
    }
    let mut bytes = Vec::new();
    loop {
        let chunk = tokio::select! {
            _ = cancellation.cancelled() => {
                wipe(&mut bytes);
                return Err(ProviderError::cancelled());
            }
            chunk = response.chunk() => chunk.map_err(|error| map_reqwest_error(&error))?,
        };
        let Some(chunk) = chunk else {
            break;
        };
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            wipe(&mut bytes);
            return Err(ProviderError::response_too_large());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn consume_stream(
    mut response: BlockingResponse,
    provider: ProviderId,
    adapter: &dyn ProviderAdapter,
    cancellation: &CancellationToken,
    sink: &mut dyn FnMut(NormalizedEvent) -> Result<(), ProviderError>,
) -> Result<NormalizedResponse, ProviderError> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(ProviderError::response_too_large());
    }
    let mut decoder = SseDecoder::default();
    let mut aggregate = String::new();
    let mut usage = None;
    let mut finish_reason = None;
    let mut done = false;
    let mut sequence = 0u32;
    let mut event_count = 0usize;
    let mut tool_state = ToolCallAccumulator::default();
    let mut response_bytes = 0usize;
    let mut chunk = [0u8; 8 * 1024];

    loop {
        if cancellation.is_cancelled() {
            return Err(ProviderError::cancelled());
        }
        let read = response
            .read(&mut chunk)
            .map_err(|_| ProviderError::unreachable())?;
        if read == 0 {
            break;
        }
        response_bytes = response_bytes.saturating_add(read);
        if response_bytes > MAX_RESPONSE_BYTES {
            return Err(ProviderError::response_too_large());
        }
        for data in decoder.push(&chunk[..read])? {
            apply_stream_data(
                adapter,
                &data,
                &mut aggregate,
                &mut usage,
                &mut finish_reason,
                &mut done,
                &mut sequence,
                &mut event_count,
                &mut tool_state,
                sink,
            )?;
        }
        if done {
            break;
        }
    }
    if !done {
        for data in decoder.finish()? {
            apply_stream_data(
                adapter,
                &data,
                &mut aggregate,
                &mut usage,
                &mut finish_reason,
                &mut done,
                &mut sequence,
                &mut event_count,
                &mut tool_state,
                sink,
            )?;
        }
    }
    if cancellation.is_cancelled() {
        return Err(ProviderError::cancelled());
    }
    if !done {
        return Err(ProviderError::protocol());
    }

    let (tool_call, continuation) = tool_state.finish(provider, &aggregate)?;
    if (finish_reason == Some(FinishReason::ToolCall)) != tool_call.is_some() {
        return Err(ProviderError::protocol());
    }
    Ok(NormalizedResponse {
        text: aggregate,
        usage,
        finish_reason: finish_reason.unwrap_or(FinishReason::Unknown),
        provider_id: provider,
        model: preset(provider).model,
        tool_call,
        continuation,
    })
}

async fn consume_stream_cancellable<F>(
    mut response: AsyncResponse,
    provider: ProviderId,
    adapter: &dyn ProviderAdapter,
    cancellation: &CancellationToken,
    sink: &mut F,
) -> Result<NormalizedResponse, ProviderError>
where
    F: FnMut(NormalizedEvent) -> Result<(), ProviderError> + Send,
{
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(ProviderError::response_too_large());
    }
    let mut decoder = SseDecoder::default();
    let mut aggregate = String::new();
    let mut usage = None;
    let mut finish_reason = None;
    let mut done = false;
    let mut sequence = 0u32;
    let mut event_count = 0usize;
    let mut tool_state = ToolCallAccumulator::default();
    let mut response_bytes = 0usize;

    loop {
        let chunk = tokio::select! {
            _ = cancellation.cancelled() => return Err(ProviderError::cancelled()),
            chunk = response.chunk() => chunk.map_err(|error| map_reqwest_error(&error))?,
        };
        let Some(chunk) = chunk else {
            break;
        };
        response_bytes = response_bytes.saturating_add(chunk.len());
        if response_bytes > MAX_RESPONSE_BYTES {
            return Err(ProviderError::response_too_large());
        }
        for data in decoder.push(&chunk)? {
            apply_stream_data(
                adapter,
                &data,
                &mut aggregate,
                &mut usage,
                &mut finish_reason,
                &mut done,
                &mut sequence,
                &mut event_count,
                &mut tool_state,
                sink,
            )?;
        }
        if done {
            break;
        }
    }
    if !done {
        for data in decoder.finish()? {
            apply_stream_data(
                adapter,
                &data,
                &mut aggregate,
                &mut usage,
                &mut finish_reason,
                &mut done,
                &mut sequence,
                &mut event_count,
                &mut tool_state,
                sink,
            )?;
        }
    }
    if cancellation.is_cancelled() {
        return Err(ProviderError::cancelled());
    }
    if !done {
        return Err(ProviderError::protocol());
    }

    let (tool_call, continuation) = tool_state.finish(provider, &aggregate)?;
    if (finish_reason == Some(FinishReason::ToolCall)) != tool_call.is_some() {
        return Err(ProviderError::protocol());
    }
    Ok(NormalizedResponse {
        text: aggregate,
        usage,
        finish_reason: finish_reason.unwrap_or(FinishReason::Unknown),
        provider_id: provider,
        model: preset(provider).model,
        tool_call,
        continuation,
    })
}

#[allow(clippy::too_many_arguments)]
fn apply_stream_data(
    adapter: &dyn ProviderAdapter,
    data: &str,
    aggregate: &mut String,
    usage: &mut Option<Usage>,
    finish_reason: &mut Option<FinishReason>,
    done: &mut bool,
    sequence: &mut u32,
    event_count: &mut usize,
    tool_state: &mut ToolCallAccumulator,
    sink: &mut dyn FnMut(NormalizedEvent) -> Result<(), ProviderError>,
) -> Result<(), ProviderError> {
    for event in adapter.parse_stream_data(data)? {
        *event_count = event_count.saturating_add(1);
        if *event_count > MAX_STREAM_EVENTS {
            return Err(ProviderError::response_too_large());
        }
        match event {
            AdapterStreamEvent::Delta(delta) => {
                if delta.len() > MAX_EVENT_BYTES
                    || aggregate.len().saturating_add(delta.len()) > MAX_RESPONSE_BYTES
                {
                    return Err(ProviderError::response_too_large());
                }
                aggregate.push_str(&delta);
                *sequence = sequence
                    .checked_add(1)
                    .ok_or_else(ProviderError::protocol)?;
                sink(NormalizedEvent::TextDelta {
                    sequence: *sequence,
                    delta,
                })?;
            }
            AdapterStreamEvent::ToolCallDelta {
                index,
                call_id,
                name,
                arguments_delta,
            } => {
                tool_state.push_delta(index, call_id, name, arguments_delta)?;
            }
            AdapterStreamEvent::ToolEnvelope { call, continuation } => {
                tool_state.set_envelope(call, continuation)?;
            }
            AdapterStreamEvent::Usage(next_usage) => {
                *usage = Some(next_usage);
                *sequence = sequence
                    .checked_add(1)
                    .ok_or_else(ProviderError::protocol)?;
                sink(NormalizedEvent::Usage {
                    sequence: *sequence,
                    usage: next_usage,
                })?;
            }
            AdapterStreamEvent::Finished(reason) => {
                *finish_reason = Some(reason);
            }
            AdapterStreamEvent::Done => {
                if *done {
                    return Err(ProviderError::protocol());
                }
                *done = true;
                *sequence = sequence
                    .checked_add(1)
                    .ok_or_else(ProviderError::protocol)?;
                sink(NormalizedEvent::Completed {
                    sequence: *sequence,
                    finish_reason: finish_reason.unwrap_or(FinishReason::Unknown),
                })?;
            }
        }
    }
    Ok(())
}

#[derive(Default)]
struct SseDecoder {
    buffer: Vec<u8>,
    data_lines: Vec<String>,
}

impl SseDecoder {
    fn push(&mut self, bytes: &[u8]) -> Result<Vec<String>, ProviderError> {
        if self.buffer.len().saturating_add(bytes.len()) > MAX_EVENT_BYTES * 2 {
            return Err(ProviderError::response_too_large());
        }
        self.buffer.extend_from_slice(bytes);
        let mut events = Vec::new();
        while let Some(newline) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let mut line = self.buffer.drain(..=newline).collect::<Vec<_>>();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            self.consume_line(&line, &mut events)?;
        }
        Ok(events)
    }

    fn finish(&mut self) -> Result<Vec<String>, ProviderError> {
        let mut events = Vec::new();
        if !self.buffer.is_empty() {
            let line = std::mem::take(&mut self.buffer);
            self.consume_line(&line, &mut events)?;
        }
        if !self.data_lines.is_empty() {
            events.push(self.take_event()?);
        }
        Ok(events)
    }

    fn consume_line(&mut self, line: &[u8], events: &mut Vec<String>) -> Result<(), ProviderError> {
        if line.is_empty() {
            if !self.data_lines.is_empty() {
                events.push(self.take_event()?);
            }
            return Ok(());
        }
        if line.starts_with(b":") {
            return Ok(());
        }
        if let Some(data) = line.strip_prefix(b"data:") {
            let data = data.strip_prefix(b" ").unwrap_or(data);
            let text = std::str::from_utf8(data).map_err(|_| ProviderError::protocol())?;
            if text.len() > MAX_EVENT_BYTES {
                return Err(ProviderError::response_too_large());
            }
            self.data_lines.push(text.to_owned());
        }
        Ok(())
    }

    fn take_event(&mut self) -> Result<String, ProviderError> {
        let event = self.data_lines.join("\n");
        self.data_lines.clear();
        if event.len() > MAX_EVENT_BYTES {
            return Err(ProviderError::response_too_large());
        }
        Ok(event)
    }
}

fn map_http_status(status: StatusCode) -> ProviderError {
    match status.as_u16() {
        401 | 403 => ProviderError::authentication(),
        404 => ProviderError::model_unavailable(),
        408 => ProviderError::timeout(),
        429 => ProviderError::rate_limited(),
        500 | 502 | 503 | 504 => ProviderError::unreachable(),
        _ => ProviderError::protocol(),
    }
}

fn map_reqwest_error(error: &reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        ProviderError::timeout()
    } else if error.is_connect() || error.is_request() {
        ProviderError::unreachable()
    } else {
        ProviderError::protocol()
    }
}

fn bounded_retry_after(headers: &HeaderMap) -> Option<Duration> {
    headers
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(|seconds| Duration::from_secs(seconds.min(2)))
}

fn sleep_retry(attempt: u8, retry_after: Option<Duration>) {
    let fallback = Duration::from_millis(20 * u64::from(attempt + 1));
    std::thread::sleep(retry_after.unwrap_or(fallback));
}

async fn sleep_retry_cancellable(
    attempt: u8,
    retry_after: Option<Duration>,
    cancellation: &CancellationToken,
) -> Result<(), ProviderError> {
    let fallback = Duration::from_millis(20 * u64::from(attempt + 1));
    tokio::select! {
        _ = cancellation.cancelled() => Err(ProviderError::cancelled()),
        _ = tokio::time::sleep(retry_after.unwrap_or(fallback)) => Ok(()),
    }
}

fn wipe(bytes: &mut [u8]) {
    for byte in bytes {
        unsafe { std::ptr::write_volatile(byte, 0) };
    }
    std::sync::atomic::compiler_fence(Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    fn capture_outbound_json(
        provider: ProviderId,
        request: &NormalizedRequest,
        streaming: bool,
    ) -> Value {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut received = Vec::new();
            let mut buffer = [0u8; 4 * 1024];
            let header_end = loop {
                let read = stream.read(&mut buffer).unwrap();
                assert!(read > 0, "request ended before headers completed");
                received.extend_from_slice(&buffer[..read]);
                if let Some(index) = received.windows(4).position(|window| window == b"\r\n\r\n") {
                    break index + 4;
                }
            };
            let headers = std::str::from_utf8(&received[..header_end]).unwrap();
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .expect("loopback request must include content-length");
            while received.len() < header_end + content_length {
                let read = stream.read(&mut buffer).unwrap();
                assert!(read > 0, "request ended before body completed");
                received.extend_from_slice(&buffer[..read]);
            }
            stream
                .write_all(
                    b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .unwrap();
            serde_json::from_slice(&received[header_end..header_end + content_length]).unwrap()
        });
        let client = ProviderHttpClient::for_test(
            provider,
            format!("http://{address}/{}", provider.as_str()),
        )
        .unwrap();
        let cancellation = CancellationToken::default();
        let result = if streaming {
            client
                .stream_once_cancellable_blocking(
                    provider,
                    b"synthetic-loopback-key",
                    request,
                    &cancellation,
                    &mut |_| Ok(()),
                )
                .map(|_| ())
        } else {
            client
                .complete_once_cancellable_blocking(
                    provider,
                    b"synthetic-loopback-key",
                    request,
                    &cancellation,
                )
                .map(|_| ())
        };
        assert_eq!(result.unwrap_err(), ProviderError::unreachable());
        server.join().unwrap()
    }

    #[test]
    fn preset_v1_is_exact_hashed_and_has_only_fixed_https_hosts() {
        assert_eq!(ProviderId::ALL.len(), 2);
        assert_eq!(preset(ProviderId::Deepseek), &DEEPSEEK_PRESET);
        assert_eq!(preset(ProviderId::Openai), &OPENAI_PRESET);
        assert_eq!(
            DEEPSEEK_PRESET.request_url(),
            "https://api.deepseek.com/chat/completions"
        );
        assert_eq!(
            OPENAI_PRESET.request_url(),
            "https://api.openai.com/v1/responses"
        );
        assert!(validate_preset(&DEEPSEEK_PRESET).is_ok());
        assert!(validate_preset(&OPENAI_PRESET).is_ok());
        assert_eq!(preset_manifest_hash().len(), 64);

        let manifest: Value =
            serde_json::from_str(include_str!("../provider-presets.v1.json")).unwrap();
        assert_eq!(manifest["providers"][0]["model"], DEEPSEEK_PRESET.model);
        assert_eq!(manifest["providers"][1]["model"], OPENAI_PRESET.model);
    }

    #[test]
    fn request_validation_is_bounded_and_user_cannot_supply_preset_fields() {
        let request = NormalizedRequest::user_text("写一个场景建议".to_owned(), 512).unwrap();
        assert_eq!(request.messages.len(), 2);
        assert!(NormalizedRequest::user_text(String::new(), 512).is_err());
        assert!(NormalizedRequest::user_text("x".repeat(MAX_MESSAGE_BYTES + 1), 512).is_err());
        assert!(NormalizedRequest::user_text("ok".to_owned(), MAX_OUTPUT_TOKENS + 1).is_err());

        let deepseek = DEEPSEEK_ADAPTER.streaming_body(&request);
        let openai = OPENAI_ADAPTER.streaming_body(&request);
        assert_eq!(deepseek["model"], DEEPSEEK_PRESET.model);
        assert_eq!(openai["model"], OPENAI_PRESET.model);
        assert!(deepseek.get("base_url").is_none());
        assert!(openai.get("base_url").is_none());
    }

    #[test]
    fn both_adapters_capture_native_tool_requests_and_typed_continuations() {
        let initial = NormalizedRequest::user_text("执行测试操作".to_owned(), 512)
            .unwrap()
            .with_tool_definitions(vec![
                crate::canon_context::novelist_read_context_definition(),
            ])
            .unwrap();
        let deepseek_initial = capture_outbound_json(ProviderId::Deepseek, &initial, true);
        assert_eq!(
            deepseek_initial["tools"][0]["function"]["name"],
            "novelist.read_context"
        );
        assert_eq!(deepseek_initial["tool_choice"], "auto");
        assert_eq!(deepseek_initial["thinking"]["type"], "disabled");

        let openai_initial = capture_outbound_json(ProviderId::Openai, &initial, true);
        assert_eq!(openai_initial["tools"][0]["name"], "novelist.read_context");
        assert_eq!(openai_initial["tools"][0]["strict"], true);
        assert_eq!(
            deepseek_initial["tools"][0]["function"]["parameters"],
            openai_initial["tools"][0]["parameters"]
        );
        assert_eq!(openai_initial["parallel_tool_calls"], false);
        assert_eq!(
            openai_initial["include"],
            json!(["reasoning.encrypted_content"])
        );

        let result = ToolResult {
            call_id: "call_1".to_owned(),
            name: "novelist.read_context".to_owned(),
            version: 1,
            status: crate::tool_runtime::ToolResultStatus::Succeeded,
            output_json: Some(json!({"schemaVersion": 1})),
            error_code: None,
        };
        let call = ProviderToolCall {
            call_id: "call_1".to_owned(),
            name: "novelist.read_context".to_owned(),
            arguments_json: r#"{"capabilityId":"novel-story-telling","capabilityResources":[],"canonOperations":[]}"#.to_owned(),
        };
        let deepseek_continuation = initial
            .continue_after_tool(
                ProviderContinuation::Deepseek {
                    assistant_text: "我先处理。".to_owned(),
                    call: call.clone(),
                },
                result.clone(),
                512,
            )
            .unwrap();
        let deepseek_body =
            capture_outbound_json(ProviderId::Deepseek, &deepseek_continuation, true);
        assert_eq!(deepseek_body["tool_choice"], "none");
        assert_eq!(deepseek_body["thinking"]["type"], "disabled");
        assert_eq!(
            deepseek_body["messages"][deepseek_body["messages"].as_array().unwrap().len() - 1]
                ["tool_call_id"],
            "call_1"
        );

        let openai_continuation = initial
            .continue_after_tool(
                ProviderContinuation::Openai {
                    items: vec![
                        OpenAiContinuationItem::OutputMessage {
                            item_id: "msg_1".to_owned(),
                            status: "completed".to_owned(),
                            role: "assistant".to_owned(),
                            content: vec![OpenAiMessageContent::OutputText {
                                text: "我先处理。".to_owned(),
                            }],
                            phase: Some("commentary".to_owned()),
                        },
                        OpenAiContinuationItem::EncryptedReasoning {
                            item_id: "rs_1".to_owned(),
                            encrypted_content: "encrypted".to_owned(),
                            phase: None,
                        },
                        OpenAiContinuationItem::FunctionCall {
                            item_id: "fc_1".to_owned(),
                            call_id: call.call_id,
                            name: call.name,
                            arguments: call.arguments_json,
                            status: "completed".to_owned(),
                            phase: Some("commentary".to_owned()),
                        },
                    ],
                },
                result,
                512,
            )
            .unwrap();
        let openai_body = capture_outbound_json(ProviderId::Openai, &openai_continuation, true);
        assert_eq!(openai_body["tool_choice"], "none");
        assert!(openai_body.get("tools").is_none());
        let input = openai_body["input"].as_array().unwrap();
        assert_eq!(input[input.len() - 1]["type"], "function_call_output");
        assert_eq!(input[input.len() - 1]["call_id"], "call_1");
        assert_eq!(input[input.len() - 4]["phase"], "commentary");
    }

    #[test]
    fn both_adapters_capture_the_fixed_novelist_tool_and_catalog_within_body_limit() {
        let addendum = crate::capability_pack::embedded_pack()
            .unwrap()
            .agent_system_addendum();
        let request = NormalizedRequest::with_system(
            format!("{CONVERSATION_SYSTEM_INSTRUCTION}\n\n{addendum}"),
            vec![NormalizedMessage {
                role: MessageRole::User,
                content: "我们随便聊聊，再由你判断是否需要查看小说资料。".to_owned(),
            }],
            512,
        )
        .unwrap()
        .with_tool_definitions(vec![
            crate::canon_context::novelist_read_context_definition(),
        ])
        .unwrap();

        let deepseek = capture_outbound_json(ProviderId::Deepseek, &request, true);
        let openai = capture_outbound_json(ProviderId::Openai, &request, true);
        assert_eq!(deepseek["tools"].as_array().unwrap().len(), 1);
        assert_eq!(
            deepseek["tools"][0]["function"]["name"],
            "novelist.read_context"
        );
        assert_eq!(openai["tools"].as_array().unwrap().len(), 1);
        assert_eq!(openai["tools"][0]["name"], "novelist.read_context");
        assert_eq!(deepseek["tool_choice"], "auto");
        assert_eq!(openai["tool_choice"], "auto");
        assert!(deepseek.to_string().contains("novel-story-telling"));
        assert!(openai.to_string().contains("novel-story-telling"));
        assert!(serde_json::to_vec(&deepseek).unwrap().len() <= MAX_REQUEST_BYTES);
        assert!(serde_json::to_vec(&openai).unwrap().len() <= MAX_REQUEST_BYTES);
        assert!(!deepseek.to_string().contains("novelist.read_context@1"));
        assert!(!openai.to_string().contains("novelist.read_context@1"));
    }

    #[test]
    fn large_history_memory_user_and_tool_result_fit_both_continuation_body_limits() {
        let addendum = crate::capability_pack::embedded_pack()
            .unwrap()
            .agent_system_addendum();
        let system = format!(
            "{CONVERSATION_SYSTEM_INSTRUCTION}\n\n{addendum}\n\n派生 Memory：{}",
            "忆".repeat(4_000)
        );
        let initial = NormalizedRequest::with_system(
            system,
            vec![
                NormalizedMessage {
                    role: MessageRole::Assistant,
                    content: "史".repeat(2_000),
                },
                NormalizedMessage {
                    role: MessageRole::User,
                    content: "用".repeat(10_000),
                },
            ],
            512,
        )
        .unwrap()
        .with_tool_definitions(vec![
            crate::canon_context::novelist_read_context_definition(),
        ])
        .unwrap();
        let call = ProviderToolCall {
            call_id: "call_large_nr3".to_owned(),
            name: "novelist.read_context".to_owned(),
            arguments_json: r#"{"capabilityId":"novel-story-telling","capabilityResources":[],"canonOperations":[]}"#.to_owned(),
        };
        let result = ToolResult {
            call_id: call.call_id.clone(),
            name: call.name.clone(),
            version: 1,
            status: crate::tool_runtime::ToolResultStatus::Succeeded,
            output_json: Some(json!({
                "schemaVersion": 1,
                "sourceType": "canon_snapshot",
                "content": "x".repeat(crate::tool_runtime::MAX_TOOL_RESULT_BYTES - 256)
            })),
            error_code: None,
        };
        result.validate().unwrap();

        let deepseek = initial
            .clone()
            .continue_after_tool(
                ProviderContinuation::Deepseek {
                    assistant_text: "查阅中。".to_owned(),
                    call: call.clone(),
                },
                result.clone(),
                512,
            )
            .unwrap();
        let openai = initial
            .continue_after_tool(
                ProviderContinuation::Openai {
                    items: vec![OpenAiContinuationItem::FunctionCall {
                        item_id: "fc_large_nr3".to_owned(),
                        call_id: call.call_id,
                        name: call.name,
                        arguments: call.arguments_json,
                        status: "completed".to_owned(),
                        phase: Some("commentary".to_owned()),
                    }],
                },
                result,
                512,
            )
            .unwrap();
        let deepseek_body = capture_outbound_json(ProviderId::Deepseek, &deepseek, true);
        let openai_body = capture_outbound_json(ProviderId::Openai, &openai, true);
        assert!(serde_json::to_vec(&deepseek_body).unwrap().len() <= MAX_REQUEST_BYTES);
        assert!(serde_json::to_vec(&openai_body).unwrap().len() <= MAX_REQUEST_BYTES);
        assert_eq!(deepseek_body["tool_choice"], "none");
        assert_eq!(openai_body["tool_choice"], "none");
    }

    #[test]
    fn native_tool_parsers_reject_multiple_or_unknown_required_items() {
        let deepseek_calls = json!([
            {
                "id": "call_1",
                "type": "function",
                "function": {"name": "test.echo", "arguments": "{}"}
            },
            {
                "id": "call_2",
                "type": "function",
                "function": {"name": "test.echo", "arguments": "{}"}
            }
        ]);
        assert_eq!(
            parse_deepseek_tool_call(Some(&deepseek_calls))
                .err()
                .unwrap()
                .code,
            "provider.protocol"
        );

        let openai_response = json!({
            "output": [
                {
                    "type": "function_call",
                    "id": "fc_1",
                    "call_id": "call_1",
                    "name": "test.echo",
                    "arguments": "{}",
                    "status": "completed"
                },
                {"type": "future_required_item", "id": "future_1"}
            ]
        });
        assert_eq!(
            parse_openai_output(&openai_response).err().unwrap().code,
            "provider.protocol"
        );
    }

    #[test]
    fn both_provider_stream_and_completion_parsers_preserve_pairable_raw_arguments() {
        for raw in [
            r#"{"capabilityId":"novel-story-telling""#.to_owned(),
            r#"["not-an-object"]"#.to_owned(),
            format!(r#"{{"padding":"{}"}}"#, "x".repeat(40 * 1024)),
        ] {
            let deepseek_completion = json!({
                "model": DEEPSEEK_PRESET.model,
                "choices": [{
                    "message": {
                        "content": "",
                        "tool_calls": [{
                            "id": "call_raw",
                            "type": "function",
                            "function": {
                                "name": "novelist.read_context",
                                "arguments": raw
                            }
                        }]
                    },
                    "finish_reason": "tool_calls"
                }]
            });
            let parsed = DEEPSEEK_ADAPTER
                .parse_completion(&deepseek_completion)
                .unwrap();
            assert_eq!(parsed.tool_call.unwrap().arguments_json, raw);

            let deepseek_stream = json!({
                "model": DEEPSEEK_PRESET.model,
                "choices": [{
                    "delta": {
                        "tool_calls": [{
                            "index": 0,
                            "id": "call_raw",
                            "function": {
                                "name": "novelist.read_context",
                                "arguments": raw
                            }
                        }]
                    },
                    "finish_reason": "tool_calls"
                }]
            })
            .to_string();
            let mut accumulator = ToolCallAccumulator::default();
            let mut finish_reason = None;
            for event in DEEPSEEK_ADAPTER
                .parse_stream_data(&deepseek_stream)
                .unwrap()
            {
                match event {
                    AdapterStreamEvent::ToolCallDelta {
                        index,
                        call_id,
                        name,
                        arguments_delta,
                    } => accumulator
                        .push_delta(index, call_id, name, arguments_delta)
                        .unwrap(),
                    AdapterStreamEvent::Finished(reason) => finish_reason = Some(reason),
                    _ => {}
                }
            }
            assert_eq!(finish_reason, Some(FinishReason::ToolCall));
            let (call, continuation) = accumulator.finish(ProviderId::Deepseek, "").unwrap();
            assert_eq!(call.unwrap().arguments_json, raw);
            assert!(continuation.is_some());

            let openai_response = json!({
                "model": OPENAI_PRESET.model,
                "status": "completed",
                "output": [{
                    "type": "function_call",
                    "id": "fc_raw",
                    "call_id": "call_raw",
                    "name": "novelist.read_context",
                    "arguments": raw,
                    "status": "completed",
                    "phase": "commentary"
                }]
            });
            let parsed = OPENAI_ADAPTER.parse_completion(&openai_response).unwrap();
            assert_eq!(parsed.tool_call.unwrap().arguments_json, raw);
            let stream_event = json!({
                "type": "response.completed",
                "response": openai_response
            })
            .to_string();
            let events = OPENAI_ADAPTER.parse_stream_data(&stream_event).unwrap();
            let streamed = events.into_iter().find_map(|event| match event {
                AdapterStreamEvent::ToolEnvelope { call, .. } => Some(call),
                _ => None,
            });
            assert_eq!(streamed.unwrap().arguments_json, raw);
        }
    }

    #[test]
    fn provider_parsers_keep_damaged_envelopes_unpairable() {
        let mut accumulator = ToolCallAccumulator::default();
        accumulator
            .push_delta(
                0,
                Some("call_raw".to_owned()),
                None,
                r#"{"partial":"#.to_owned(),
            )
            .unwrap();
        assert_eq!(
            accumulator
                .finish(ProviderId::Deepseek, "")
                .err()
                .unwrap()
                .code,
            "provider.protocol"
        );

        let incomplete_openai = json!({
            "model": OPENAI_PRESET.model,
            "status": "completed",
            "output": [{
                "type": "function_call",
                "id": "fc_raw",
                "call_id": "call_raw",
                "name": "novelist.read_context",
                "arguments": "{",
                "status": "in_progress"
            }]
        });
        assert_eq!(
            OPENAI_ADAPTER
                .parse_completion(&incomplete_openai)
                .unwrap_err()
                .code,
            "provider.protocol"
        );
    }

    #[test]
    fn continuation_bounds_and_provider_identity_reject_before_outbound() {
        let initial = NormalizedRequest::user_text("执行测试操作".to_owned(), 512).unwrap();
        let result = ToolResult {
            call_id: "call_1".to_owned(),
            name: "test.echo".to_owned(),
            version: 1,
            status: crate::tool_runtime::ToolResultStatus::Succeeded,
            output_json: Some(json!({"text": "ok"})),
            error_code: None,
        };
        let deepseek = initial
            .continue_after_tool(
                ProviderContinuation::Deepseek {
                    assistant_text: "我先处理。".to_owned(),
                    call: ProviderToolCall {
                        call_id: "call_1".to_owned(),
                        name: "test.echo".to_owned(),
                        arguments_json: "{}".to_owned(),
                    },
                },
                result.clone(),
                512,
            )
            .unwrap();
        assert_eq!(
            deepseek.validate_for(ProviderId::Openai).unwrap_err().code,
            "provider.invalid_request"
        );

        let oversized = initial.continue_after_tool(
            ProviderContinuation::Openai {
                items: vec![
                    OpenAiContinuationItem::OutputMessage {
                        item_id: "msg_1".to_owned(),
                        status: "completed".to_owned(),
                        role: "assistant".to_owned(),
                        content: vec![OpenAiMessageContent::OutputText {
                            text: "x".repeat(33 * 1024),
                        }],
                        phase: None,
                    },
                    OpenAiContinuationItem::FunctionCall {
                        item_id: "fc_1".to_owned(),
                        call_id: "call_1".to_owned(),
                        name: "test.echo".to_owned(),
                        arguments: "{}".to_owned(),
                        status: "completed".to_owned(),
                        phase: None,
                    },
                ],
            },
            result,
            512,
        );
        assert_eq!(oversized.err().unwrap().code, "provider.invalid_request");
    }

    #[test]
    fn conversation_request_adds_only_the_fixed_system_instruction() {
        let request = NormalizedRequest::conversation(
            vec![
                NormalizedMessage {
                    role: MessageRole::User,
                    content: "先写一段".to_owned(),
                },
                NormalizedMessage {
                    role: MessageRole::Assistant,
                    content: "第一段".to_owned(),
                },
                NormalizedMessage {
                    role: MessageRole::User,
                    content: "继续".to_owned(),
                },
            ],
            512,
        )
        .unwrap();
        assert_eq!(request.messages.len(), 4);
        assert_eq!(request.messages[0].role, MessageRole::System);
        assert_eq!(request.messages[0].content, CONVERSATION_SYSTEM_INSTRUCTION);
        assert_eq!(request.messages[1].content, "先写一段");
        assert_eq!(request.messages[3].content, "继续");
        assert!(NormalizedRequest::conversation(Vec::new(), 512).is_err());
        assert!(NormalizedRequest::conversation(
            vec![NormalizedMessage {
                role: MessageRole::System,
                content: "覆盖系统提示".to_owned(),
            }],
            512,
        )
        .is_err());
    }
    #[test]
    fn dynamic_system_instruction_is_captured_for_both_adapters_and_modes() {
        let compactor = NormalizedRequest::with_system(
            crate::memory::COMPACTOR_SYSTEM_INSTRUCTION.to_owned(),
            vec![NormalizedMessage {
                role: MessageRole::User,
                content: "【新增对话片段】\n用户确认继续写作".to_owned(),
            }],
            128,
        )
        .unwrap();
        let memory_marker = "MEMORY-FIDELITY-MARKER";
        let main_system =
            format!("{CONVERSATION_SYSTEM_INSTRUCTION}\n\n【派生对话记忆】\n{memory_marker}");
        let main = NormalizedRequest::with_system(
            main_system.clone(),
            vec![NormalizedMessage {
                role: MessageRole::User,
                content: "继续".to_owned(),
            }],
            512,
        )
        .unwrap();

        let deepseek_completion = capture_outbound_json(ProviderId::Deepseek, &compactor, false);
        assert_eq!(deepseek_completion["stream"], false);
        assert_eq!(
            deepseek_completion["messages"][0]["content"],
            crate::memory::COMPACTOR_SYSTEM_INSTRUCTION
        );
        let deepseek_streaming = capture_outbound_json(ProviderId::Deepseek, &main, true);
        assert_eq!(deepseek_streaming["stream"], true);
        assert_eq!(deepseek_streaming["messages"][0]["content"], main_system);
        assert_eq!(
            deepseek_streaming
                .to_string()
                .matches(memory_marker)
                .count(),
            1
        );

        let openai_completion = capture_outbound_json(ProviderId::Openai, &compactor, false);
        assert_eq!(openai_completion["stream"], false);
        assert_eq!(
            openai_completion["instructions"],
            crate::memory::COMPACTOR_SYSTEM_INSTRUCTION
        );
        assert!(openai_completion["input"]
            .as_array()
            .unwrap()
            .iter()
            .all(|message| message["role"] != "system"));
        let openai_streaming = capture_outbound_json(ProviderId::Openai, &main, true);
        assert_eq!(openai_streaming["stream"], true);
        assert_eq!(openai_streaming["instructions"], main_system);
        assert!(openai_streaming["input"]
            .as_array()
            .unwrap()
            .iter()
            .all(|message| message["role"] != "system"));
        assert_eq!(
            openai_streaming.to_string().matches(memory_marker).count(),
            1
        );
    }

    #[test]
    fn invalid_system_contract_is_rejected_before_any_outbound_attempt() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let client = ProviderHttpClient::for_test(
            ProviderId::Deepseek,
            format!("http://{}/invalid-system", listener.local_addr().unwrap()),
        )
        .unwrap();
        let invalid_requests = [
            NormalizedRequest {
                messages: vec![
                    NormalizedMessage {
                        role: MessageRole::User,
                        content: "用户消息".to_owned(),
                    },
                    NormalizedMessage {
                        role: MessageRole::System,
                        content: "不在首位".to_owned(),
                    },
                ],
                max_output_tokens: 128,
                tool_definitions: Vec::new(),
                continuation: None,
                tool_result: None,
            },
            NormalizedRequest {
                messages: vec![
                    NormalizedMessage {
                        role: MessageRole::System,
                        content: "第一条".to_owned(),
                    },
                    NormalizedMessage {
                        role: MessageRole::System,
                        content: "重复".to_owned(),
                    },
                    NormalizedMessage {
                        role: MessageRole::User,
                        content: "用户消息".to_owned(),
                    },
                ],
                max_output_tokens: 128,
                tool_definitions: Vec::new(),
                continuation: None,
                tool_result: None,
            },
            NormalizedRequest {
                messages: vec![
                    NormalizedMessage {
                        role: MessageRole::System,
                        content: " \n\t".to_owned(),
                    },
                    NormalizedMessage {
                        role: MessageRole::User,
                        content: "用户消息".to_owned(),
                    },
                ],
                max_output_tokens: 128,
                tool_definitions: Vec::new(),
                continuation: None,
                tool_result: None,
            },
        ];
        for request in invalid_requests {
            assert_eq!(
                client
                    .complete_once(
                        ProviderId::Deepseek,
                        b"synthetic-loopback-key",
                        &request,
                        &CancellationToken::default(),
                    )
                    .unwrap_err(),
                ProviderError::invalid_request()
            );
        }
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn adapters_map_non_streaming_text_usage_and_finish_reason() {
        let deepseek = DEEPSEEK_ADAPTER
            .parse_completion(&json!({
                "model": "deepseek-v4-pro",
                "choices": [{
                    "finish_reason": "stop",
                    "message": {"content": "深度回答"}
                }],
                "usage": {"prompt_tokens": 3, "completion_tokens": 4, "total_tokens": 7}
            }))
            .unwrap();
        assert_eq!(deepseek.text, "深度回答");
        assert_eq!(deepseek.usage.unwrap().total_tokens, 7);
        assert_eq!(deepseek.finish_reason, FinishReason::Stop);

        let openai = OPENAI_ADAPTER
            .parse_completion(&json!({
                "model": "gpt-5.6-terra",
                "status": "completed",
                "output": [{
                    "type": "message",
                    "content": [{"type": "output_text", "text": "开放回答"}]
                }],
                "usage": {"input_tokens": 5, "output_tokens": 6, "total_tokens": 11}
            }))
            .unwrap();
        assert_eq!(openai.text, "开放回答");
        assert_eq!(openai.usage.unwrap().input_tokens, 5);
        assert_eq!(openai.finish_reason, FinishReason::Stop);
    }

    #[test]
    fn adapters_standardize_stream_events_without_exposing_raw_payloads() {
        let deepseek = DEEPSEEK_ADAPTER
            .parse_stream_data(
                r#"{"model":"deepseek-v4-pro","choices":[{"delta":{"content":"你"},"finish_reason":null}]}"#,
            )
            .unwrap();
        assert!(matches!(&deepseek[0], AdapterStreamEvent::Delta(delta) if delta == "你"));
        let openai = OPENAI_ADAPTER
            .parse_stream_data(
                r#"{"type":"response.output_text.delta","delta":"好","sequence_number":4}"#,
            )
            .unwrap();
        assert!(matches!(&openai[0], AdapterStreamEvent::Delta(delta) if delta == "好"));
        assert_eq!(
            DEEPSEEK_ADAPTER
                .parse_stream_data(r#"{"model":"other","choices":[]}"#)
                .err()
                .unwrap(),
            ProviderError::model_unavailable()
        );
        assert_eq!(
            OPENAI_ADAPTER.parse_stream_data("not-json").err().unwrap(),
            ProviderError::protocol()
        );
    }

    #[test]
    fn sse_decoder_handles_crlf_utf8_fragmentation_comments_and_multiline_data() {
        let mut decoder = SseDecoder::default();
        let bytes = "data: 你\r\ndata: 好\r\n\r\n: ping\n\ndata: [DONE]\n\n".as_bytes();
        let split = bytes.iter().position(|byte| *byte >= 0x80).unwrap() + 1;
        assert!(decoder.push(&bytes[..split]).unwrap().is_empty());
        let events = decoder.push(&bytes[split..]).unwrap();
        assert_eq!(events, vec!["你\n好", "[DONE]"]);
        assert!(decoder.finish().unwrap().is_empty());
    }

    #[test]
    fn status_error_and_cancellation_contracts_are_stable_and_bounded() {
        assert_eq!(
            map_http_status(StatusCode::UNAUTHORIZED),
            ProviderError::authentication()
        );
        assert_eq!(
            map_http_status(StatusCode::TOO_MANY_REQUESTS),
            ProviderError::rate_limited()
        );
        assert_eq!(
            map_http_status(StatusCode::NOT_FOUND),
            ProviderError::model_unavailable()
        );
        assert_eq!(MAX_RETRIES, 2);

        let token = CancellationToken::default();
        assert!(!token.is_cancelled());
        token.cancel();
        assert!(token.is_cancelled());

        assert!(ProviderHttpClient::for_test(
            ProviderId::Openai,
            "https://example.com/responses".to_owned()
        )
        .is_err());
    }
    #[test]
    fn nr1_compactor_and_main_primitives_make_one_attempt_each_without_retry() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::atomic::AtomicUsize;
        use std::thread;
        use std::time::{Duration, Instant};

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let attempts = Arc::new(AtomicUsize::new(0));
        let observed = attempts.clone();
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_millis(800);
            while Instant::now() < deadline {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        observed.fetch_add(1, Ordering::SeqCst);
                        let mut request = [0u8; 8 * 1024];
                        let _ = stream.read(&mut request);
                        let _ = stream.write_all(
                            b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        );
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("loopback accept failed: {error}"),
                }
            }
        });
        let client = ProviderHttpClient::for_test(
            ProviderId::Openai,
            format!("http://{address}/v1/responses"),
        )
        .unwrap();
        let request = NormalizedRequest::conversation(
            vec![NormalizedMessage {
                role: MessageRole::User,
                content: "继续".to_owned(),
            }],
            128,
        )
        .unwrap();
        let cancellation = CancellationToken::default();

        let completion = client.complete_once_cancellable_blocking(
            ProviderId::Openai,
            b"synthetic-loopback-key",
            &request,
            &cancellation,
        );
        assert_eq!(completion.unwrap_err(), ProviderError::unreachable());
        let streaming = client.stream_once_cancellable_blocking(
            ProviderId::Openai,
            b"synthetic-loopback-key",
            &request,
            &cancellation,
            &mut |_| Ok(()),
        );
        assert_eq!(streaming.unwrap_err(), ProviderError::unreachable());
        server.join().unwrap();
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn nr1_compactor_request_is_cancellable_while_http_is_in_flight() {
        use std::io::Read;
        use std::net::TcpListener;
        use std::sync::mpsc;
        use std::thread;
        use std::time::{Duration, Instant};

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (accepted_tx, accepted_rx) = mpsc::channel();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 8 * 1024];
            let _ = stream.read(&mut request);
            let _ = accepted_tx.send(());
            thread::sleep(Duration::from_secs(2));
        });
        let client = ProviderHttpClient::for_test(
            ProviderId::Openai,
            format!("http://{address}/v1/responses"),
        )
        .unwrap();
        let request = NormalizedRequest::conversation(
            vec![NormalizedMessage {
                role: MessageRole::User,
                content: "整理上下文".to_owned(),
            }],
            128,
        )
        .unwrap();
        let cancellation = CancellationToken::default();
        let worker_cancellation = cancellation.clone();
        let worker = thread::spawn(move || {
            client.complete_once_cancellable_blocking(
                ProviderId::Openai,
                b"synthetic-loopback-key",
                &request,
                &worker_cancellation,
            )
        });
        accepted_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("compactor loopback must receive exactly one request");
        let cancelled_at = Instant::now();
        cancellation.cancel();
        assert_eq!(
            worker.join().unwrap().unwrap_err(),
            ProviderError::cancelled()
        );
        assert!(cancelled_at.elapsed() < Duration::from_secs(2));
    }
}
