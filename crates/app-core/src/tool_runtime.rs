use crate::canon_context::{novelist_read_context_definition, NovelistReadContextExecutorV1};
use crate::capability_pack::ValidatedCapabilityPack;
use crate::project::ProjectHandle;
use crate::provider_core::CancellationToken;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;

pub const MAX_TOOL_DEFINITIONS: usize = 16;
pub const MAX_TOOL_DESCRIPTION_BYTES: usize = 2 * 1024;
pub const MAX_TOOL_SCHEMA_BYTES: usize = 8 * 1024;
pub const MAX_TOOL_DEFINITIONS_BYTES: usize = 24 * 1024;
pub const MAX_TOOL_ARGUMENTS_BYTES: usize = 16 * 1024;
pub const MAX_TOOL_CONTINUATION_BYTES: usize = 48 * 1024;
pub const MAX_TOOL_RESULT_BYTES: usize = 24 * 1024;

#[derive(Clone, PartialEq, Eq)]
pub struct ToolDefinition {
    pub name: String,
    pub version: u32,
    pub description: String,
    pub input_schema: Value,
    pub display_label: String,
}

impl ToolDefinition {
    pub fn validate(&self) -> Result<(), ToolContractError> {
        if !valid_tool_name(&self.name)
            || self.version == 0
            || self.description.trim().is_empty()
            || self.description.len() > MAX_TOOL_DESCRIPTION_BYTES
            || self.display_label.trim().is_empty()
            || self.display_label.len() > 160
            || self.display_label.contains(['\0', '\r', '\n'])
            || serde_json::to_vec(&self.input_schema)
                .map_err(|_| ToolContractError::SchemaInvalid)?
                .len()
                > MAX_TOOL_SCHEMA_BYTES
        {
            return Err(ToolContractError::DefinitionInvalid);
        }
        validate_schema(&self.input_schema, true)
    }

    pub fn validate_arguments(&self, arguments: &Value) -> Result<(), ToolContractError> {
        let bytes =
            serde_json::to_vec(arguments).map_err(|_| ToolContractError::ArgumentsInvalid)?;
        if bytes.len() > MAX_TOOL_ARGUMENTS_BYTES || !arguments.is_object() {
            return Err(ToolContractError::ArgumentsInvalid);
        }
        validate_value(arguments, &self.input_schema)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ProviderToolCall {
    pub call_id: String,
    pub name: String,
    pub arguments_json: String,
}

impl ProviderToolCall {
    pub fn validate_envelope(&self) -> Result<(), ToolContractError> {
        if !valid_call_id(&self.call_id)
            || !valid_tool_name(&self.name)
            || self.arguments_json.len() > MAX_TOOL_CONTINUATION_BYTES
        {
            return Err(ToolContractError::ArgumentsInvalid);
        }
        Ok(())
    }

    pub fn arguments(&self) -> Result<Value, ToolContractError> {
        self.validate_envelope()?;
        let value: Value = serde_json::from_str(&self.arguments_json)
            .map_err(|_| ToolContractError::ArgumentsInvalid)?;
        value
            .is_object()
            .then_some(value)
            .ok_or(ToolContractError::ArgumentsInvalid)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolResultStatus {
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ToolResult {
    pub call_id: String,
    pub name: String,
    pub version: u32,
    pub status: ToolResultStatus,
    pub output_json: Option<Value>,
    pub error_code: Option<String>,
}

impl ToolResult {
    pub fn validate(&self) -> Result<(), ToolContractError> {
        if !valid_call_id(&self.call_id)
            || !valid_tool_name(&self.name)
            || self.version == 0
            || !valid_error_code(self.error_code.as_deref())
        {
            return Err(ToolContractError::ResultInvalid);
        }
        match self.status {
            ToolResultStatus::Succeeded => {
                let output = self
                    .output_json
                    .as_ref()
                    .ok_or(ToolContractError::ResultInvalid)?;
                if !output.is_object()
                    || self.error_code.is_some()
                    || serde_json::to_vec(output)
                        .map_err(|_| ToolContractError::ResultInvalid)?
                        .len()
                        > MAX_TOOL_RESULT_BYTES
                {
                    return Err(ToolContractError::ResultInvalid);
                }
            }
            ToolResultStatus::Failed | ToolResultStatus::Cancelled => {
                if self.output_json.is_some() || self.error_code.is_none() {
                    return Err(ToolContractError::ResultInvalid);
                }
            }
        }
        Ok(())
    }

    pub fn provider_payload(&self) -> Value {
        match self.status {
            ToolResultStatus::Succeeded => serde_json::json!({
                "status": "succeeded", "output": self.output_json
            }),
            ToolResultStatus::Failed => serde_json::json!({
                "status": "failed", "errorCode": self.error_code
            }),
            ToolResultStatus::Cancelled => serde_json::json!({
                "status": "cancelled", "errorCode": self.error_code
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolContractError {
    DefinitionInvalid,
    SchemaInvalid,
    ArgumentsInvalid,
    ArgumentsTooLarge,
    ResultInvalid,
    UnknownTool,
    ExecutorUnqualified,
}

impl ToolContractError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::DefinitionInvalid => "agent.tool_definition_invalid",
            Self::SchemaInvalid => "agent.tool_schema_invalid",
            Self::ArgumentsInvalid => "agent.tool_arguments_invalid",
            Self::ArgumentsTooLarge => "agent.tool_arguments_too_large",
            Self::ResultInvalid => "agent.tool_result_invalid",
            Self::UnknownTool => "agent.tool_unknown",
            Self::ExecutorUnqualified => "agent.tool_executor_unqualified",
        }
    }
}

#[derive(Clone)]
pub struct ToolExecutionContext {
    pub project_id: String,
    pub conversation_session_id: String,
    pub turn_id: String,
}

pub trait ToolExecutor: Send + Sync {
    fn validate_arguments(&self, _arguments: &Value) -> Result<(), &'static str> {
        Ok(())
    }

    fn execute(
        &self,
        context: &ToolExecutionContext,
        arguments: &Value,
        cancellation: &CancellationToken,
    ) -> ToolExecutionOutput;
}

pub struct ToolExecutionOutput {
    pub status: ToolResultStatus,
    pub output_json: Option<Value>,
    pub error_code: Option<&'static str>,
}

#[derive(Clone)]
struct RegisteredTool {
    definition: ToolDefinition,
    executor: Arc<dyn ToolExecutor>,
}

#[derive(Clone, Default)]
pub struct ToolRegistry {
    tools: Vec<RegisteredTool>,
}

impl ToolRegistry {
    pub fn empty_production() -> Self {
        Self { tools: Vec::new() }
    }

    pub fn novelist_production(
        project: &ProjectHandle,
        pack: Arc<ValidatedCapabilityPack>,
        expected_session_id: String,
        expected_turn_id: String,
    ) -> Result<Self, ToolContractError> {
        let definition = novelist_read_context_definition();
        definition.validate()?;
        let registry = Self {
            tools: vec![RegisteredTool {
                definition,
                executor: Arc::new(NovelistReadContextExecutorV1::new(
                    project.clone(),
                    pack,
                    expected_session_id,
                    expected_turn_id,
                )),
            }],
        };
        validate_definitions(&registry.definitions())?;
        Ok(registry)
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .iter()
            .map(|registered| registered.definition.clone())
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    pub fn resolve(
        &self,
        call: &ProviderToolCall,
    ) -> Result<(&ToolDefinition, &dyn ToolExecutor, Value), ToolContractError> {
        let registered = self
            .tools
            .iter()
            .find(|registered| registered.definition.name == call.name)
            .ok_or(ToolContractError::UnknownTool)?;
        let arguments = self.decode_arguments(call, &registered.definition)?;
        registered.definition.validate_arguments(&arguments)?;
        Ok((
            &registered.definition,
            registered.executor.as_ref(),
            arguments,
        ))
    }

    pub fn lookup(
        &self,
        call: &ProviderToolCall,
    ) -> Result<(&ToolDefinition, &dyn ToolExecutor), ToolContractError> {
        call.validate_envelope()?;
        let registered = self
            .tools
            .iter()
            .find(|registered| registered.definition.name == call.name)
            .ok_or(ToolContractError::UnknownTool)?;
        Ok((&registered.definition, registered.executor.as_ref()))
    }

    pub fn decode_arguments(
        &self,
        call: &ProviderToolCall,
        definition: &ToolDefinition,
    ) -> Result<Value, ToolContractError> {
        let arguments = self.parse_arguments(call)?;
        definition.validate_arguments(&arguments)?;
        Ok(arguments)
    }

    pub fn parse_arguments(&self, call: &ProviderToolCall) -> Result<Value, ToolContractError> {
        call.validate_envelope()?;
        if call.arguments_json.len() > MAX_TOOL_ARGUMENTS_BYTES {
            return Err(ToolContractError::ArgumentsTooLarge);
        }
        call.arguments()
    }
}

pub fn validate_definitions(definitions: &[ToolDefinition]) -> Result<(), ToolContractError> {
    if definitions.len() > MAX_TOOL_DEFINITIONS {
        return Err(ToolContractError::DefinitionInvalid);
    }
    let mut names = HashSet::new();
    let mut total = 0usize;
    for definition in definitions {
        definition.validate()?;
        if !names.insert(definition.name.as_str()) {
            return Err(ToolContractError::DefinitionInvalid);
        }
        total = total
            .saturating_add(definition.description.len())
            .saturating_add(
                serde_json::to_vec(&definition.input_schema)
                    .map_err(|_| ToolContractError::DefinitionInvalid)?
                    .len(),
            );
    }
    (total <= MAX_TOOL_DEFINITIONS_BYTES)
        .then_some(())
        .ok_or(ToolContractError::DefinitionInvalid)
}

fn valid_tool_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn valid_call_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn valid_error_code(value: Option<&str>) -> bool {
    value.is_none_or(|code| {
        !code.is_empty()
            && code.len() <= 96
            && code.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b'-')
            })
    })
}

fn schema_type(object: &serde_json::Map<String, Value>) -> Result<(&str, bool), ToolContractError> {
    match object.get("type") {
        Some(Value::String(value)) => Ok((value.as_str(), false)),
        Some(Value::Array(types))
            if types.len() == 2
                && types[1].as_str() == Some("null")
                && matches!(types[0].as_str(), Some("string" | "integer" | "boolean")) =>
        {
            Ok((
                types[0].as_str().ok_or(ToolContractError::SchemaInvalid)?,
                true,
            ))
        }
        _ => Err(ToolContractError::SchemaInvalid),
    }
}

fn validate_schema(schema: &Value, root: bool) -> Result<(), ToolContractError> {
    let object = schema.as_object().ok_or(ToolContractError::SchemaInvalid)?;
    const ALLOWED: &[&str] = &[
        "type",
        "properties",
        "required",
        "additionalProperties",
        "items",
        "enum",
        "description",
    ];
    if object.keys().any(|key| !ALLOWED.contains(&key.as_str())) {
        return Err(ToolContractError::SchemaInvalid);
    }
    let (schema_type, nullable) = schema_type(object)?;
    if nullable && root {
        return Err(ToolContractError::SchemaInvalid);
    }
    if root && schema_type != "object" {
        return Err(ToolContractError::SchemaInvalid);
    }
    match schema_type {
        "object" => {
            if object.get("additionalProperties") != Some(&Value::Bool(false)) {
                return Err(ToolContractError::SchemaInvalid);
            }
            let properties = object
                .get("properties")
                .and_then(Value::as_object)
                .ok_or(ToolContractError::SchemaInvalid)?;
            for child in properties.values() {
                validate_schema(child, false)?;
            }
            let required = object
                .get("required")
                .and_then(Value::as_array)
                .ok_or(ToolContractError::SchemaInvalid)?;
            let mut seen = HashSet::new();
            for key in required {
                let key = key.as_str().ok_or(ToolContractError::SchemaInvalid)?;
                if !properties.contains_key(key) || !seen.insert(key) {
                    return Err(ToolContractError::SchemaInvalid);
                }
            }
            if seen.len() != properties.len()
                || properties.keys().any(|key| !seen.contains(key.as_str()))
            {
                return Err(ToolContractError::SchemaInvalid);
            }
        }
        "array" => validate_schema(
            object
                .get("items")
                .ok_or(ToolContractError::SchemaInvalid)?,
            false,
        )?,
        "string" | "number" | "integer" | "boolean" => {}
        _ => return Err(ToolContractError::SchemaInvalid),
    }
    Ok(())
}

fn validate_value(value: &Value, schema: &Value) -> Result<(), ToolContractError> {
    let object = schema.as_object().ok_or(ToolContractError::SchemaInvalid)?;
    let (schema_type, nullable) = schema_type(object)?;
    if value.is_null() {
        return nullable
            .then_some(())
            .ok_or(ToolContractError::ArgumentsInvalid);
    }
    let matches = match schema_type {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "boolean" => value.is_boolean(),
        _ => false,
    };
    if !matches
        || object
            .get("enum")
            .and_then(Value::as_array)
            .is_some_and(|allowed| !allowed.contains(value))
    {
        return Err(ToolContractError::ArgumentsInvalid);
    }
    if let Some(map) = value.as_object() {
        let properties = object
            .get("properties")
            .and_then(Value::as_object)
            .ok_or(ToolContractError::SchemaInvalid)?;
        if map.keys().any(|key| !properties.contains_key(key)) {
            return Err(ToolContractError::ArgumentsInvalid);
        }
        if object
            .get("required")
            .and_then(Value::as_array)
            .is_some_and(|required| {
                required
                    .iter()
                    .filter_map(Value::as_str)
                    .any(|key| !map.contains_key(key))
            })
        {
            return Err(ToolContractError::ArgumentsInvalid);
        }
        for (key, child) in map {
            validate_value(
                child,
                properties
                    .get(key)
                    .ok_or(ToolContractError::ArgumentsInvalid)?,
            )?;
        }
    }
    if let Some(items) = value.as_array() {
        let item_schema = object
            .get("items")
            .ok_or(ToolContractError::SchemaInvalid)?;
        for item in items {
            validate_value(item, item_schema)?;
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{mpsc, Arc};
    use std::thread;
    use std::time::{Duration, Instant};

    #[derive(Clone, Copy)]
    enum QualifiedTestExecutor {
        EchoV1,
        CooperativeCancellationV1,
        DeterministicFailureV1,
    }

    impl QualifiedTestExecutor {
        fn definition(self) -> ToolDefinition {
            let description = match self {
                Self::EchoV1 => "回显确定性输入",
                Self::CooperativeCancellationV1 => "等待并确认取消",
                Self::DeterministicFailureV1 => "返回确定性失败",
            };
            ToolDefinition {
                name: "test.echo".to_owned(),
                version: 1,
                description: description.to_owned(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {"text": {"type": "string"}},
                    "required": ["text"],
                    "additionalProperties": false
                }),
                display_label: "处理测试内容".to_owned(),
            }
        }
    }

    struct TestRegistration {
        qualification: Option<QualifiedTestExecutor>,
        executor: Arc<dyn ToolExecutor>,
    }

    fn from_test_registrations(
        registrations: Vec<TestRegistration>,
    ) -> Result<ToolRegistry, ToolContractError> {
        let mut tools = Vec::with_capacity(registrations.len());
        for registration in registrations {
            let qualification = registration
                .qualification
                .ok_or(ToolContractError::ExecutorUnqualified)?;
            tools.push(RegisteredTool {
                definition: qualification.definition(),
                executor: registration.executor,
            });
        }
        validate_definitions(
            &tools
                .iter()
                .map(|registered| registered.definition.clone())
                .collect::<Vec<_>>(),
        )?;
        Ok(ToolRegistry { tools })
    }

    struct EchoExecutor {
        executions: Arc<AtomicUsize>,
    }

    struct CooperativeCancellationExecutor {
        started: mpsc::Sender<()>,
        exits: Arc<AtomicUsize>,
    }

    struct DeterministicFailureExecutor {
        executions: Arc<AtomicUsize>,
    }

    struct UnqualifiedExecutor {
        executions: Arc<AtomicUsize>,
    }

    impl ToolExecutor for EchoExecutor {
        fn execute(
            &self,
            _context: &ToolExecutionContext,
            arguments: &Value,
            cancellation: &CancellationToken,
        ) -> ToolExecutionOutput {
            self.executions.fetch_add(1, Ordering::SeqCst);
            if cancellation.is_cancelled() {
                return ToolExecutionOutput {
                    status: ToolResultStatus::Cancelled,
                    output_json: None,
                    error_code: Some("agent.tool_cancelled"),
                };
            }
            ToolExecutionOutput {
                status: ToolResultStatus::Succeeded,
                output_json: Some(arguments.clone()),
                error_code: None,
            }
        }
    }

    impl ToolExecutor for CooperativeCancellationExecutor {
        fn execute(
            &self,
            _context: &ToolExecutionContext,
            _arguments: &Value,
            cancellation: &CancellationToken,
        ) -> ToolExecutionOutput {
            let _ = self.started.send(());
            while !cancellation.is_cancelled() {
                thread::sleep(Duration::from_millis(5));
            }
            self.exits.fetch_add(1, Ordering::SeqCst);
            ToolExecutionOutput {
                status: ToolResultStatus::Cancelled,
                output_json: None,
                error_code: Some("agent.tool_cancelled"),
            }
        }
    }

    impl ToolExecutor for DeterministicFailureExecutor {
        fn execute(
            &self,
            _context: &ToolExecutionContext,
            _arguments: &Value,
            _cancellation: &CancellationToken,
        ) -> ToolExecutionOutput {
            self.executions.fetch_add(1, Ordering::SeqCst);
            ToolExecutionOutput {
                status: ToolResultStatus::Failed,
                output_json: None,
                error_code: Some("agent.tool_executor_failed"),
            }
        }
    }

    impl ToolExecutor for UnqualifiedExecutor {
        fn execute(
            &self,
            _context: &ToolExecutionContext,
            _arguments: &Value,
            _cancellation: &CancellationToken,
        ) -> ToolExecutionOutput {
            self.executions.fetch_add(1, Ordering::SeqCst);
            ToolExecutionOutput {
                status: ToolResultStatus::Succeeded,
                output_json: Some(serde_json::json!({"unexpected": true})),
                error_code: None,
            }
        }
    }

    fn qualified_registry(
        qualification: QualifiedTestExecutor,
        executor: Arc<dyn ToolExecutor>,
    ) -> ToolRegistry {
        from_test_registrations(vec![TestRegistration {
            qualification: Some(qualification),
            executor,
        }])
        .expect("qualified synthetic executor")
    }

    pub(crate) fn echo_registry(executions: Arc<AtomicUsize>) -> ToolRegistry {
        qualified_registry(
            QualifiedTestExecutor::EchoV1,
            Arc::new(EchoExecutor { executions }),
        )
    }

    pub(crate) fn cooperative_cancellation_registry(
        started: mpsc::Sender<()>,
        exits: Arc<AtomicUsize>,
    ) -> ToolRegistry {
        qualified_registry(
            QualifiedTestExecutor::CooperativeCancellationV1,
            Arc::new(CooperativeCancellationExecutor { started, exits }),
        )
    }

    pub(crate) fn deterministic_failure_registry(executions: Arc<AtomicUsize>) -> ToolRegistry {
        qualified_registry(
            QualifiedTestExecutor::DeterministicFailureV1,
            Arc::new(DeterministicFailureExecutor { executions }),
        )
    }

    fn definition() -> ToolDefinition {
        ToolDefinition {
            name: "fixture.echo".to_owned(),
            version: 1,
            description: "回显输入".to_owned(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {"text": {"type": "string"}},
                "required": ["text"],
                "additionalProperties": false
            }),
            display_label: "处理测试内容".to_owned(),
        }
    }

    #[test]
    fn schema_and_arguments_are_strict_local_boundaries() {
        let definition = definition();
        definition.validate().unwrap();
        definition
            .validate_arguments(&serde_json::json!({"text": "ok"}))
            .unwrap();
        assert_eq!(
            definition
                .validate_arguments(&serde_json::json!({"text": "ok", "path": "x"}))
                .unwrap_err(),
            ToolContractError::ArgumentsInvalid
        );
        let mut invalid = definition;
        invalid.input_schema["$ref"] = Value::String("https://invalid.test/schema".to_owned());
        assert_eq!(
            invalid.validate().unwrap_err(),
            ToolContractError::SchemaInvalid
        );
    }

    #[test]
    fn strict_nullable_schema_accepts_only_required_scalar_null_unions() {
        let definition = ToolDefinition {
            name: "fixture.nullable".to_owned(),
            version: 1,
            description: "验证严格 nullable 合同".to_owned(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "cursor": {"type": ["integer", "null"]},
                    "query": {"type": ["string", "null"]},
                    "caseSensitive": {"type": ["boolean", "null"]}
                },
                "required": ["cursor", "query", "caseSensitive"],
                "additionalProperties": false
            }),
            display_label: "验证严格合同".to_owned(),
        };
        definition.validate().unwrap();
        definition
            .validate_arguments(&serde_json::json!({
                "cursor": null,
                "query": "陆沉",
                "caseSensitive": false
            }))
            .unwrap();

        for invalid_schema in [
            serde_json::json!({
                "type": "object",
                "properties": {"child": {"type": ["object", "null"]}},
                "required": ["child"],
                "additionalProperties": false
            }),
            serde_json::json!({
                "type": "object",
                "properties": {"items": {"type": ["array", "null"]}},
                "required": ["items"],
                "additionalProperties": false
            }),
            serde_json::json!({
                "type": "object",
                "properties": {"query": {"type": ["null", "string"]}},
                "required": ["query"],
                "additionalProperties": false
            }),
            serde_json::json!({
                "type": "object",
                "properties": {"query": {"type": ["string", "null"]}},
                "required": [],
                "additionalProperties": false
            }),
        ] {
            let mut invalid = definition.clone();
            invalid.input_schema = invalid_schema;
            assert_eq!(invalid.validate(), Err(ToolContractError::SchemaInvalid));
        }
    }

    #[test]
    fn production_registry_is_empty() {
        let registry = ToolRegistry::empty_production();
        assert!(registry.is_empty());
        assert!(registry.definitions().is_empty());
    }

    #[test]
    fn provider_visible_tool_names_are_unique_across_versions() {
        let first = definition();
        let mut second = definition();
        second.version = 2;
        assert_eq!(
            validate_definitions(&[first, second]).unwrap_err(),
            ToolContractError::DefinitionInvalid
        );
    }

    #[test]
    fn qualified_executor_contracts_cover_success_failure_and_two_second_cancellation() {
        let context = ToolExecutionContext {
            project_id: "project".to_owned(),
            conversation_session_id: "session".to_owned(),
            turn_id: "turn".to_owned(),
        };
        let arguments = serde_json::json!({"text": "ok"});
        let call = ProviderToolCall {
            call_id: "call_contract".to_owned(),
            name: "test.echo".to_owned(),
            arguments_json: r#"{"text":"ok"}"#.to_owned(),
        };

        let success_count = Arc::new(AtomicUsize::new(0));
        let success_registry = echo_registry(success_count.clone());
        let (_, success_executor) = success_registry.lookup(&call).unwrap();
        assert_eq!(
            success_executor
                .execute(&context, &arguments, &CancellationToken::default())
                .status,
            ToolResultStatus::Succeeded
        );
        assert_eq!(success_count.load(Ordering::SeqCst), 1);

        let failure_count = Arc::new(AtomicUsize::new(0));
        let failure_registry = deterministic_failure_registry(failure_count.clone());
        let (_, failure_executor) = failure_registry.lookup(&call).unwrap();
        assert_eq!(
            failure_executor
                .execute(&context, &arguments, &CancellationToken::default())
                .status,
            ToolResultStatus::Failed
        );
        assert_eq!(failure_count.load(Ordering::SeqCst), 1);

        let cancellation = CancellationToken::default();
        let worker_cancellation = cancellation.clone();
        let (started_tx, started_rx) = mpsc::channel();
        let exits = Arc::new(AtomicUsize::new(0));
        let worker_exits = exits.clone();
        let worker = thread::spawn(move || {
            let registry = cooperative_cancellation_registry(started_tx, worker_exits);
            let (_, executor) = registry.lookup(&call).unwrap();
            executor.execute(&context, &arguments, &worker_cancellation)
        });
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("qualified cancellation executor must start");
        let cancelled_at = Instant::now();
        cancellation.cancel();
        let output = worker.join().unwrap();
        assert!(
            cancelled_at.elapsed() < crate::conversation_runtime::TOOL_CANCELLATION_ACK_DEADLINE
        );
        assert_eq!(output.status, ToolResultStatus::Cancelled);
        assert_eq!(exits.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn unqualified_executor_is_rejected_before_visibility_or_execution() {
        let executions = Arc::new(AtomicUsize::new(0));
        let result = from_test_registrations(vec![TestRegistration {
            qualification: None,
            executor: Arc::new(UnqualifiedExecutor {
                executions: executions.clone(),
            }),
        }]);
        assert_eq!(result.err(), Some(ToolContractError::ExecutorUnqualified));
        assert_eq!(executions.load(Ordering::SeqCst), 0);
    }
}
