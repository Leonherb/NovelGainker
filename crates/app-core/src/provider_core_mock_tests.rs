use super::*;
use crate::canon_apply::apply_canon_file_recoverable;
use crate::project::{create_project, inspect_project_root, ProjectScanMode};
use crate::recovery::RecoveryStore;
use novelgainker_workflows::character_v2::parse_and_serialize_character_v2_create;
use novelgainker_workflows::material::parse_and_serialize_material;
use novelgainker_workflows::plot::parse_and_serialize_plot;
use novelgainker_workflows::proposal::{
    build_canon_proposal, BuildCanonProposalInput, CanonFileOperationKind,
};
use novelgainker_workflows::setting::parse_and_serialize_setting;
use novelgainker_workflows::{
    registry, workflow_resources, CHARACTER_CREATE_WORKFLOW_ID, MATERIAL_CREATE_WORKFLOW_ID,
    PLOT_CREATE_WORKFLOW_ID, SETTING_CREATE_WORKFLOW_ID,
};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

use serde_json::Value;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

struct RecordedRequest {
    target: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

struct ResponsePlan {
    status: &'static str,
    headers: Vec<(String, String)>,
    chunks: Vec<Vec<u8>>,
    declared_length: Option<usize>,
    before_headers: Duration,
    between_chunks: Duration,
}

impl ResponsePlan {
    fn json(status: &'static str, body: Vec<u8>) -> Self {
        Self {
            status,
            headers: vec![("Content-Type".to_owned(), "application/json".to_owned())],
            chunks: vec![body],
            declared_length: None,
            before_headers: Duration::ZERO,
            between_chunks: Duration::ZERO,
        }
    }

    fn empty(status: &'static str) -> Self {
        Self::json(status, Vec::new())
    }

    fn sse(chunks: Vec<Vec<u8>>, between_chunks: Duration) -> Self {
        Self {
            status: "200 OK",
            headers: vec![("Content-Type".to_owned(), "text/event-stream".to_owned())],
            chunks,
            declared_length: None,
            before_headers: Duration::ZERO,
            between_chunks,
        }
    }

    fn with_header(mut self, name: &str, value: String) -> Self {
        self.headers.push((name.to_owned(), value));
        self
    }

    fn delayed(mut self, delay: Duration) -> Self {
        self.before_headers = delay;
        self
    }

    fn with_declared_length(mut self, length: usize) -> Self {
        self.declared_length = Some(length);
        self
    }
}

struct MockServer {
    endpoint: String,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
    handle: JoinHandle<()>,
}

impl MockServer {
    fn spawn(plans: Vec<ResponsePlan>) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let endpoint = format!("http://{address}/mock");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&requests);
        let handle = thread::spawn(move || {
            for plan in plans {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                if let Some(request) = read_request(&mut stream) {
                    recorded.lock().unwrap().push(request);
                }
                if !plan.before_headers.is_zero() {
                    thread::sleep(plan.before_headers);
                }
                write_response(&mut stream, plan);
            }
        });
        Self {
            endpoint,
            requests,
            handle,
        }
    }

    fn finish(self) -> Vec<RecordedRequest> {
        self.handle.join().unwrap();
        let mut requests = self.requests.lock().unwrap();
        requests.drain(..).collect()
    }
}

struct NoRequestSpy {
    endpoint: String,
    seen: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

impl NoRequestSpy {
    fn spawn() -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!(
            "http://{}/must-not-be-called",
            listener.local_addr().unwrap()
        );
        let seen = Arc::new(AtomicBool::new(false));
        let observed = Arc::clone(&seen);
        let handle = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_millis(400);
            while Instant::now() < deadline {
                match listener.accept() {
                    Ok(_) => {
                        observed.store(true, AtomicOrdering::SeqCst);
                        return;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => return,
                }
            }
        });
        Self {
            endpoint,
            seen,
            handle,
        }
    }

    fn finish(self) -> bool {
        self.handle.join().unwrap();
        self.seen.load(AtomicOrdering::SeqCst)
    }
}

fn read_request(stream: &mut TcpStream) -> Option<RecordedRequest> {
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 4 * 1024];
    let header_end = loop {
        let read = stream.read(&mut buffer).ok()?;
        if read == 0 {
            return None;
        }
        bytes.extend_from_slice(&buffer[..read]);
        if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break index + 4;
        }
        if bytes.len() > MAX_REQUEST_BYTES + 16 * 1024 {
            return None;
        }
    };
    let header_text = std::str::from_utf8(&bytes[..header_end]).ok()?;
    let mut lines = header_text.split("\r\n");
    let target = lines.next()?.split_whitespace().nth(1)?.to_owned();
    let mut headers = HashMap::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line.split_once(':')?;
        headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_owned());
    }
    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_default();
    while bytes.len() < header_end.saturating_add(content_length) {
        let read = stream.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    Some(RecordedRequest {
        target,
        headers,
        body: bytes[header_end..].to_vec(),
    })
}

fn write_response(stream: &mut TcpStream, plan: ResponsePlan) {
    let body_length = plan
        .declared_length
        .unwrap_or_else(|| plan.chunks.iter().map(Vec::len).sum());
    let mut head = format!(
        "HTTP/1.1 {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        plan.status, body_length
    );
    for (name, value) in plan.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }
    let _ = stream.flush();
    for (index, chunk) in plan.chunks.into_iter().enumerate() {
        if index > 0 && !plan.between_chunks.is_zero() {
            thread::sleep(plan.between_chunks);
        }
        if stream.write_all(&chunk).is_err() {
            return;
        }
        let _ = stream.flush();
    }
}

fn success_body(provider: ProviderId, include_usage: bool) -> Vec<u8> {
    success_body_with_text(provider, "OK", include_usage)
}

fn success_body_with_text(provider: ProviderId, text: &str, include_usage: bool) -> Vec<u8> {
    let value = match provider {
        ProviderId::Deepseek => json!({
            "model": DEEPSEEK_PRESET.model,
            "choices": [{
                "message": { "content": text },
                "finish_reason": "stop"
            }],
            "usage": include_usage.then(|| json!({
                "prompt_tokens": 2,
                "completion_tokens": 1,
                "total_tokens": 3
            }))
        }),
        ProviderId::Openai => json!({
            "model": OPENAI_PRESET.model,
            "status": "completed",
            "output": [{
                "content": [{ "type": "output_text", "text": text }]
            }],
            "usage": include_usage.then(|| json!({
                "input_tokens": 2,
                "output_tokens": 1,
                "total_tokens": 3
            }))
        }),
    };
    serde_json::to_vec(&value).unwrap()
}

struct VerifiedTemporaryRoot {
    root: PathBuf,
    temporary_base: PathBuf,
}

impl VerifiedTemporaryRoot {
    fn new() -> Self {
        let temporary_base = std::env::temp_dir().canonicalize().unwrap();
        let root = temporary_base.join(format!("novelgainker-ng306-provider-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        assert_eq!(root.parent(), Some(temporary_base.as_path()));
        assert!(root
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.starts_with("novelgainker-ng306-provider-")));
        Self {
            root,
            temporary_base,
        }
    }
}

impl Drop for VerifiedTemporaryRoot {
    fn drop(&mut self) {
        assert_eq!(self.root.parent(), Some(self.temporary_base.as_path()));
        assert!(self
            .root
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.starts_with("novelgainker-ng306-provider-")));
        fs::remove_dir_all(&self.root).unwrap();
    }
}
fn stream_payload(provider: ProviderId, include_usage: bool) -> String {
    match provider {
        ProviderId::Deepseek => {
            let usage = if include_usage {
                format!(
                    "data: {{\"model\":\"{}\",\"choices\":[],\"usage\":{{\"prompt_tokens\":2,\"completion_tokens\":2,\"total_tokens\":4}}}}\n\n",
                    DEEPSEEK_PRESET.model
                )
            } else {
                String::new()
            };
            format!(
                "data: {{\"model\":\"{}\",\"choices\":[{{\"delta\":{{\"content\":\"你\"}},\"finish_reason\":null}}]}}\n\n\
                 data: {{\"model\":\"{}\",\"choices\":[{{\"delta\":{{\"content\":\"好\"}},\"finish_reason\":null}}]}}\n\n\
                 {usage}\
                 data: {{\"model\":\"{}\",\"choices\":[{{\"delta\":{{}},\"finish_reason\":\"stop\"}}]}}\n\n\
                 data: [DONE]\n\n",
                DEEPSEEK_PRESET.model, DEEPSEEK_PRESET.model, DEEPSEEK_PRESET.model
            )
        }
        ProviderId::Openai => {
            let usage = if include_usage {
                "\"usage\":{\"input_tokens\":2,\"output_tokens\":2,\"total_tokens\":4},"
            } else {
                ""
            };
            format!(
                "data: {{\"type\":\"response.output_text.delta\",\"sequence_number\":1,\"delta\":\"你\"}}\n\n\
                 data: {{\"type\":\"response.output_text.delta\",\"sequence_number\":2,\"delta\":\"好\"}}\n\n\
                 data: {{\"type\":\"response.completed\",\"response\":{{\"model\":\"{}\",{usage}\"status\":\"completed\"}}}}\n\n",
                OPENAI_PRESET.model
            )
        }
    }
}

fn fragmented_utf8(payload: String) -> Vec<Vec<u8>> {
    let bytes = payload.into_bytes();
    let split = bytes.iter().position(|byte| *byte >= 0x80).unwrap() + 1;
    vec![bytes[..split].to_vec(), bytes[split..].to_vec()]
}

fn request() -> NormalizedRequest {
    NormalizedRequest::user_text("固定 Mock 请求".to_owned(), 32).unwrap()
}

#[test]
fn ng_405_character_v2_schema_is_fixed_for_both_loopback_providers() {
    let schema = r#"{
      "$schema":"https://json-schema.org/draft/2020-12/schema",
      "$id":"https://schemas.novelgainker.local/character.json",
      "title":"CharacterProposalV2",
      "type":"object",
      "additionalProperties":false,
      "required":["schemaVersion"],
      "properties":{"schemaVersion":{"type":"integer","const":2}}
    }"#;
    for provider in ProviderId::ALL {
        let server = MockServer::spawn(vec![ResponsePlan::json(
            "200 OK",
            success_body(provider, false),
        )]);
        let client = ProviderHttpClient::for_test(provider, server.endpoint.clone()).unwrap();
        let request =
            NormalizedRequest::character_proposal_json("固定人物 Prompt".to_owned(), 512, schema)
                .unwrap();
        client
            .complete(
                provider,
                b"ng305-structured-canary",
                &request,
                &CancellationToken::default(),
            )
            .unwrap();
        let recorded = server.finish();
        assert_eq!(recorded.len(), 1);
        let body: Value = serde_json::from_slice(&recorded[0].body).unwrap();
        match provider {
            ProviderId::Deepseek => {
                assert_eq!(body["response_format"], json!({ "type": "json_object" }));
            }
            ProviderId::Openai => {
                assert_eq!(body["text"]["format"]["type"], "json_schema");
                assert_eq!(body["text"]["format"]["strict"], true);
                assert_eq!(body["store"], false);
            }
        }
    }
}

#[test]
fn ng_402_setting_generation_is_strict_and_identical_across_both_loopback_providers() {
    let schema = workflow_resources(SETTING_CREATE_WORKFLOW_ID)
        .unwrap()
        .output_schema;
    for provider in ProviderId::ALL {
        let setting_json = json!({
            "schemaVersion": 1,
            "workflowId": SETTING_CREATE_WORKFLOW_ID,
            "setting": {
                "displayName": "雾城",
                "settingType": "location",
                "summary": "一座终年被浓雾笼罩的工业城市",
                "atmosphere": "潮湿、幽暗，钟声控制居民行动",
                "rules": ["第三次钟声后任何人不得离开室内"],
                "constraints": ["机械装置不能自行运转"],
                "plotHooks": ["钟楼记录与官方历法相差一天"]
            }
        })
        .to_string();
        let server = MockServer::spawn(vec![ResponsePlan::json(
            "200 OK",
            success_body_with_text(provider, &setting_json, false),
        )]);
        let client = ProviderHttpClient::for_test(provider, server.endpoint.clone()).unwrap();
        let request =
            NormalizedRequest::setting_proposal_json("固定设定 Prompt".to_owned(), 768, schema)
                .unwrap();
        let response = client
            .complete(
                provider,
                b"ng402-setting-canary",
                &request,
                &CancellationToken::default(),
            )
            .unwrap();
        let artifact = parse_and_serialize_setting(
            &response.text,
            "setting_50000000-0000-4000-8000-000000000005",
        )
        .unwrap();
        assert_eq!(artifact.relative_path, "world/雾城.md");
        assert!(artifact.markdown.contains("kind: setting"));

        let recorded = server.finish();
        assert_eq!(recorded.len(), 1);
        let body: Value = serde_json::from_slice(&recorded[0].body).unwrap();
        match provider {
            ProviderId::Deepseek => {
                assert_eq!(body["response_format"], json!({ "type": "json_object" }));
            }
            ProviderId::Openai => {
                assert_eq!(body["text"]["format"]["name"], "setting_proposal_v1");
                assert_eq!(body["text"]["format"]["strict"], true);
                assert_eq!(body["store"], false);
            }
        }
    }
}

#[test]
fn ng_403_material_generation_is_strict_and_byte_identical_across_both_loopback_providers() {
    let schema = workflow_resources(MATERIAL_CREATE_WORKFLOW_ID)
        .unwrap()
        .output_schema;
    let material_json = json!({
        "schemaVersion": 1,
        "workflowId": MATERIAL_CREATE_WORKFLOW_ID,
        "material": {
            "displayName": "退潮铜钥匙",
            "materialType": "clue",
            "summary": "一枚只在退潮后显出字迹的旧铜钥匙。",
            "content": "钥匙来自封闭多年的潮汐塔。",
            "intendedUse": "作为第二幕调查线索。",
            "canonReason": "遵守既有潮汐规则，并可在后续回收。"
        }
    })
    .to_string();
    let mut markdown_by_provider = Vec::new();

    for provider in ProviderId::ALL {
        let server = MockServer::spawn(vec![ResponsePlan::json(
            "200 OK",
            success_body_with_text(provider, &material_json, false),
        )]);
        let client = ProviderHttpClient::for_test(provider, server.endpoint.clone()).unwrap();
        let request =
            NormalizedRequest::material_proposal_json("固定素材 Prompt".to_owned(), 1_024, schema)
                .unwrap();
        let response = client
            .complete(
                provider,
                b"ng403-material-canary",
                &request,
                &CancellationToken::default(),
            )
            .unwrap();
        let artifact = parse_and_serialize_material(
            &response.text,
            "material_60000000-0000-4000-8000-000000000006",
        )
        .unwrap();
        assert_eq!(artifact.relative_path, "materials/退潮铜钥匙.md");
        assert!(artifact.markdown.contains("kind: material"));
        markdown_by_provider.push(artifact.markdown);

        let recorded = server.finish();
        assert_eq!(recorded.len(), 1);
        let body: Value = serde_json::from_slice(&recorded[0].body).unwrap();
        match provider {
            ProviderId::Deepseek => {
                assert_eq!(body["response_format"], json!({ "type": "json_object" }));
            }
            ProviderId::Openai => {
                assert_eq!(body["text"]["format"]["name"], "material_proposal_v1");
                assert_eq!(body["text"]["format"]["strict"], true);
                assert_eq!(body["store"], false);
            }
        }
    }

    assert_eq!(markdown_by_provider.len(), 2);
    assert_eq!(markdown_by_provider[0], markdown_by_provider[1]);
}

#[test]
fn ng_404_plot_generation_is_strict_and_byte_identical_across_both_loopback_providers() {
    let schema = workflow_resources(PLOT_CREATE_WORKFLOW_ID)
        .unwrap()
        .output_schema;
    let plot_json = json!({
        "schemaVersion": 1,
        "workflowId": PLOT_CREATE_WORKFLOW_ID,
        "plot": {
            "title": "退潮之塔",
            "promise": "林遥追查潮汐塔封闭真相，并逐步发现钥匙与自身记忆的关联。",
            "pressure": "每次退潮都会缩短调查窗口，而公开真相将永久夺走林遥的一段记忆。",
            "turns": [
                "铜钥匙在退潮后显字，指向被封闭的塔基。",
                "林遥发现导师参与了旧案掩盖，调查由外部谜案转成身份危机。",
                "最终退潮前，林遥必须在保留记忆与公开真相之间主动选择。"
            ],
            "reveals": ["潮汐塔以调查者的记忆维持封印。"],
            "payoffs": ["林遥公开代价，铜钥匙成为新制度的见证物。"],
            "openQuestions": ["导师是否提前为林遥留下了规避完整代价的方法？"]
        }
    })
    .to_string();
    let mut markdown_by_provider = Vec::new();

    for provider in ProviderId::ALL {
        let server = MockServer::spawn(vec![ResponsePlan::json(
            "200 OK",
            success_body_with_text(provider, &plot_json, false),
        )]);
        let client = ProviderHttpClient::for_test(provider, server.endpoint.clone()).unwrap();
        let request =
            NormalizedRequest::plot_proposal_json("固定主剧情 Prompt".to_owned(), 2_048, schema)
                .unwrap();
        let response = client
            .complete(
                provider,
                b"ng404-plot-canary",
                &request,
                &CancellationToken::default(),
            )
            .unwrap();
        let artifact =
            parse_and_serialize_plot(&response.text, "plot_70000000-0000-4000-8000-000000000007")
                .unwrap();
        assert_eq!(artifact.relative_path, "plot/main.md");
        assert!(artifact.markdown.contains("kind: plot"));
        markdown_by_provider.push(artifact.markdown);

        let recorded = server.finish();
        assert_eq!(recorded.len(), 1);
        let body: Value = serde_json::from_slice(&recorded[0].body).unwrap();
        match provider {
            ProviderId::Deepseek => {
                assert_eq!(body["response_format"], json!({ "type": "json_object" }));
            }
            ProviderId::Openai => {
                assert_eq!(body["text"]["format"]["name"], "plot_proposal_v1");
                assert_eq!(body["text"]["format"]["strict"], true);
                assert_eq!(body["store"], false);
            }
        }
    }

    assert_eq!(markdown_by_provider.len(), 2);
    assert_eq!(markdown_by_provider[0], markdown_by_provider[1]);
}
#[test]
fn common_completion_contract_covers_both_adapters_without_key_or_payload_leakage() {
    for provider in ProviderId::ALL {
        let server = MockServer::spawn(vec![ResponsePlan::json(
            "200 OK",
            success_body(provider, true),
        )]);
        let client = ProviderHttpClient::for_test(provider, server.endpoint.clone()).unwrap();
        let key = format!("ng204-canary-{}", provider.as_str());
        let response = client
            .complete(
                provider,
                key.as_bytes(),
                &request(),
                &CancellationToken::default(),
            )
            .unwrap();
        let recorded = server.finish();

        assert_eq!(response.text, "OK");
        assert_eq!(response.provider_id, provider);
        assert_eq!(response.model, preset(provider).model);
        assert_eq!(response.usage.unwrap().total_tokens, 3);
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].target, "/mock");
        let expected_authorization = format!("Bearer {key}");
        assert!(
            recorded[0].headers.get("authorization") == Some(&expected_authorization),
            "authorization header contract mismatch"
        );
        assert_eq!(
            recorded[0].headers.get("accept").map(String::as_str),
            Some("application/json")
        );
        let body: Value = serde_json::from_slice(&recorded[0].body).unwrap();
        assert_eq!(body["model"], preset(provider).model);
        assert_eq!(body["stream"], false);
        assert!(!String::from_utf8_lossy(&recorded[0].body).contains(&key));
        match provider {
            ProviderId::Deepseek => assert!(body["messages"].is_array()),
            ProviderId::Openai => {
                assert!(body["input"].is_array());
                assert_eq!(body["store"], false);
            }
        }
    }
}

#[test]
fn ng_405_both_loopback_providers_use_one_validator_for_one_confirmed_canon_create() {
    let schema = workflow_resources(CHARACTER_CREATE_WORKFLOW_ID)
        .unwrap()
        .output_schema;
    for provider in ProviderId::ALL {
        let character_json = json!({
            "schemaVersion": 2,
            "workflowId": CHARACTER_CREATE_WORKFLOW_ID,
            "character": {
                "displayName": "林遥",
                "role": "调查记者",
                "summary": "在旧城追查失踪档案的人物。",
                "traits": ["敏锐", "克制"],
                "goal": "找回被删除的证据。",
                "conflict": "公开真相会伤害她最信任的人。",
                "motivations": ["阻止证据再次被删除"],
                "secrets": [],
                "plotFunctions": ["推动失踪档案调查"],
                "relationships": []
            },
            "rationale": "人物目标与核心冲突直接推动主线。"
        })
        .to_string();
        let server = MockServer::spawn(vec![ResponsePlan::json(
            "200 OK",
            success_body_with_text(provider, &character_json, false),
        )]);
        let client = ProviderHttpClient::for_test(provider, server.endpoint.clone()).unwrap();
        let request =
            NormalizedRequest::character_proposal_json("固定人物 Prompt".to_owned(), 512, schema)
                .unwrap();
        let response = client
            .complete(
                provider,
                b"ng306-structured-canary",
                &request,
                &CancellationToken::default(),
            )
            .unwrap();
        assert_eq!(response.finish_reason, FinishReason::Stop);
        let artifact = parse_and_serialize_character_v2_create(
            &response.text,
            "character_50000000-0000-4000-8000-000000000005",
            &[],
        )
        .unwrap();
        assert_eq!(artifact.relative_path, "characters/林遥.md");
        assert_eq!(server.finish().len(), 1);

        let fixture = VerifiedTemporaryRoot::new();
        let project_root = fixture.root.join("project");
        fs::create_dir(&project_root).unwrap();
        let inspection = inspect_project_root(&project_root, ProjectScanMode::Create).unwrap();
        let handle = create_project(&inspection).unwrap();
        let recovery = RecoveryStore::new(fixture.root.join("app-state")).unwrap();
        let proposal = build_canon_proposal(BuildCanonProposalInput {
            proposal_id: Uuid::new_v4().to_string(),
            workflow_id: CHARACTER_CREATE_WORKFLOW_ID.to_owned(),
            workflow_version: 2,
            pack_hash: registry().pack_hash.to_owned(),
            run_id: Uuid::new_v4().to_string(),
            provider_id: provider.as_str().to_owned(),
            preset_version: 1,
            model: preset(provider).model.to_owned(),
            manifest_hash: "a".repeat(64),
            operation_kind: CanonFileOperationKind::Create,
            relative_path: artifact.relative_path.clone(),
            expected_source_hash: None,
            original_content: None,
            proposed_content: artifact.markdown.clone(),
            warning_codes: Vec::new(),
        })
        .unwrap();
        let applied = apply_canon_file_recoverable(
            &handle,
            CanonFileOperationKind::Create,
            &artifact.relative_path,
            "absent",
            proposal.proposed_bytes(),
            &recovery,
        )
        .unwrap();
        assert_eq!(
            applied.content_hash,
            proposal.metadata().operation.proposed_content_hash
        );
        assert_eq!(
            fs::read(project_root.join(&artifact.relative_path)).unwrap(),
            proposal.proposed_bytes()
        );
        let collision = apply_canon_file_recoverable(
            &handle,
            CanonFileOperationKind::Create,
            &artifact.relative_path,
            "absent",
            proposal.proposed_bytes(),
            &recovery,
        )
        .unwrap_err();
        assert_eq!(collision.code, "file.source_changed");
    }
}

#[test]
fn ng_207_high_entropy_authorization_canaries_never_enter_sanitized_mock_evidence() {
    let mut canaries = Vec::new();
    for provider in ProviderId::ALL {
        let key = format!(
            "NG207{}{}{}",
            provider.as_str(),
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let server = MockServer::spawn(vec![ResponsePlan::json(
            "200 OK",
            success_body(provider, true),
        )]);
        let client = ProviderHttpClient::for_test(provider, server.endpoint.clone()).unwrap();
        let response = client
            .complete(
                provider,
                key.as_bytes(),
                &request(),
                &CancellationToken::default(),
            )
            .unwrap();
        let recorded = server.finish();

        assert_eq!(recorded.len(), 1);
        assert_eq!(
            recorded[0].headers.get("authorization"),
            Some(&format!("Bearer {key}"))
        );
        assert!(!String::from_utf8_lossy(&recorded[0].body).contains(&key));
        let evidence = serde_json::to_vec(&json!({
            "providerId": provider,
            "presetVersion": preset(provider).preset_version,
            "model": response.model,
            "result": "passed",
            "requestCount": recorded.len(),
            "canaryLeakCount": 0
        }))
        .unwrap();
        assert!(!String::from_utf8_lossy(&evidence)
            .to_ascii_lowercase()
            .contains("authorization"));
        assert!(!String::from_utf8_lossy(&evidence)
            .to_ascii_lowercase()
            .contains("bearer"));
        for (fragment_index, fragment) in key.as_bytes().windows(16).enumerate() {
            assert!(
                !evidence
                    .windows(fragment.len())
                    .any(|window| window == fragment),
                "sanitized evidence contained Canary fragment {fragment_index}"
            );
        }
        canaries.push(key);
    }
    assert_ne!(canaries[0], canaries[1]);
}

#[test]
fn common_streaming_contract_handles_fragmentation_usage_and_missing_usage() {
    for provider in ProviderId::ALL {
        for include_usage in [true, false] {
            let chunks = fragmented_utf8(stream_payload(provider, include_usage));
            let server =
                MockServer::spawn(vec![ResponsePlan::sse(chunks, Duration::from_millis(15))]);
            let client = ProviderHttpClient::for_test(provider, server.endpoint.clone()).unwrap();
            let mut deltas = Vec::new();
            let mut completed = 0usize;
            let mut usage_events = 0usize;
            let response = client
                .stream(
                    provider,
                    b"ng204-stream-canary",
                    &request(),
                    &CancellationToken::default(),
                    &mut |event| {
                        match event {
                            NormalizedEvent::TextDelta { delta, .. } => deltas.push(delta),
                            NormalizedEvent::Usage { .. } => usage_events += 1,
                            NormalizedEvent::Completed { .. } => completed += 1,
                        }
                        Ok(())
                    },
                )
                .unwrap();
            let recorded = server.finish();

            assert_eq!(deltas.concat(), "你好");
            assert_eq!(response.text, "你好");
            assert_eq!(completed, 1);
            assert_eq!(usage_events, usize::from(include_usage));
            assert_eq!(response.usage.is_some(), include_usage);
            assert_eq!(
                recorded[0].headers.get("accept").map(String::as_str),
                Some("text/event-stream")
            );
            let body: Value = serde_json::from_slice(&recorded[0].body).unwrap();
            assert_eq!(body["stream"], true);
        }
    }
}

#[test]
fn common_cancellation_contract_stops_both_streams_after_the_first_delta() {
    for provider in ProviderId::ALL {
        let payload = stream_payload(provider, true);
        let split = payload.find("\n\n").unwrap() + 2;
        let server = MockServer::spawn(vec![ResponsePlan::sse(
            vec![
                payload.as_bytes()[..split].to_vec(),
                payload.as_bytes()[split..].to_vec(),
            ],
            Duration::from_millis(80),
        )]);
        let client = ProviderHttpClient::for_test(provider, server.endpoint.clone()).unwrap();
        let cancellation = CancellationToken::default();
        let sink_token = cancellation.clone();
        let mut delta_count = 0usize;
        let result = client.stream(
            provider,
            b"ng204-cancel-canary",
            &request(),
            &cancellation,
            &mut |event| {
                if matches!(event, NormalizedEvent::TextDelta { .. }) {
                    delta_count += 1;
                    sink_token.cancel();
                }
                Ok(())
            },
        );
        let _ = server.finish();

        assert_eq!(result.err().unwrap(), ProviderError::cancelled());
        assert_eq!(delta_count, 1);
    }
}

#[test]
fn ng_406_router_one_shot_never_retries_transient_provider_errors() {
    let schema = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/workflows/v1/router/router-decision.schema.json"
    ));
    let request =
        NormalizedRequest::router_decision_json("固定 Router Prompt".to_owned(), 256, schema)
            .unwrap();
    for provider in ProviderId::ALL {
        let server = MockServer::spawn(vec![ResponsePlan::empty("429 Too Many Requests")]);
        let client = ProviderHttpClient::for_test(provider, server.endpoint.clone()).unwrap();
        let result = client.complete_once(
            provider,
            b"ng406-one-shot-canary",
            &request,
            &CancellationToken::default(),
        );
        assert!(matches!(result, Err(error) if error == ProviderError::rate_limited()));
        assert_eq!(server.finish().len(), 1);
    }
}
#[test]
fn common_error_retry_timeout_malformed_and_limit_contracts_cover_both_adapters() {
    for provider in ProviderId::ALL {
        let authentication = MockServer::spawn(vec![ResponsePlan::empty("401 Unauthorized")]);
        let client =
            ProviderHttpClient::for_test(provider, authentication.endpoint.clone()).unwrap();
        assert_eq!(
            client
                .complete(
                    provider,
                    b"ng204-auth-canary",
                    &request(),
                    &CancellationToken::default()
                )
                .err()
                .unwrap(),
            ProviderError::authentication()
        );
        assert_eq!(authentication.finish().len(), 1);

        let retry = MockServer::spawn(vec![
            ResponsePlan::empty("429 Too Many Requests").with_header("Retry-After", "0".to_owned()),
            ResponsePlan::json("200 OK", success_body(provider, true)),
        ]);
        let client = ProviderHttpClient::for_test(provider, retry.endpoint.clone()).unwrap();
        assert!(client
            .complete(
                provider,
                b"ng204-retry-canary",
                &request(),
                &CancellationToken::default()
            )
            .is_ok());
        assert_eq!(retry.finish().len(), 2);

        let unavailable = MockServer::spawn(
            (0..=MAX_RETRIES)
                .map(|_| ResponsePlan::empty("503 Service Unavailable"))
                .collect(),
        );
        let client = ProviderHttpClient::for_test(provider, unavailable.endpoint.clone()).unwrap();
        assert_eq!(
            client
                .complete(
                    provider,
                    b"ng204-503-canary",
                    &request(),
                    &CancellationToken::default()
                )
                .err()
                .unwrap(),
            ProviderError::unreachable()
        );
        assert_eq!(unavailable.finish().len(), usize::from(MAX_RETRIES) + 1);

        let malformed =
            MockServer::spawn(vec![ResponsePlan::json("200 OK", b"{malformed".to_vec())]);
        let client = ProviderHttpClient::for_test(provider, malformed.endpoint.clone()).unwrap();
        assert_eq!(
            client
                .complete(
                    provider,
                    b"ng204-malformed-canary",
                    &request(),
                    &CancellationToken::default()
                )
                .err()
                .unwrap(),
            ProviderError::protocol()
        );
        assert_eq!(malformed.finish().len(), 1);

        let oversized =
            MockServer::spawn(vec![ResponsePlan::json("200 OK", Vec::new())
                .with_declared_length(MAX_RESPONSE_BYTES + 1)]);
        let client = ProviderHttpClient::for_test(provider, oversized.endpoint.clone()).unwrap();
        assert_eq!(
            client
                .complete(
                    provider,
                    b"ng204-limit-canary",
                    &request(),
                    &CancellationToken::default()
                )
                .err()
                .unwrap(),
            ProviderError::response_too_large()
        );
        assert_eq!(oversized.finish().len(), 1);

        let timeout = MockServer::spawn(
            (0..=MAX_RETRIES)
                .map(|_| {
                    ResponsePlan::empty("200 OK")
                        .delayed(Duration::from_millis(TEST_REQUEST_TIMEOUT_MILLIS + 75))
                })
                .collect(),
        );
        let client = ProviderHttpClient::for_test(provider, timeout.endpoint.clone()).unwrap();
        assert_eq!(
            client
                .complete(
                    provider,
                    b"ng204-timeout-canary",
                    &request(),
                    &CancellationToken::default()
                )
                .err()
                .unwrap(),
            ProviderError::timeout()
        );
        assert_eq!(timeout.finish().len(), usize::from(MAX_RETRIES) + 1);

        let wrong_model = MockServer::spawn(vec![ResponsePlan::json(
            "200 OK",
            success_body(
                match provider {
                    ProviderId::Deepseek => ProviderId::Openai,
                    ProviderId::Openai => ProviderId::Deepseek,
                },
                true,
            ),
        )]);
        let client = ProviderHttpClient::for_test(provider, wrong_model.endpoint.clone()).unwrap();
        assert_eq!(
            client
                .complete(
                    provider,
                    b"ng204-model-canary",
                    &request(),
                    &CancellationToken::default()
                )
                .err()
                .unwrap(),
            ProviderError::model_unavailable()
        );
        assert_eq!(wrong_model.finish().len(), 1);
    }
}

#[test]
fn redirects_never_forward_authorization_and_cross_provider_calls_never_broadcast() {
    for provider in ProviderId::ALL {
        let redirect_target = NoRequestSpy::spawn();
        let redirect = MockServer::spawn(vec![ResponsePlan::empty("302 Found")
            .with_header("Location", redirect_target.endpoint.clone())]);
        let client = ProviderHttpClient::for_test(provider, redirect.endpoint.clone()).unwrap();
        assert_eq!(
            client
                .complete(
                    provider,
                    b"ng204-redirect-canary",
                    &request(),
                    &CancellationToken::default()
                )
                .err()
                .unwrap(),
            ProviderError::protocol()
        );
        assert_eq!(redirect.finish().len(), 1);
        assert!(!redirect_target.finish());
    }

    let blocked = [
        (ProviderId::Deepseek, ProviderId::Openai),
        (ProviderId::Openai, ProviderId::Deepseek),
    ];
    for (allowed, attempted) in blocked {
        let spy = NoRequestSpy::spawn();
        let client = ProviderHttpClient::for_test(allowed, spy.endpoint.clone()).unwrap();
        assert_eq!(
            client
                .complete(
                    attempted,
                    b"ng204-cross-provider-canary",
                    &request(),
                    &CancellationToken::default()
                )
                .err()
                .unwrap(),
            ProviderError::invalid_request()
        );
        assert!(!spy.finish());
    }
}
