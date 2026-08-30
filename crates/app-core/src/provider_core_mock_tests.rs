use super::*;
use crate::canon_apply::{
    apply_canon_file_recoverable, apply_chapter_file_recoverable, encode_chapter_target_bytes,
};
use crate::project::{create_project, inspect_project_root, ProjectScanMode};
use crate::recovery::RecoveryStore;
use crate::workspace::{LineEnding, Utf8Bom};
use novelgainker_agent::v2::{AgentControlActionV2, CanonProposalV1, SkillSelectionV2};
use novelgainker_agent::{
    agent_control_action_v2_schema_source, canon_proposal_v1_schema_source,
    skill_selection_v2_schema_source,
};
use novelgainker_workflows::chapter::{build_chapter_proposal, parse_chapter_payload};
use novelgainker_workflows::chapter_lifecycle::{
    build_chapter_status_transition, build_ledger_update, BuildChapterStatusInputV1,
    BuildLedgerUpdateInputV1, ChapterLifecycleStatusV1, STORY_LEDGER_RELATIVE_PATH,
};
use novelgainker_workflows::chapter_patch::{
    build_chapter_patch, BuildChapterPatchInputV1, ChapterPatchOperationV1,
};
use novelgainker_workflows::character_v2::parse_and_serialize_character_v2_create;
use novelgainker_workflows::continuity::{
    check_story_continuity, confirm_story_continuity_warnings, ContinuityCheckInputV1,
    ContinuitySeverityV1, ContinuitySourceV1, ContinuityTargetStateV1,
};
use novelgainker_workflows::material::parse_and_serialize_material;
use novelgainker_workflows::plot::parse_and_serialize_plot;
use novelgainker_workflows::proposal::{
    build_canon_proposal, BuildCanonProposalInput, CanonFileOperationKind,
};
use novelgainker_workflows::setting::parse_and_serialize_setting;
use novelgainker_workflows::story_context::{
    build_story_context_manifest, build_story_context_request_snapshot, sha256_hex,
    BuildStoryContextInputV2, StoryActionV2, StoryContextSelectionReasonV2,
    StoryContextSourceInputV2, StoryContextSourceKindV2, StoryReaderMentalModelV1,
};
use novelgainker_workflows::{
    registry, workflow_resources, CHARACTER_CREATE_WORKFLOW_ID, MATERIAL_CREATE_WORKFLOW_ID,
    PLOT_CREATE_WORKFLOW_ID, SETTING_CREATE_WORKFLOW_ID, STORYTELLING_WORKFLOW_ID,
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

const LEGACY_STORY_LEDGER_FIXTURE: &str = "---\ntitle: 故事状态账本\nwritingLanguage: zh-CN\nstatus: active\n---\n\n# 故事状态账本\n\n在这里记录时间线、角色状态、线索、承诺与回收等连续性事实。\n";

fn install_legacy_story_ledger_fixture(project_root: &std::path::Path) {
    fs::write(
        project_root.join(STORY_LEDGER_RELATIVE_PATH),
        LEGACY_STORY_LEDGER_FIXTURE.as_bytes(),
    )
    .expect("legacy Story Ledger kernel fixture must be installed explicitly");
}

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

    fn json_chunks(chunks: Vec<Vec<u8>>, between_chunks: Duration) -> Self {
        Self {
            status: "200 OK",
            headers: vec![("Content-Type".to_owned(), "application/json".to_owned())],
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
fn gar1_v2_dual_provider_loopback_separates_skill_control_and_conversation() {
    let selection_text = json!({
        "schemaVersion": 2,
        "requestId": "req_gar1selection00000001",
        "selectedSkillIds": ["novel-story-telling"],
        "intentClass": "project_discussion",
        "visibleReason": "需要读取当前人物设定。"
    })
    .to_string();
    let action_text = json!({
        "schemaVersion": 2,
        "requestId": "req_gar1control000000001",
        "action": "start_response",
        "selectedSkillIds": ["novel-story-telling"],
        "visibleReason": "已有足够来源，可以回复。",
        "responseBrief": "回答作者关于林遥身份的问题。",
        "sourceIds": ["src_gar1source0000000001"],
        "markSuggestions": []
    })
    .to_string();
    let conversation_text = "林遥是潮汐记录员，她负责追踪雾港的异常退潮。";
    for provider in ProviderId::ALL {
        let server = MockServer::spawn(vec![
            ResponsePlan::json(
                "200 OK",
                success_body_with_text(provider, &selection_text, true),
            ),
            ResponsePlan::json(
                "200 OK",
                success_body_with_text(provider, &action_text, true),
            ),
            ResponsePlan::json(
                "200 OK",
                success_body_with_text(provider, conversation_text, true),
            ),
        ]);
        let client = ProviderHttpClient::for_test(provider, server.endpoint.clone()).unwrap();
        let cancellation = CancellationToken::default();
        let key = b"gar1-loopback-key-canary";

        let selection_request = NormalizedRequest::agent_v2_skill_selection(
            "林遥是谁？".to_owned(),
            "novel-story-telling：整部小说叙事与连续性".to_owned(),
            "req_gar1selection00000001",
            512,
            skill_selection_v2_schema_source(),
        )
        .unwrap();
        let selection_response = client
            .complete(provider, key, &selection_request, &cancellation)
            .unwrap();
        let selection: SkillSelectionV2 = serde_json::from_str(&selection_response.text).unwrap();
        selection.validate("req_gar1selection00000001").unwrap();

        let control_request = NormalizedRequest::agent_v2_control(
            "林遥是谁？".to_owned(),
            "完整 Skill instruction/template/reference".to_owned(),
            "sourceIds=[src_gar1source0000000001]".to_owned(),
            "req_gar1control000000001",
            768,
            agent_control_action_v2_schema_source(),
        )
        .unwrap();
        let control_response = client
            .complete(provider, key, &control_request, &cancellation)
            .unwrap();
        let action: AgentControlActionV2 = serde_json::from_str(&control_response.text).unwrap();
        action
            .validate(
                "req_gar1control000000001",
                &["novel-story-telling".to_owned()],
            )
            .unwrap();

        let conversation_request = NormalizedRequest::agent_v2_conversation(
            vec![NormalizedMessage {
                role: MessageRole::User,
                content: "林遥是谁？".to_owned(),
            }],
            "完整 Skill instruction/template/reference".to_owned(),
            "characters/林遥.md 的 Hash 绑定片段".to_owned(),
            1_024,
        )
        .unwrap();
        let conversation_response = client
            .complete(provider, key, &conversation_request, &cancellation)
            .unwrap();
        assert_eq!(conversation_response.text, conversation_text);

        let recorded = server.finish();
        assert_eq!(recorded.len(), 3);
        let bodies = recorded
            .iter()
            .map(|request| String::from_utf8_lossy(&request.body).to_string())
            .collect::<Vec<_>>();
        assert!(bodies
            .iter()
            .all(|body| !body.contains("gar1-loopback-key-canary")));
        assert!(!bodies[1].contains(conversation_text));
        assert!(!bodies[2].contains("\"action\":\"start_response\""));
        match provider {
            ProviderId::Deepseek => {
                assert!(bodies[0].contains("json_object"));
                assert!(bodies[1].contains("json_object"));
                assert!(!bodies[2].contains("json_object"));
            }
            ProviderId::Openai => {
                assert!(bodies[0].contains("skill_selection_v2"));
                assert!(bodies[1].contains("agent_control_action_v2"));
                assert!(!bodies[2].contains("json_schema"));
            }
        }
    }
}

#[test]
fn gar2_dual_provider_loopback_uses_one_source_bound_canon_reconciliation_contract() {
    let request_id = "req_gar2reconcile0000001";
    let source_id = "art_gar2artifact00000001";
    let source_hash = "a".repeat(64);
    let proposal_text = json!({
        "schemaVersion": 1,
        "requestId": request_id,
        "proposalId": "prop_gar2proposal0000001",
        "targetKind": "character",
        "displayName": "林舟",
        "operation": "create",
        "facts": [{
            "field": "身份",
            "value": "守灯人",
            "sources": [{
                "kind": "draft_artifact",
                "sourceId": source_id,
                "sourceHash": source_hash
            }]
        }]
    })
    .to_string();
    for provider in ProviderId::ALL {
        let server = MockServer::spawn(vec![ResponsePlan::json(
            "200 OK",
            success_body_with_text(provider, &proposal_text, true),
        )]);
        let client = ProviderHttpClient::for_test(provider, server.endpoint.clone()).unwrap();
        let request = NormalizedRequest::agent_v2_reconciliation(
            "作者已确认：主角就叫林舟。".to_owned(),
            "create-character 完整资源".to_owned(),
            format!("sourceId={source_id}; sourceHash={source_hash}"),
            request_id,
            1_024,
            canon_proposal_v1_schema_source(),
        )
        .unwrap();
        let response = client
            .complete(
                provider,
                b"gar2-reconciliation-loopback-canary",
                &request,
                &CancellationToken::default(),
            )
            .unwrap();
        let proposal: CanonProposalV1 = serde_json::from_str(&response.text).unwrap();
        proposal
            .validate(request_id, &[(source_id.to_owned(), source_hash.clone())])
            .unwrap();
        let recorded = server.finish();
        assert_eq!(recorded.len(), 1);
        let body = String::from_utf8_lossy(&recorded[0].body);
        assert!(!body.contains("gar2-reconciliation-loopback-canary"));
        assert!(!body.contains("characters/林舟.md"));
        match provider {
            ProviderId::Deepseek => assert!(body.contains("json_object")),
            ProviderId::Openai => assert!(body.contains("canon_proposal_v1")),
        }
    }
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
fn ng_503_chapter_proposal_is_provider_neutral_across_both_loopback_providers() {
    let schema = novelgainker_workflows::storytelling_chapter_proposal_output_schema();
    let source_id = "project:plot/main.md".to_owned();
    let payload = json!({
        "schemaVersion": 1,
        "title": "潮痕",
        "draftMarkdown": "潮水退去，石阶上留下发光的纹路。",
        "intentSummary": "推进潮门异响线索。",
        "readerModelDelta": {
            "readerKnows": "读者看见了发光纹路。",
            "readerInfers": "纹路可能与潮门有关。",
            "readerMisreads": "",
            "readerQuestions": "谁留下了纹路？",
            "chapterPromises": "下一章继续追查纹路。",
            "forbiddenReveals": ""
        },
        "continuityClaims": [{"claim": "潮门已关闭", "sourceIds": [source_id]}],
        "ledgerCandidates": [{"summary": "石阶出现发光纹路", "sourceIds": [source_id]}]
    })
    .to_string();
    let mut parsed_payloads = Vec::new();
    for provider in ProviderId::ALL {
        let server = MockServer::spawn(vec![ResponsePlan::json(
            "200 OK",
            success_body_with_text(provider, &payload, false),
        )]);
        let client = ProviderHttpClient::for_test(provider, server.endpoint.clone()).unwrap();
        let request = NormalizedRequest::chapter_proposal_json(
            vec![
                "固定章节任务与 Reader Model".to_owned(),
                format!("sourceId: {source_id}\n潮门已经关闭。"),
            ],
            4_096,
            schema,
        )
        .unwrap();
        let response = client
            .complete(
                provider,
                b"ng503-chapter-key-canary",
                &request,
                &CancellationToken::default(),
            )
            .unwrap();
        parsed_payloads.push(
            serde_json::to_vec(
                &parse_chapter_payload(&response.text, true, std::slice::from_ref(&source_id))
                    .unwrap(),
            )
            .unwrap(),
        );
        let recorded = server.finish();
        assert_eq!(recorded.len(), 1);
        let body: Value = serde_json::from_slice(&recorded[0].body).unwrap();
        let serialized = serde_json::to_vec(&body).unwrap();
        assert!(!serialized
            .windows(b"ng503-chapter-key-canary".len())
            .any(|window| window == b"ng503-chapter-key-canary"));
        match provider {
            ProviderId::Deepseek => {
                assert_eq!(body["response_format"], json!({ "type": "json_object" }));
            }
            ProviderId::Openai => {
                assert_eq!(body["text"]["format"]["name"], "chapter_proposal_v1");
                assert_eq!(body["text"]["format"]["strict"], true);
                assert_eq!(body["store"], false);
            }
        }
    }
    assert_eq!(parsed_payloads.len(), 2);
    assert_eq!(parsed_payloads[0], parsed_payloads[1]);
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
fn ga2_r005_cancellable_completion_aborts_before_response_headers_without_retrying() {
    for provider in ProviderId::ALL {
        let server = MockServer::spawn(vec![ResponsePlan::json(
            "200 OK",
            success_body(provider, true),
        )
        .delayed(Duration::from_millis(TEST_REQUEST_TIMEOUT_MILLIS + 75))]);
        let client = ProviderHttpClient::for_test(provider, server.endpoint.clone()).unwrap();
        let cancellation = CancellationToken::default();
        let cancel_signal = cancellation.clone();
        let cancel_thread = thread::spawn(move || {
            thread::sleep(Duration::from_millis(25));
            cancel_signal.cancel();
        });
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let started = Instant::now();
        let result = runtime.block_on(client.complete_cancellable(
            provider,
            b"ga2-r005-cancel-canary",
            &request(),
            &cancellation,
        ));
        let elapsed = started.elapsed();
        cancel_thread.join().unwrap();
        let recorded = server.finish();

        assert!(matches!(result, Err(error) if error == ProviderError::cancelled()));
        assert!(
            elapsed < Duration::from_millis(150),
            "cancellation took {elapsed:?} for {provider:?}"
        );
        assert_eq!(recorded.len(), 1, "cancelled requests must never retry");
    }
}

#[test]
fn ga2_r005_cancellable_completion_aborts_while_response_body_is_stalled() {
    for provider in ProviderId::ALL {
        let body = success_body(provider, true);
        let split = body.len() / 2;
        let server = MockServer::spawn(vec![ResponsePlan::json_chunks(
            vec![body[..split].to_vec(), body[split..].to_vec()],
            Duration::from_millis(TEST_REQUEST_TIMEOUT_MILLIS + 75),
        )]);
        let client = ProviderHttpClient::for_test(provider, server.endpoint.clone()).unwrap();
        let cancellation = CancellationToken::default();
        let cancel_signal = cancellation.clone();
        let cancel_thread = thread::spawn(move || {
            thread::sleep(Duration::from_millis(25));
            cancel_signal.cancel();
        });
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let started = Instant::now();
        let result = runtime.block_on(client.complete_cancellable(
            provider,
            b"ga2-r005-body-cancel-canary",
            &request(),
            &cancellation,
        ));
        let elapsed = started.elapsed();
        cancel_thread.join().unwrap();
        let recorded = server.finish();

        assert!(matches!(result, Err(error) if error == ProviderError::cancelled()));
        assert!(
            elapsed < Duration::from_millis(150),
            "body cancellation took {elapsed:?} for {provider:?}"
        );
        assert_eq!(recorded.len(), 1, "cancelled body reads must never retry");
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

fn ng507_source(
    source_id: &str,
    kind: StoryContextSourceKindV2,
    priority: u16,
    content: &str,
    modified_time_ms: u64,
) -> StoryContextSourceInputV2 {
    let builtin = source_id.starts_with("builtin:");
    StoryContextSourceInputV2 {
        source_id: source_id.to_owned(),
        kind,
        relative_path: (!builtin).then(|| "plot/main.md".to_owned()),
        builtin_resource_id: builtin.then(|| "storytelling/workflow.md".to_owned()),
        display_name: source_id.to_owned(),
        selection_reason: if builtin {
            StoryContextSelectionReasonV2::Required
        } else {
            StoryContextSelectionReasonV2::Explicit
        },
        required: builtin,
        user_deselectable: !builtin,
        priority,
        content: content.to_owned(),
        content_hash: sha256_hex(content.as_bytes()),
        source_byte_count: content.len() as u64,
        modified_time_ms,
        included: true,
    }
}

fn ng507_context(
    action: StoryActionV2,
    modified_time_ms: u64,
) -> (
    novelgainker_workflows::story_context::StoryContextRequestSnapshotV1,
    Vec<ContinuitySourceV1>,
    Vec<String>,
) {
    let protocol = "固定 Storytelling 协议：只生成提案，不直接写文件。";
    let outline = "第一章《潮痕》：林遥在退潮石阶发现发光纹路，并决定继续追查潮门。";
    let manifest = build_story_context_manifest(BuildStoryContextInputV2 {
        workflow_id: STORYTELLING_WORKFLOW_ID.to_owned(),
        project_root_identity: "ng507-project-fixture".to_owned(),
        action,
        chapter_number: 1,
        chapter_title: "潮痕".to_owned(),
        sources: vec![
            ng507_source(
                "builtin:storytelling-workflow",
                StoryContextSourceKindV2::BuiltinProtocol,
                1,
                protocol,
                modified_time_ms,
            ),
            ng507_source(
                "project:plot/main.md",
                StoryContextSourceKindV2::Outline,
                20,
                outline,
                modified_time_ms,
            ),
        ],
    })
    .unwrap();
    let reviewed_manifest_hash = manifest.aggregate_hash.clone();
    let snapshot = build_story_context_request_snapshot(
        manifest,
        reviewed_manifest_hash,
        StoryReaderMentalModelV1 {
            reader_knows: "林遥知道潮门已经关闭。".to_owned(),
            reader_infers: "潮门仍在影响旧城。".to_owned(),
            reader_misreads: String::new(),
            reader_questions: "发光纹路是谁留下的？".to_owned(),
            chapter_promises: String::new(),
            forbidden_reveals: String::new(),
        },
    )
    .unwrap();
    let continuity_sources = vec![ContinuitySourceV1 {
        source_id: "project:plot/main.md".to_owned(),
        kind: StoryContextSourceKindV2::Outline,
        content: outline.to_owned(),
    }];
    let allowed_source_ids = snapshot.manifest.source_order.clone();
    (snapshot, continuity_sources, allowed_source_ids)
}

fn ng507_payload(draft_markdown: &str) -> String {
    json!({
        "schemaVersion": 1,
        "title": "潮痕",
        "draftMarkdown": draft_markdown,
        "intentSummary": "推进潮门异响与发光纹路线索。",
        "readerModelDelta": {
            "readerKnows": "读者看见林遥发现了发光纹路。",
            "readerInfers": "纹路可能与潮门有关。",
            "readerMisreads": "",
            "readerQuestions": "谁留下了纹路？",
            "chapterPromises": "下一章继续追查纹路。",
            "forbiddenReveals": ""
        },
        "continuityClaims": [{
            "claim": "潮门已经关闭",
            "sourceIds": ["project:plot/main.md"]
        }],
        "ledgerCandidates": [{
            "summary": "石阶出现发光纹路",
            "sourceIds": ["project:plot/main.md"]
        }]
    })
    .to_string()
}

fn ng507_request(
    snapshot: &novelgainker_workflows::story_context::StoryContextRequestSnapshotV1,
) -> NormalizedRequest {
    NormalizedRequest::chapter_proposal_json(
        vec![
            format!(
                "固定动作={:?}; manifestHash={}; requestHash={}",
                snapshot.manifest.action, snapshot.manifest.aggregate_hash, snapshot.request_hash
            ),
            "sourceId: project:plot/main.md\n潮门已经关闭。".to_owned(),
        ],
        4_096,
        novelgainker_workflows::storytelling_chapter_proposal_output_schema(),
    )
    .unwrap()
}

fn ng507_confirmed_report(
    proposal: &novelgainker_workflows::chapter::ChapterProposalV1,
    snapshot: &novelgainker_workflows::story_context::StoryContextRequestSnapshotV1,
    sources: &[ContinuitySourceV1],
    target: &ContinuityTargetStateV1,
    checked_at: u64,
) -> novelgainker_workflows::continuity::StoryContinuityReportV1 {
    let mut report = check_story_continuity(ContinuityCheckInputV1 {
        proposal,
        expected_action: snapshot.manifest.action,
        expected_target: &snapshot.manifest.target_chapter,
        reader_model: &snapshot.reader_mental_model,
        current_snapshot: Some(snapshot),
        sources,
        generation_target: target,
        current_target: target,
        checked_at,
    });
    let warning_ids = report
        .results
        .iter()
        .filter(|item| item.severity == ContinuitySeverityV1::Warning)
        .map(|item| item.rule_id.clone())
        .collect::<Vec<_>>();
    let unknown_ids = report
        .results
        .iter()
        .filter(|item| item.severity == ContinuitySeverityV1::Unknown)
        .map(|item| item.rule_id.clone())
        .collect::<Vec<_>>();
    assert_eq!(unknown_ids.len(), 2);
    assert!(confirm_story_continuity_warnings(
        &mut report,
        &warning_ids,
        &unknown_ids,
    ));
    assert!(report.review_ready);
    assert_eq!(report.summary.unknown, 2);
    assert!(!report.write_enabled);
    report
}

fn ng507_target(content: &str) -> ContinuityTargetStateV1 {
    ContinuityTargetStateV1 {
        available: true,
        exists: true,
        content_hash: Some(sha256_hex(content.as_bytes())),
        content: Some(content.to_owned()),
    }
}

fn ng507_chapter_from_provider(
    client: &ProviderHttpClient,
    provider: ProviderId,
    key: &[u8],
    snapshot: &novelgainker_workflows::story_context::StoryContextRequestSnapshotV1,
    allowed_source_ids: &[String],
    started_at: u64,
    generated_at: u64,
) -> (
    novelgainker_workflows::chapter::ChapterProposalPayloadV1,
    novelgainker_workflows::chapter::ChapterProposalV1,
) {
    let response = client
        .complete(
            provider,
            key,
            &ng507_request(snapshot),
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(response.finish_reason, FinishReason::Stop);
    let payload = parse_chapter_payload(&response.text, true, allowed_source_ids).unwrap();
    let proposal = build_chapter_proposal(
        snapshot,
        payload.clone(),
        snapshot.manifest.aggregate_hash.clone(),
        provider.as_str().to_owned(),
        preset(provider).model.to_owned(),
        started_at,
        generated_at,
        "stop".to_owned(),
    )
    .unwrap();
    (payload, proposal)
}

#[test]
fn ng_507_both_loopback_providers_complete_the_storytelling_transaction_state_machine() {
    let key = format!(
        "NG507{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    );
    let create_payload = ng507_payload("潮水退去，石阶上留下发光的纹路。林遥俯身记录纹路走向。");
    let revise_payload =
        ng507_payload("潮水退去，石阶上留下发光的纹路。林遥沿着纹路走到封闭潮门前，听见门后回声。");

    for provider in ProviderId::ALL {
        let server = MockServer::spawn(vec![
            ResponsePlan::json(
                "200 OK",
                success_body_with_text(provider, &create_payload, true),
            ),
            ResponsePlan::json(
                "200 OK",
                success_body_with_text(provider, &revise_payload, true),
            ),
            ResponsePlan::json(
                "200 OK",
                success_body_with_text(provider, &revise_payload, true),
            ),
        ]);
        let client = ProviderHttpClient::for_test(provider, server.endpoint.clone()).unwrap();
        let fixture = VerifiedTemporaryRoot::new();
        let project_root = fixture.root.join("project");
        fs::create_dir(&project_root).unwrap();
        let inspection = inspect_project_root(&project_root, ProjectScanMode::Create).unwrap();
        let handle = create_project(&inspection).unwrap();
        install_legacy_story_ledger_fixture(&project_root);
        let recovery = RecoveryStore::new(fixture.root.join("app-state")).unwrap();

        let (create_snapshot, create_sources, create_allowed_ids) =
            ng507_context(StoryActionV2::Create, 1_000);
        let (create_output, create_proposal) = ng507_chapter_from_provider(
            &client,
            provider,
            key.as_bytes(),
            &create_snapshot,
            &create_allowed_ids,
            10,
            11,
        );
        let missing = ContinuityTargetStateV1::missing();
        let create_report = ng507_confirmed_report(
            &create_proposal,
            &create_snapshot,
            &create_sources,
            &missing,
            12,
        );
        let create_patch = build_chapter_patch(BuildChapterPatchInputV1 {
            proposal: &create_proposal,
            continuity_report: &create_report,
            current_content: None,
            current_content_hash: None,
            accepted_or_final_unlock_confirmed: false,
            create_chapter_id: Some(
                Uuid::parse_str("51000000-0000-4000-8000-000000000001").unwrap(),
            ),
        })
        .unwrap();
        let create_bytes = encode_chapter_target_bytes(
            &create_patch.target_content,
            Utf8Bom::None,
            LineEnding::Lf,
        )
        .unwrap();
        let create_applied = apply_chapter_file_recoverable(
            &handle,
            ChapterPatchOperationV1::Create,
            &create_patch.preview.target_relative_path,
            None,
            &create_bytes,
            &recovery,
        )
        .unwrap();
        let chapter_path = project_root.join(&create_patch.preview.target_relative_path);
        let created_content = fs::read_to_string(&chapter_path).unwrap();
        assert_eq!(create_applied.file.content_hash, sha256_hex(&create_bytes));

        let (revise_snapshot, revise_sources, revise_allowed_ids) =
            ng507_context(StoryActionV2::Revise, 2_000);
        let (_, stale_revise_proposal) = ng507_chapter_from_provider(
            &client,
            provider,
            key.as_bytes(),
            &revise_snapshot,
            &revise_allowed_ids,
            20,
            21,
        );
        let created_target = ng507_target(&created_content);
        let stale_revise_report = ng507_confirmed_report(
            &stale_revise_proposal,
            &revise_snapshot,
            &revise_sources,
            &created_target,
            22,
        );
        let stale_revise_patch = build_chapter_patch(BuildChapterPatchInputV1 {
            proposal: &stale_revise_proposal,
            continuity_report: &stale_revise_report,
            current_content: Some(&created_content),
            current_content_hash: created_target.content_hash.as_deref(),
            accepted_or_final_unlock_confirmed: false,
            create_chapter_id: None,
        })
        .unwrap();
        let stale_revise_bytes = encode_chapter_target_bytes(
            &stale_revise_patch.target_content,
            Utf8Bom::None,
            LineEnding::Lf,
        )
        .unwrap();

        let mut manual_content = created_content.clone();
        manual_content.push_str("\n作者手动补记：潮声比先前更近。\n");
        fs::write(&chapter_path, manual_content.as_bytes()).unwrap();
        let stale_error = apply_chapter_file_recoverable(
            &handle,
            ChapterPatchOperationV1::Replace,
            &stale_revise_patch.preview.target_relative_path,
            stale_revise_patch.preview.expected_source_hash.as_deref(),
            &stale_revise_bytes,
            &recovery,
        )
        .unwrap_err();
        assert_eq!(stale_error.code, "file.source_changed");
        assert_eq!(fs::read_to_string(&chapter_path).unwrap(), manual_content);

        let (_, fresh_revise_proposal) = ng507_chapter_from_provider(
            &client,
            provider,
            key.as_bytes(),
            &revise_snapshot,
            &revise_allowed_ids,
            30,
            31,
        );
        let manual_target = ng507_target(&manual_content);
        let fresh_revise_report = ng507_confirmed_report(
            &fresh_revise_proposal,
            &revise_snapshot,
            &revise_sources,
            &manual_target,
            32,
        );
        let revise_patch = build_chapter_patch(BuildChapterPatchInputV1 {
            proposal: &fresh_revise_proposal,
            continuity_report: &fresh_revise_report,
            current_content: Some(&manual_content),
            current_content_hash: manual_target.content_hash.as_deref(),
            accepted_or_final_unlock_confirmed: false,
            create_chapter_id: None,
        })
        .unwrap();
        let revise_bytes = encode_chapter_target_bytes(
            &revise_patch.target_content,
            Utf8Bom::None,
            LineEnding::Lf,
        )
        .unwrap();
        let revise_applied = apply_chapter_file_recoverable(
            &handle,
            ChapterPatchOperationV1::Replace,
            &revise_patch.preview.target_relative_path,
            revise_patch.preview.expected_source_hash.as_deref(),
            &revise_bytes,
            &recovery,
        )
        .unwrap();
        let revised_content = fs::read_to_string(&chapter_path).unwrap();

        let revised_hash = sha256_hex(revised_content.as_bytes());
        let status_document = build_chapter_status_transition(BuildChapterStatusInputV1 {
            target_relative_path: &revise_patch.preview.target_relative_path,
            current_content: &revised_content,
            current_content_hash: &revised_hash,
            current_content_has_utf8_bom: false,
            next_status: ChapterLifecycleStatusV1::Accepted,
            proposal: Some(&fresh_revise_proposal),
            pending_id: Some(Uuid::parse_str("52000000-0000-4000-8000-000000000002").unwrap()),
        })
        .unwrap();
        let status_bytes = encode_chapter_target_bytes(
            &status_document.target_content,
            Utf8Bom::None,
            LineEnding::Lf,
        )
        .unwrap();
        let status_applied = apply_chapter_file_recoverable(
            &handle,
            ChapterPatchOperationV1::Replace,
            &status_document.preview.target_relative_path,
            Some(&status_document.preview.expected_source_hash),
            &status_bytes,
            &recovery,
        )
        .unwrap();
        let accepted_content = fs::read_to_string(&chapter_path).unwrap();
        assert!(accepted_content.contains("status: accepted"));
        assert!(accepted_content.contains("ledger_pending_"));

        let ledger_path = project_root.join(STORY_LEDGER_RELATIVE_PATH);
        let ledger_before = fs::read_to_string(&ledger_path).unwrap();
        let ledger_before_hash = sha256_hex(ledger_before.as_bytes());
        let accepted_hash = sha256_hex(accepted_content.as_bytes());
        let ledger_document = build_ledger_update(BuildLedgerUpdateInputV1 {
            chapter_relative_path: &revise_patch.preview.target_relative_path,
            chapter_content: &accepted_content,
            chapter_content_hash: &accepted_hash,
            chapter_content_has_utf8_bom: false,
            ledger_content: &ledger_before,
            ledger_content_hash: &ledger_before_hash,
            ledger_content_has_utf8_bom: false,
        })
        .unwrap();
        let ledger_bytes = encode_chapter_target_bytes(
            &ledger_document.target_content,
            Utf8Bom::None,
            LineEnding::Lf,
        )
        .unwrap();
        let ledger_applied = apply_chapter_file_recoverable(
            &handle,
            ChapterPatchOperationV1::Replace,
            STORY_LEDGER_RELATIVE_PATH,
            Some(&ledger_document.preview.expected_source_hash),
            &ledger_bytes,
            &recovery,
        )
        .unwrap();
        let ledger_after = fs::read(&ledger_path).unwrap();

        let recorded = server.finish();
        assert_eq!(recorded.len(), 3);
        for request in recorded {
            assert!(!String::from_utf8_lossy(&request.body).contains(&key));
        }
        println!(
            "NG507_EVIDENCE:{}",
            json!({
                "schemaVersion": 1,
                "providerId": provider.as_str(),
                "modelId": preset(provider).model,
                "stateMachineVersion": 1,
                "stateSequence": [
                    "context", "proposal", "continuity", "patch-preview", "chapter-create",
                    "manual-edit", "stale-reject", "proposal-refresh", "chapter-revise",
                    "status-accepted", "ledger-preview", "ledger-apply"
                ],
                "manifestHash": create_snapshot.manifest.aggregate_hash,
                "requestHash": create_snapshot.request_hash,
                "outputHash": sha256_hex(&serde_json::to_vec(&create_output).unwrap()),
                "createProposalHash": create_proposal.proposal_hash,
                "reviseProposalHash": fresh_revise_proposal.proposal_hash,
                "createdChapterHash": create_applied.file.content_hash,
                "manualChapterHash": sha256_hex(manual_content.as_bytes()),
                "revisedChapterHash": revise_applied.file.content_hash,
                "acceptedChapterHash": status_applied.file.content_hash,
                "ledgerHash": ledger_applied.file.content_hash,
                "ledgerBytesHash": sha256_hex(&ledger_after),
                "continuityUnknownCount": fresh_revise_report.summary.unknown,
                "unknownsConfirmed": fresh_revise_report.unknowns_confirmed,
                "providerRequestCount": 3,
                "automaticProjectWriteCount": 0,
                "statusConfirmationCount": 1,
                "ledgerConfirmationCount": 1,
                "staleOverwriteCount": 0
            })
        );
    }
}

#[test]
fn ng_507_both_loopback_providers_cancel_truncate_error_and_stale_sources_without_writes() {
    let key = format!(
        "NG507{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    );
    for provider in ProviderId::ALL {
        let fixture = VerifiedTemporaryRoot::new();
        let project_root = fixture.root.join("project");
        fs::create_dir(&project_root).unwrap();
        let inspection = inspect_project_root(&project_root, ProjectScanMode::Create).unwrap();
        let _handle = create_project(&inspection).unwrap();
        install_legacy_story_ledger_fixture(&project_root);
        let chapter_path = project_root.join("chapters/001.md");
        let ledger_path = project_root.join(STORY_LEDGER_RELATIVE_PATH);
        let ledger_before = fs::read(&ledger_path).unwrap();
        let (snapshot, sources, allowed_source_ids) = ng507_context(StoryActionV2::Create, 3_000);

        let stream = stream_payload(provider, true);
        let split = stream.find("\n\n").unwrap() + 2;
        let cancel_server = MockServer::spawn(vec![ResponsePlan::sse(
            vec![
                stream.as_bytes()[..split].to_vec(),
                stream.as_bytes()[split..].to_vec(),
            ],
            Duration::from_millis(80),
        )]);
        let client =
            ProviderHttpClient::for_test(provider, cancel_server.endpoint.clone()).unwrap();
        let cancellation = CancellationToken::default();
        let sink_token = cancellation.clone();
        let mut delta_count = 0usize;
        let cancelled = client.stream(
            provider,
            key.as_bytes(),
            &ng507_request(&snapshot),
            &cancellation,
            &mut |event| {
                if matches!(event, NormalizedEvent::TextDelta { .. }) {
                    delta_count += 1;
                    sink_token.cancel();
                }
                Ok(())
            },
        );
        let cancel_requests = cancel_server.finish();
        assert!(matches!(
            cancelled,
            Err(error) if error == ProviderError::cancelled()
        ));
        assert_eq!(delta_count, 1);
        assert_eq!(cancel_requests.len(), 1);

        let complete_payload = ng507_payload("潮水退去，纹路短暂发光。");
        assert!(parse_chapter_payload(&complete_payload, false, &allowed_source_ids).is_err());
        assert!(parse_chapter_payload("{\"schemaVersion\":1", true, &allowed_source_ids).is_err());

        let error_server = MockServer::spawn(vec![ResponsePlan::empty("429 Too Many Requests")]);
        let error_client =
            ProviderHttpClient::for_test(provider, error_server.endpoint.clone()).unwrap();
        assert!(matches!(
            error_client.complete_once(
                provider,
                key.as_bytes(),
                &ng507_request(&snapshot),
                &CancellationToken::default(),
            ),
            Err(error) if error == ProviderError::rate_limited()
        ));
        assert_eq!(error_server.finish().len(), 1);

        let payload = parse_chapter_payload(&complete_payload, true, &allowed_source_ids).unwrap();
        let stale_proposal = build_chapter_proposal(
            &snapshot,
            payload,
            "f".repeat(64),
            provider.as_str().to_owned(),
            preset(provider).model.to_owned(),
            40,
            41,
            "stop".to_owned(),
        )
        .unwrap();
        let missing = ContinuityTargetStateV1::missing();
        let stale_report = check_story_continuity(ContinuityCheckInputV1 {
            proposal: &stale_proposal,
            expected_action: StoryActionV2::Create,
            expected_target: &snapshot.manifest.target_chapter,
            reader_model: &snapshot.reader_mental_model,
            current_snapshot: Some(&snapshot),
            sources: &sources,
            generation_target: &missing,
            current_target: &missing,
            checked_at: 42,
        });
        assert!(stale_report.summary.blocking > 0);
        assert!(!stale_report.review_ready);

        assert!(!chapter_path.exists());
        assert_eq!(fs::read(&ledger_path).unwrap(), ledger_before);
        println!(
            "NG507_NEGATIVE:{}",
            json!({
                "schemaVersion": 1,
                "providerId": provider.as_str(),
                "cancelledAfterDeltaCount": 1,
                "cancelledProjectWriteCount": 0,
                "truncatedOutputWriteCount": 0,
                "invalidJsonWriteCount": 0,
                "providerErrorWriteCount": 0,
                "sourceStaleWriteCount": 0,
                "chapterExists": false,
                "ledgerUnchanged": true
            })
        );
    }
}
