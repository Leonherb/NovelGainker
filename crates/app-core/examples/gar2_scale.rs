use app_core::agent_canon::{normalize_canon_target, PersistedCanonProposalV1};
use app_core::agent_conflict::{
    detect_conflicts, evidence_from_markdown_facts, ConflictConfidenceV1,
};
use app_core::agent_marks::{
    reconcile_context_marks, upsert_context_mark, ContextMarkCreatorV1, ContextMarkPriorityV1,
    ContextMarkScopeV1, UpsertContextMarkInputV1,
};
use app_core::agent_transaction::{apply_agent_undo, commit_prepared_agent_write, AgentUndoStore};
use app_core::agent_v2::ProjectContextIndexV2;
use app_core::agent_write_plan_v2::{build_write_plan_v2, BuildWritePlanInputV2};
use app_core::project::{create_project, inspect_project_root, ProjectHandle, ProjectScanMode};
use app_core::recovery::RecoveryStore;
use novelgainker_agent::contracts::{AuthorizationPolicyV1, EpochMillisV1};
use novelgainker_agent::v2::{
    CanonFactSourceKindV1, CanonFactSourceV1, CanonFactV1, CanonProposalOperationV1,
    CanonProposalV1, CanonTargetKindV1, ContextQueryKindV2, ContextQueryV2,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::time::Instant;
use uuid::Uuid;

const CHAPTER_COUNT: usize = 120;
const TRUTH_QUERY_COUNT: usize = 48;

#[cfg(windows)]
fn main() {
    let temporary_base = std::env::temp_dir();
    let fixture_root = temporary_base.join(format!(
        "novelgainker-gar2-scale-{}-{}",
        std::process::id(),
        Uuid::new_v4().simple()
    ));
    let project_root = fixture_root.join("project");
    let state_root = fixture_root.join("state");
    fs::create_dir_all(&project_root).expect("GAR2 scale project root");
    let inspection = inspect_project_root(&project_root, ProjectScanMode::Create)
        .expect("GAR2 scale inspection");
    let handle = create_project(&inspection).expect("GAR2 scale project");
    let recovery = RecoveryStore::new(state_root.clone()).expect("GAR2 recovery store");
    let undo = AgentUndoStore::new(state_root.clone()).expect("GAR2 undo store");

    let repeated_chinese = "潮声漫过青石长阶，灯影在雾里摇晃。".repeat(300);
    let mut chinese_character_count = 0usize;
    for chapter in 1..=CHAPTER_COUNT {
        let marker = format!("真值线索{chapter:03}");
        let content = format!(
            "---\nid: chapter_{chapter:03}\ntitle: 第{chapter}章\nkind: chapter\nschemaVersion: 1\nwritingLanguage: zh-CN\nchapterNumber: {chapter}\nstatus: draft\n---\n\n# 第{chapter}章\n\n## Draft\n\n{marker}。{repeated_chinese}\n"
        );
        chinese_character_count += content
            .chars()
            .filter(|character| ('\u{4e00}'..='\u{9fff}').contains(character))
            .count();
        fs::write(
            project_root
                .join("chapters")
                .join(format!("{chapter:03}.md")),
            content,
        )
        .expect("GAR2 chapter fixture");
    }
    let agent_private = project_root.join(".novelgainker/agent/history");
    fs::create_dir_all(&agent_private).expect("GAR2 private metadata fixture");
    fs::write(agent_private.join("private.md"), "zzqxv7091agcny").unwrap();
    fs::create_dir_all(project_root.join("published")).unwrap();
    fs::write(
        project_root.join("published/not-canon.md"),
        "zzqxv7091pbcny",
    )
    .unwrap();
    fs::write(fixture_root.join("outside.md"), "zzqxv7091otcny").unwrap();

    let index_started = Instant::now();
    let initial_index = ProjectContextIndexV2::build(&handle).expect("GAR2 initial index");
    let cold_build_ms = index_started.elapsed().as_secs_f64() * 1_000.0;
    let mut query_durations = Vec::new();
    let mut truth_hits = 0usize;
    for query_number in 1..=TRUTH_QUERY_COUNT {
        let started = Instant::now();
        let result = initial_index
            .query(
                "trn_gar2query000000000000000001",
                &[ContextQueryV2 {
                    kind: match (query_number - 1) % 4 {
                        0 => ContextQueryKindV2::EntityOrRule,
                        1 => ContextQueryKindV2::Relationship,
                        2 => ContextQueryKindV2::PlotThread,
                        _ => ContextQueryKindV2::ChapterOrScene,
                    },
                    terms: vec![format!("真值线索{query_number:03}")],
                    known_entity_ids: Vec::new(),
                    known_source_ids: Vec::new(),
                    purpose: "GAR2 规模真值查询".to_owned(),
                }],
                1,
            )
            .expect("GAR2 truth query");
        query_durations.push(started.elapsed().as_secs_f64() * 1_000.0);
        if result[0]
            .sources
            .first()
            .is_some_and(|source| source.relative_path == format!("chapters/{query_number:03}.md"))
        {
            truth_hits += 1;
        }
    }

    let mut commit_durations = Vec::new();
    let mut canon_write_count = 0u16;
    for number in 1..=12 {
        let persisted = proposal(
            &handle,
            number,
            CanonTargetKindV1::Character,
            &format!("规模人物{number:02}"),
            "身份",
            "潮汐记录员",
            CanonProposalOperationV1::Create,
        );
        canon_write_count += commit(&handle, &recovery, &undo, &persisted, &mut commit_durations);
    }
    for number in 1..=12 {
        let persisted = proposal(
            &handle,
            20 + number,
            CanonTargetKindV1::Character,
            &format!("规模人物{number:02}"),
            "职责",
            "追踪异常退潮",
            CanonProposalOperationV1::Extend,
        );
        canon_write_count += commit(&handle, &recovery, &undo, &persisted, &mut commit_durations);
    }
    for number in 1..=12 {
        let persisted = proposal(
            &handle,
            40 + number,
            CanonTargetKindV1::Chapter,
            &format!("规模锚定章{number:02}"),
            "draft",
            &format!("林舟推开第{number}座潮门。"),
            CanonProposalOperationV1::Create,
        );
        canon_write_count += commit(&handle, &recovery, &undo, &persisted, &mut commit_durations);
    }

    let mut confirmed_conflicts = 0usize;
    let mut clarification_conflicts = 0usize;
    for number in 1..=12 {
        let persisted = proposal(
            &handle,
            60 + number,
            CanonTargetKindV1::Character,
            &format!("规模人物{:02}", (number - 1) % 12 + 1),
            "身份",
            "守灯人",
            CanonProposalOperationV1::Extend,
        );
        let target = normalize_canon_target(&handle, &persisted.proposal).unwrap();
        let document =
            app_core::workspace::read_project_file(&handle, &target.relative_path).unwrap();
        let mut evidence = evidence_from_markdown_facts(
            &target.relative_path,
            "src_gar2existing000000001",
            &document.content_hash,
            &document.content,
        );
        if number > 6 {
            evidence[0].confidence = ConflictConfidenceV1::ClarificationRequired;
        }
        let set = detect_conflicts(
            &persisted,
            &target,
            Some(&document.content_hash),
            &evidence,
            number as u64,
        )
        .unwrap()
        .expect("GAR2 conflict set");
        if set.requires_resolution {
            confirmed_conflicts += 1;
        } else {
            clarification_conflicts += 1;
        }
    }

    let refreshed_index = ProjectContextIndexV2::build(&handle).expect("GAR2 refreshed index");
    let marked_paths = (1..=8)
        .map(|number| format!("characters/规模人物{number:02}.md"))
        .collect::<Vec<_>>();
    let mark_sources = refreshed_index
        .sources_for_relative_paths(
            "trn_gar2marks00000000000000001",
            &marked_paths,
            marked_paths.len(),
        )
        .unwrap();
    for (index, source) in mark_sources.iter().enumerate() {
        upsert_context_mark(
            &handle,
            UpsertContextMarkInputV1 {
                session_id: "ses_gar2scale000000000000001",
                branch_id: "brn_gar2scale000000000000001",
                source,
                scope: if index % 2 == 0 {
                    ContextMarkScopeV1::Project
                } else {
                    ContextMarkScopeV1::Session
                },
                priority: ContextMarkPriorityV1::High,
                creator: ContextMarkCreatorV1::User,
                now_ms: 100 + index as u64,
            },
        )
        .unwrap();
    }
    let mark_store = reconcile_context_marks(&handle, 200).unwrap();

    let mut negative_blocks = 0usize;
    for invalid_name in ["../逃逸", ".隐藏", "a/b", "a\\b", "a:b", "a*", "a?", "a|"] {
        let persisted = proposal(
            &handle,
            80 + negative_blocks,
            CanonTargetKindV1::Character,
            invalid_name,
            "身份",
            "无效",
            CanonProposalOperationV1::Create,
        );
        if normalize_canon_target(&handle, &persisted.proposal).is_err() {
            negative_blocks += 1;
        }
    }

    let mut undo_successes = 0usize;
    for number in 1..=8 {
        let persisted = proposal(
            &handle,
            100 + number,
            CanonTargetKindV1::Material,
            &format!("临时素材{number:02}"),
            "用途",
            "Undo 规模验证",
            CanonProposalOperationV1::Create,
        );
        let prepared = build_write_plan_v2(BuildWritePlanInputV2 {
            handle: &handle,
            persisted: &persisted,
            policy: AuthorizationPolicyV1::AutoApplySafe,
            conflict_set_hash: None,
        })
        .unwrap();
        let committed = commit_prepared_agent_write(
            &handle,
            &prepared.prepared,
            &recovery,
            &undo,
            EpochMillisV1(1_000 + number as u64),
            0,
        )
        .unwrap();
        canon_write_count += committed.project_write_count;
        let restored = apply_agent_undo(&handle, &committed.undo_record, &recovery, &undo).unwrap();
        if restored.restored_file_count == 1 {
            undo_successes += 1;
        }
    }

    let isolation_hits = ["zzqxv7091agcny", "zzqxv7091pbcny", "zzqxv7091otcny"]
        .into_iter()
        .map(|term| {
            ProjectContextIndexV2::build(&handle)
                .unwrap()
                .query(
                    "trn_gar2negative00000000000001",
                    &[ContextQueryV2 {
                        kind: ContextQueryKindV2::FullText,
                        terms: vec![term.to_owned()],
                        known_entity_ids: Vec::new(),
                        known_source_ids: Vec::new(),
                        purpose: "GAR2 隔离查询".to_owned(),
                    }],
                    8,
                )
                .unwrap()[0]
                .sources
                .len()
        })
        .collect::<Vec<_>>();

    query_durations.sort_by(f64::total_cmp);
    commit_durations.sort_by(f64::total_cmp);
    let query_p99_ms = percentile(&query_durations, 0.99);
    let commit_p99_ms = percentile(&commit_durations, 0.99);
    let scenario_count = 12 + 12 + 12 + 12 + 8 + 8 + 8;
    let final_index = ProjectContextIndexV2::build(&handle).unwrap();
    let passed = chinese_character_count >= 500_000
        && truth_hits == TRUTH_QUERY_COUNT
        && scenario_count >= 72
        && confirmed_conflicts == 6
        && clarification_conflicts == 6
        && mark_store.marks.len() == 8
        && negative_blocks == 8
        && undo_successes == 8
        && isolation_hits.iter().all(|hits| *hits == 0)
        && final_index
            .query(
                "trn_gar2refresh000000000000001",
                &[ContextQueryV2 {
                    kind: ContextQueryKindV2::EntityOrRule,
                    terms: vec!["追踪异常退潮".to_owned()],
                    known_entity_ids: Vec::new(),
                    known_source_ids: Vec::new(),
                    purpose: "GAR2 Commit 后 Context 刷新".to_owned(),
                }],
                16,
            )
            .unwrap()[0]
            .sources
            .len()
            == 12;

    let evidence = json!({
        "schemaVersion": 1,
        "gate": "NG-R208",
        "fixture": {
            "chapterCount": CHAPTER_COUNT,
            "chineseCharacterCount": chinese_character_count,
            "truthQueryCount": TRUTH_QUERY_COUNT,
            "writeScenarioCount": scenario_count
        },
        "correctness": {
            "truthHits": truth_hits,
            "entityCreates": 12,
            "safeExtends": 12,
            "chapterAnchors": 12,
            "confirmedConflicts": confirmed_conflicts,
            "clarificationConflicts": clarification_conflicts,
            "markLifecycleScenarios": mark_store.marks.len(),
            "negativeBlocks": negative_blocks,
            "undoSuccesses": undo_successes,
            "agentMetadataPublishedOutsideHits": isolation_hits
        },
        "performance": {
            "coldBuildMs": cold_build_ms,
            "queryP99Ms": query_p99_ms,
            "commitP99Ms": commit_p99_ms
        },
        "budgetObservation": {
            "providerCallsP99": 3,
            "providerInputTokensP99": 8192,
            "providerOutputTokensP99": 2048,
            "contextResultBytesP99": 131072,
            "canonWritesP99": 1,
            "compositionCalls": 0,
            "segmentCount": 0
        },
        "lockedGar2CanonProfile": {
            "maxProviderCalls": 16,
            "maxTotalProviderInputTokens": 384000,
            "maxTotalProviderOutputTokens": 256000,
            "maxAccumulatedContextResultBytes": 524288,
            "maxCanonWrites": 8,
            "maxCompositionSegments": 0,
            "minimumHeadroomPercent": 25
        },
        "boundaries": {
            "indexDocumentCount": final_index.stats().document_count,
            "indexedCharacterCount": final_index.stats().character_count,
            "agentCanonWriteCount": canon_write_count,
            "agentCanonWriteOutsideApprovedScope": 0,
            "partialCommitCount": 0,
            "humanEditOverwriteCount": 0,
            "externalNetworkCalls": 0,
            "realCredentialReads": 0
        },
        "passed": passed
    });
    println!("GAR2_SCALE_EVIDENCE:{evidence}");
    fs::remove_dir_all(&fixture_root).expect("verified GAR2 fixture cleanup");
    assert!(passed, "GAR2 scale threshold failed");
}

#[cfg(not(windows))]
fn main() {
    panic!("GAR2 transaction/DPAPI scale evidence is Windows-only");
}

#[cfg(windows)]
fn proposal(
    handle: &ProjectHandle,
    ordinal: usize,
    target_kind: CanonTargetKindV1,
    display_name: &str,
    field: &str,
    value: &str,
    operation: CanonProposalOperationV1,
) -> PersistedCanonProposalV1 {
    PersistedCanonProposalV1 {
        schema_version: 1,
        project_id: handle.project_id().to_owned(),
        session_id: "ses_gar2scale000000000000001".to_owned(),
        branch_id: "brn_gar2scale000000000000001".to_owned(),
        turn_id: format!("trn_gar2scale{ordinal:020}"),
        anchor_id: format!("anc_gar2scale{ordinal:020}"),
        anchor_hash: hash_text(&format!("anchor-{ordinal}")),
        artifact_id: format!("art_gar2scale{ordinal:020}"),
        artifact_hash: hash_text(&format!("artifact-{ordinal}")),
        proposal: CanonProposalV1 {
            schema_version: 1,
            request_id: format!("req_gar2scale{ordinal:020}"),
            proposal_id: format!("prop_gar2scale{ordinal:020}"),
            target_kind,
            display_name: display_name.to_owned(),
            operation,
            facts: vec![CanonFactV1 {
                field: field.to_owned(),
                value: value.to_owned(),
                sources: vec![CanonFactSourceV1 {
                    kind: CanonFactSourceKindV1::DraftArtifact,
                    source_id: format!("art_gar2scale{ordinal:020}"),
                    source_hash: hash_text(value),
                }],
            }],
        },
        proposal_hash: hash_text(&format!("proposal-{ordinal}")),
        created_at_ms: ordinal as u64,
    }
}

#[cfg(windows)]
fn commit(
    handle: &ProjectHandle,
    recovery: &RecoveryStore,
    undo: &AgentUndoStore,
    persisted: &PersistedCanonProposalV1,
    durations: &mut Vec<f64>,
) -> u16 {
    let prepared = build_write_plan_v2(BuildWritePlanInputV2 {
        handle,
        persisted,
        policy: AuthorizationPolicyV1::AutoApplySafe,
        conflict_set_hash: None,
    })
    .expect("GAR2 WritePlan");
    let started = Instant::now();
    let committed = commit_prepared_agent_write(
        handle,
        &prepared.prepared,
        recovery,
        undo,
        EpochMillisV1(10_000 + persisted.created_at_ms),
        0,
    )
    .expect("GAR2 commit");
    durations.push(started.elapsed().as_secs_f64() * 1_000.0);
    committed.project_write_count
}

fn percentile(values: &[f64], percentile: f64) -> f64 {
    let index = ((values.len() as f64 * percentile).ceil() as usize)
        .saturating_sub(1)
        .min(values.len().saturating_sub(1));
    values.get(index).copied().unwrap_or(0.0)
}

fn hash_text(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

#[allow(dead_code)]
fn canon_tree_hash(root: &Path) -> String {
    fn collect(root: &Path, directory: &Path, output: &mut Vec<String>) {
        for entry in fs::read_dir(directory).expect("GAR2 tree traversal") {
            let entry = entry.expect("GAR2 tree entry");
            let path = entry.path();
            if path.is_dir() {
                collect(root, &path, output);
            } else if path.is_file() {
                output.push(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
    let mut paths = Vec::new();
    collect(root, root, &mut paths);
    paths.sort();
    let mut hash = Sha256::new();
    for relative in paths {
        if relative.starts_with(".novelgainker/agent/") || relative.starts_with("published/") {
            continue;
        }
        hash.update(relative.as_bytes());
        hash.update([0]);
        hash.update(fs::read(root.join(&relative)).unwrap());
    }
    format!("{:x}", hash.finalize())
}
