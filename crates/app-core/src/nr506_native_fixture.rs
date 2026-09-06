use super::*;
use std::io::{Read, Write};
use std::net::TcpStream;

pub enum Plan {
    Chat,
    Failure {
        partial: bool,
    },
    InvalidTool,
    Create {
        kind: &'static str,
        title: String,
        text: String,
        chapter: Option<u16>,
        chapter_source: Option<String>,
        risk: &'static str,
        final_error: bool,
    },
    Read {
        capability: &'static str,
        paths: Vec<String>,
    },
    Publish {
        failure: bool,
        final_error: bool,
    },
}

fn accept(listener: &TcpListener) -> TcpStream {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                return stream;
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline =>
            {
                thread::sleep(Duration::from_millis(5))
            }
            Err(error) => panic!("missing isolated HTTP request: {error}"),
        }
    }
}
fn request(stream: &mut TcpStream) -> Value {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = stream.read(&mut chunk).unwrap();
        assert!(n > 0);
        bytes.extend_from_slice(&chunk[..n]);
        assert!(bytes.len() <= 768 * 1024 + 8192);
        let Some(header) = bytes.windows(4).position(|v| v == b"\r\n\r\n") else {
            continue;
        };
        assert!(header <= 8192);
        let head = std::str::from_utf8(&bytes[..header]).unwrap();
        let length = head
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().unwrap())
            })
            .unwrap();
        assert!(length <= 768 * 1024);
        if bytes.len() >= header + 4 + length {
            return serde_json::from_slice(&bytes[header + 4..header + 4 + length]).unwrap();
        }
    }
}
fn emit(stream: &mut TcpStream, provider: ProviderId, tool: Option<(&str, Value)>, index: usize) {
    let call_id = format!("fixture_{}_{}", Uuid::new_v4(), index);
    let text = "总体判断：合成链可继续。作品检查：以实际读取为准。行文与校对：本轮无额外改稿。";
    let body = match provider {
        ProviderId::Deepseek => {
            let delta = match tool {
                Some((name, args)) => {
                    json!({"tool_calls":[{"index":0,"id":call_id,"type":"function","function":{"name":name,"arguments":args.to_string()}}]})
                }
                None => json!({"content":text}),
            };
            let reason = if delta.get("tool_calls").is_some() {
                "tool_calls"
            } else {
                "stop"
            };
            format!(
                "data: {}\n\ndata: [DONE]\n\n",
                json!({"choices":[{"delta":delta,"finish_reason":reason}]})
            )
        }
        ProviderId::Openai => {
            if let Some((name, args)) = tool {
                format!(
                    "data: {}\n\n",
                    json!({"type":"response.completed","response":{"model":"gpt-5.6-terra","status":"completed","output":[{
                        "type":"function_call","id":format!("fc_{}", Uuid::new_v4()),"call_id":call_id,"name":name,"arguments":args.to_string(),"status":"completed"
                    }]}})
                )
            } else {
                format!(
                    "data: {}\n\ndata: {}\n\n",
                    json!({"type":"response.output_text.delta","delta":text,"sequence_number":1}),
                    json!({"type":"response.completed","response":{"model":"gpt-5.6-terra","status":"completed"}})
                )
            }
        }
    };
    write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
}
pub fn serve(listener: TcpListener, provider: ProviderId, plan: Plan) -> usize {
    let count = match &plan {
        Plan::Chat | Plan::Failure { .. } => 1,
        Plan::Read { .. } | Plan::InvalidTool => 2,
        _ => 3,
    };
    let mut source = Value::Null;
    for index in 0..count {
        let mut stream = accept(&listener);
        let body = request(&mut stream);
        if index == 0 {
            assert_eq!(body["tools"].as_array().unwrap().len(), 3);
            let instruction = match provider {
                ProviderId::Deepseek => &body["messages"][0]["content"],
                ProviderId::Openai => &body["instructions"],
            }
            .as_str()
            .unwrap();
            let sources: Value = serde_json::from_str(
                instruction
                    .lines()
                    .find(|line| line.starts_with("[{") && line.contains("\"currentTurn\""))
                    .unwrap(),
            )
            .unwrap();
            let current = sources
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["currentTurn"] == true && s["role"] == "user")
                .unwrap();
            source = json!({"messageId":current["messageId"],"contentHash":current["contentHash"]});
            let (capability, paths) = match &plan {
                Plan::Chat => {
                    emit(&mut stream, provider, None, index);
                    continue;
                }
                Plan::Failure { partial } => {
                    if *partial {
                        let delta = match provider {
                            ProviderId::Deepseek => {
                                json!({"choices":[{"delta":{"content":"保留这段已生成内容"},"finish_reason":null}]})
                            }
                            ProviderId::Openai => {
                                json!({"type":"response.output_text.delta","delta":"保留这段已生成内容","sequence_number":1})
                            }
                        };
                        let data = format!("data: {delta}\n\n");
                        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{data}", data.len() + 100).unwrap();
                    } else {
                        write!(stream, "HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                    }
                    continue;
                }
                Plan::InvalidTool => {
                    emit(
                        &mut stream,
                        provider,
                        Some(("novelist_read_context", json!({"unexpected":true}))),
                        index,
                    );
                    continue;
                }
                Plan::Create {
                    kind,
                    chapter_source,
                    ..
                } => (
                    match *kind {
                        "character" => "create-character",
                        "world" => "create-setting",
                        "material" => "create-material",
                        "chapter" => "novel-story-telling",
                        _ => unreachable!(),
                    },
                    chapter_source.iter().cloned().collect::<Vec<_>>(),
                ),
                Plan::Read { capability, paths } => (*capability, paths.clone()),
                Plan::Publish { .. } => ("publish-novel", vec![]),
            };
            let operations = paths.iter().map(|path| json!({"kind":"read","relativePath":path,"startLine":1,"maxBytes":32768})).collect::<Vec<_>>();
            let args = app_core::canon_context::normalize_deepseek_novelist_read_context_arguments(
                json!({"capabilityId":capability,"canonOperations":operations}),
            )
            .unwrap();
            emit(
                &mut stream,
                provider,
                Some(("novelist_read_context", args)),
                index,
            );
            continue;
        }
        let output = match provider {
            ProviderId::Deepseek => {
                &body["messages"].as_array().unwrap().last().unwrap()["content"]
            }
            ProviderId::Openai => &body["input"].as_array().unwrap().last().unwrap()["output"],
        };
        let result: Value = serde_json::from_str(output.as_str().unwrap()).unwrap();
        if index + 1 == count {
            assert_eq!(
                result["status"],
                if matches!(
                    plan,
                    Plan::Publish { failure: true, .. } | Plan::InvalidTool
                ) {
                    "failed"
                } else {
                    "succeeded"
                },
                "{result}"
            );
            if matches!(plan, Plan::InvalidTool) {
                assert_eq!(result["errorCode"], "agent.tool_arguments_invalid");
            }
            if let Plan::Create {
                risk, final_error, ..
            } = &plan
            {
                assert_eq!(
                    result["output"]["status"],
                    if *risk != "low" {
                        "pending_confirmation"
                    } else {
                        "committed"
                    },
                    "{result}"
                );
                if *final_error {
                    write!(stream, "HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                    continue;
                }
            }
            if let Plan::Publish {
                failure,
                final_error,
            } = plan
            {
                if !failure {
                    assert_eq!(result["output"]["status"], "generated", "{result}");
                }
                if final_error {
                    write!(stream, "HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                    continue;
                }
            }
            emit(&mut stream, provider, None, index);
            continue;
        }
        assert_eq!(result["status"], "succeeded", "{result}");
        let method = &result["output"]["changeEvidence"]["methodReceiptId"];
        match &plan {
            Plan::Create {
                kind,
                title,
                text,
                chapter,
                chapter_source,
                risk,
                ..
            } => {
                if chapter_source.is_some() {
                    assert_eq!(result["output"]["chapterSources"][0]["status"], "current");
                }
                let capability = match *kind {
                    "character" => "create-character",
                    "world" => "create-setting",
                    "material" => "create-material",
                    "chapter" => "novel-story-telling",
                    _ => unreachable!(),
                };
                let metadata_kind = if *kind == "world" { "setting" } else { kind };
                let version = if *kind == "character" { 2 } else { 1 };
                let chapter_fields = chapter
                    .map(|number| format!("chapterNumber: {number}\nstatus: draft\n"))
                    .unwrap_or_default();
                let markdown = format!("---\nid: new\ntitle: {title}\nkind: {metadata_kind}\nschemaVersion: {version}\nwritingLanguage: zh-CN\n{chapter_fields}---\n\n# {title}\n\n## {}\n\n{text}\n", if chapter.is_some() {"Draft"} else {"摘要"});
                let args = json!({"schemaVersion":2,"sourceKind":if chapter_source.is_some() {"chapter_commit"} else {"user_decision"},
                    "risk":risk,"chapterIntent":chapter.map(|_| "write_and_save"),"finalize":chapter.map(|_| true),"candidateId":null,
                    "sources":[source],"operations":[{"kind":"create","entityKind":kind,"capabilityId":capability,"methodReceiptId":method,
                        "targetId":null,"readReceiptId":null,"expectedSourceHash":null,"proposedMarkdown":markdown}]});
                emit(
                    &mut stream,
                    provider,
                    Some(("novelist_propose_change", args)),
                    index,
                );
            }
            Plan::Publish { .. } => emit(
                &mut stream,
                provider,
                Some((
                    "novelist_publish",
                    json!({"schemaVersion":2,"action":"build","methodReceiptId":method,"source":source}),
                )),
                index,
            ),
            _ => unreachable!(),
        }
    }
    count
}
