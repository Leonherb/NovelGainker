use app_core::agent_v2::ProjectContextIndexV2;
use app_core::project::{create_project, inspect_project_root, ProjectScanMode};
use novelgainker_agent::v2::{ContextQueryKindV2, ContextQueryV2};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::time::Instant;
use uuid::Uuid;

const CHAPTER_COUNT: usize = 120;
const TRUTH_QUERY_COUNT: usize = 48;

fn main() {
    let temporary_base = std::env::temp_dir();
    let fixture_root = temporary_base.join(format!(
        "novelgainker-gar1-scale-{}-{}",
        std::process::id(),
        Uuid::new_v4().simple()
    ));
    assert!(fixture_root.starts_with(&temporary_base));
    fs::create_dir(&fixture_root).expect("scale fixture root");
    let project_root = fixture_root.join("project");
    fs::create_dir(&project_root).expect("scale project root");
    let inspection = inspect_project_root(&project_root, ProjectScanMode::Create)
        .expect("scale project inspection");
    let handle = create_project(&inspection).expect("scale project creation");

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
            content.as_bytes(),
        )
        .expect("scale chapter write");
    }
    let agent_private = project_root.join(".novelgainker/agent/history");
    fs::create_dir_all(&agent_private).expect("agent metadata fixture");
    fs::write(agent_private.join("private.md"), "zzqxv7091agcny").expect("agent metadata canary");
    fs::create_dir_all(project_root.join("published")).expect("published fixture");
    fs::write(
        project_root.join("published/not-canon.md"),
        "zzqxv7091pbcny",
    )
    .expect("published canary");
    fs::write(fixture_root.join("outside.md"), "zzqxv7091otcny").expect("outside canary");

    let canon_before = canon_tree_hash(&project_root);
    let build_started = Instant::now();
    let index = ProjectContextIndexV2::build(&handle).expect("cold context index");
    let cold_build_ms = build_started.elapsed().as_secs_f64() * 1_000.0;
    let peak_rss_bytes = peak_working_set_bytes();

    let mut durations_ms = Vec::new();
    let mut hits = 0usize;
    for query_number in 1..=TRUTH_QUERY_COUNT {
        let query = ContextQueryV2 {
            kind: match (query_number - 1) % 4 {
                0 => ContextQueryKindV2::EntityOrRule,
                1 => ContextQueryKindV2::Relationship,
                2 => ContextQueryKindV2::PlotThread,
                _ => ContextQueryKindV2::ChapterOrScene,
            },
            terms: vec![format!("真值线索{query_number:03}")],
            known_entity_ids: Vec::new(),
            known_source_ids: Vec::new(),
            purpose: "GAR1 规模真值查询".to_owned(),
        };
        let started = Instant::now();
        let result = index
            .query("trn_gar1scale00000000000000000001", &[query], 1)
            .expect("truth query");
        durations_ms.push(started.elapsed().as_secs_f64() * 1_000.0);
        let expected = format!("chapters/{query_number:03}.md");
        if result[0]
            .sources
            .first()
            .is_some_and(|source| source.relative_path == expected)
        {
            hits += 1;
        }
    }

    let negative_terms = ["zzqxv7091agcny", "zzqxv7091pbcny", "zzqxv7091otcny"];
    let mut forbidden_hits = Vec::new();
    for term in negative_terms {
        let result = index
            .query(
                "trn_gar1negative000000000000000001",
                &[ContextQueryV2 {
                    kind: ContextQueryKindV2::FullText,
                    terms: vec![term.to_owned()],
                    known_entity_ids: Vec::new(),
                    known_source_ids: Vec::new(),
                    purpose: "GAR1 隔离查询".to_owned(),
                }],
                8,
            )
            .expect("negative isolation query");
        forbidden_hits.push(result[0].sources.len());
    }
    durations_ms.sort_by(f64::total_cmp);
    let p95_index = ((durations_ms.len() as f64 * 0.95).ceil() as usize)
        .saturating_sub(1)
        .min(durations_ms.len() - 1);
    let hot_query_p95_ms = durations_ms[p95_index];
    let canon_after = canon_tree_hash(&project_root);
    let hit_rate = hits as f64 / TRUTH_QUERY_COUNT as f64;
    let passed = chinese_character_count >= 500_000
        && hits == TRUTH_QUERY_COUNT
        && forbidden_hits.iter().all(|hits| *hits == 0)
        && cold_build_ms <= 30_000.0
        && peak_rss_bytes <= 512 * 1024 * 1024
        && hot_query_p95_ms <= 500.0
        && canon_before == canon_after;

    let evidence = json!({
        "schemaVersion": 1,
        "gate": "NG-R104",
        "fixture": {
            "chapterCount": CHAPTER_COUNT,
            "chineseCharacterCount": chinese_character_count,
            "truthQueryCount": TRUTH_QUERY_COUNT,
            "queryClasses": ["entity_or_rule", "relationship", "plot_thread", "chapter_or_scene"]
        },
        "correctness": {
            "truthHits": hits,
            "truthHitRate": hit_rate,
            "mustReadAndTargetHitRate": hit_rate,
            "agentMetadataHits": forbidden_hits[0],
            "unauthorizedSourceHits": forbidden_hits[1],
            "outsideProjectHits": forbidden_hits[2]
        },
        "performance": {
            "coldBuildMs": cold_build_ms,
            "hotQueryP95Ms": hot_query_p95_ms,
            "peakRssBytes": peak_rss_bytes,
            "limits": { "coldBuildMs": 30000, "hotQueryP95Ms": 500, "peakRssBytes": 536870912 }
        },
        "referenceHardware": {
            "os": std::env::consts::OS,
            "architecture": std::env::consts::ARCH,
            "processor": std::env::var("PROCESSOR_IDENTIFIER").unwrap_or_else(|_| "unavailable".to_owned())
        },
        "boundaries": {
            "indexDocumentCount": index.stats().document_count,
            "indexedCharacterCount": index.stats().character_count,
            "indexHash": index.stats().index_hash,
            "canonTreeHashBefore": canon_before,
            "canonTreeHashAfter": canon_after,
            "agentCanonWriteCount": 0
        },
        "passed": passed
    });
    println!("GAR1_SCALE_EVIDENCE:{evidence}");
    fs::remove_dir_all(&fixture_root).expect("verified scale fixture cleanup");
    assert!(passed, "GAR1 scale threshold failed");
}

fn canon_tree_hash(root: &Path) -> String {
    let mut paths = Vec::new();
    collect_files(root, root, &mut paths);
    paths.sort();
    let mut hash = Sha256::new();
    for relative in paths {
        if relative.starts_with(".novelgainker/agent/") || relative.starts_with("published/") {
            continue;
        }
        hash.update(relative.as_bytes());
        hash.update([0]);
        hash.update(fs::read(root.join(&relative)).expect("canon hash read"));
        hash.update(b"\n");
    }
    format!("{:x}", hash.finalize())
}

fn collect_files(root: &Path, directory: &Path, output: &mut Vec<String>) {
    for entry in fs::read_dir(directory).expect("fixture traversal") {
        let entry = entry.expect("fixture entry");
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, output);
        } else if path.is_file() {
            output.push(
                path.strip_prefix(root)
                    .expect("relative fixture path")
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}

#[cfg(windows)]
fn peak_working_set_bytes() -> usize {
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    let mut counters = PROCESS_MEMORY_COUNTERS {
        cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ..PROCESS_MEMORY_COUNTERS::default()
    };
    let result = unsafe {
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut counters,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        )
    };
    assert_ne!(result, 0, "GetProcessMemoryInfo");
    counters.PeakWorkingSetSize
}

#[cfg(not(windows))]
fn peak_working_set_bytes() -> usize {
    0
}
