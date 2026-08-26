use super::*;
use novelgainker_workflows::story_context::{
    build_story_context_manifest, sha256_hex, BuildStoryContextInputV2, StoryActionV2,
    StoryContextBuildError, StoryContextManifestV2, StoryContextSelectionReasonV2,
    StoryContextSourceInputV2, StoryContextSourceKindV2,
};
use novelgainker_workflows::STORYTELLING_WORKFLOW_ID;
use std::collections::{BTreeMap, BTreeSet};

pub const STORY_CONTEXT_CATALOG_SCHEMA_VERSION: u16 = 2;
pub const STORY_CONTEXT_BUILD_SCHEMA_VERSION: u16 = 2;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryContextCatalogEntryV2 {
    source_id: String,
    kind: StoryContextSourceKindV2,
    relative_path: Option<String>,
    builtin_resource_id: Option<String>,
    display_name: String,
    selection_reason: StoryContextSelectionReasonV2,
    required: bool,
    user_deselectable: bool,
    default_selected: bool,
    priority: u16,
    byte_length: u64,
    content_hash: String,
    modified_time_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryContextCatalogSnapshotV2 {
    schema_version: u16,
    workflow_id: String,
    suggested_chapter_number: u32,
    sources: Vec<StoryContextCatalogEntryV2>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoryContextBuildRequestV2 {
    schema_version: u16,
    action: StoryActionV2,
    chapter_number: u32,
    chapter_title: String,
    selected_source_paths: Vec<String>,
}

#[derive(Clone)]
struct StoryCatalogMaterial {
    entry: StoryContextCatalogEntryV2,
    content: String,
    source_byte_count: u64,
}

#[tauri::command]
pub fn story_context_catalog(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
) -> Result<StoryContextCatalogSnapshotV2, AppError> {
    load_story_context_catalog(&state, &session_id)
}

#[tauri::command]
pub fn story_context_build(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    input: StoryContextBuildRequestV2,
) -> Result<StoryContextManifestV2, AppError> {
    build_registered_story_context(&state, &session_id, input)
}

fn load_story_context_catalog(
    state: &ProjectRegistry,
    session_id: &str,
) -> Result<StoryContextCatalogSnapshotV2, AppError> {
    let handle = session_handle(state, session_id)?;
    let materials = load_story_catalog_materials(&handle)?;
    Ok(StoryContextCatalogSnapshotV2 {
        schema_version: STORY_CONTEXT_CATALOG_SCHEMA_VERSION,
        workflow_id: STORYTELLING_WORKFLOW_ID.to_owned(),
        suggested_chapter_number: suggested_chapter_number(&materials),
        sources: materials
            .into_iter()
            .map(|material| material.entry)
            .collect(),
    })
}

fn build_registered_story_context(
    state: &ProjectRegistry,
    session_id: &str,
    input: StoryContextBuildRequestV2,
) -> Result<StoryContextManifestV2, AppError> {
    if input.schema_version != STORY_CONTEXT_BUILD_SCHEMA_VERSION
        || input.selected_source_paths.len() > 256
    {
        return Err(AppError::workflow_input_invalid());
    }
    let selected = input
        .selected_source_paths
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if selected.len() != input.selected_source_paths.len() {
        return Err(AppError::workflow_source_not_allowed());
    }
    let handle = session_handle(state, session_id)?;
    let materials = load_story_catalog_materials(&handle)?;
    let allowed = materials
        .iter()
        .filter_map(|material| material.entry.relative_path.as_deref())
        .collect::<BTreeSet<_>>();
    if selected.iter().any(|path| !allowed.contains(path)) {
        return Err(AppError::workflow_source_not_allowed());
    }
    let sources = materials
        .into_iter()
        .map(|material| {
            let mut reason = material.entry.selection_reason;
            let included = material.entry.required
                || material
                    .entry
                    .relative_path
                    .as_deref()
                    .is_some_and(|path| selected.contains(path));
            if included && !material.entry.required && !material.entry.default_selected {
                reason = StoryContextSelectionReasonV2::Explicit;
            }
            StoryContextSourceInputV2 {
                source_id: material.entry.source_id,
                kind: material.entry.kind,
                relative_path: material.entry.relative_path,
                builtin_resource_id: material.entry.builtin_resource_id,
                display_name: material.entry.display_name,
                selection_reason: reason,
                required: material.entry.required,
                user_deselectable: material.entry.user_deselectable,
                priority: if reason == StoryContextSelectionReasonV2::Explicit {
                    20
                } else {
                    material.entry.priority
                },
                content: material.content,
                content_hash: material.entry.content_hash,
                source_byte_count: material.source_byte_count,
                modified_time_ms: material.entry.modified_time_ms,
                included,
            }
        })
        .collect();
    build_story_context_manifest(BuildStoryContextInputV2 {
        workflow_id: STORYTELLING_WORKFLOW_ID.to_owned(),
        project_root_identity: handle.project_id().to_owned(),
        action: input.action,
        chapter_number: input.chapter_number,
        chapter_title: input.chapter_title,
        sources,
    })
    .map_err(story_context_error)
}

fn story_context_error(error: StoryContextBuildError) -> AppError {
    match error {
        StoryContextBuildError::InvalidInput => AppError::workflow_input_invalid(),
        StoryContextBuildError::DuplicateSource
        | StoryContextBuildError::InvalidSource
        | StoryContextBuildError::RequiredSourceExcluded => AppError::workflow_source_not_allowed(),
        StoryContextBuildError::BudgetExceeded => AppError::workflow_source_limit_exceeded(),
        StoryContextBuildError::Serialization => AppError::workflow_unavailable(),
    }
}

fn load_story_catalog_materials(
    handle: &ProjectHandle,
) -> Result<Vec<StoryCatalogMaterial>, AppError> {
    let resources = novelgainker_workflows::workflow_resources(STORYTELLING_WORKFLOW_ID)
        .ok_or_else(AppError::workflow_unavailable)?;
    let mut materials = vec![
        builtin_material(
            "builtin:storytelling/workflow",
            StoryContextSourceKindV2::BuiltinProtocol,
            "storytelling/workflow.md",
            "工作流协议",
            1,
            resources.workflow_instructions,
        ),
        builtin_material(
            "builtin:storytelling/input-schema",
            StoryContextSourceKindV2::BuiltinSchema,
            "storytelling/input.schema.json",
            "上下文输入 Schema",
            2,
            resources.input_schema,
        ),
        builtin_material(
            "builtin:storytelling/output-schema",
            StoryContextSourceKindV2::BuiltinSchema,
            "storytelling/output.schema.json",
            "上下文输出 Schema",
            3,
            resources.output_schema,
        ),
        builtin_material(
            "builtin:storytelling/chapter-template",
            StoryContextSourceKindV2::BuiltinTemplate,
            "storytelling/chapter.template.md",
            "章节模板",
            4,
            resources.template_resource,
        ),
    ];
    materials.push(required_project_material(
        read_project_file(handle, "project.md")?,
        "项目规则",
        10,
    ));
    materials.push(required_project_material(
        read_project_file(handle, "AGENTS.md")?,
        "项目协作规则",
        11,
    ));

    let index = list_project_files(handle)?;
    let mut candidates = Vec::new();
    collect_story_markdown_paths(&index.nodes, &mut candidates);
    candidates.sort();
    candidates.dedup();
    let mut chapter_materials = BTreeMap::new();
    let mut other_materials = Vec::new();
    for relative_path in candidates {
        let document = match read_project_file(handle, &relative_path) {
            Ok(document) => document,
            Err(_) => continue,
        };
        if let Some(number) = chapter_number_from_path(&relative_path) {
            let Some(draft) = extract_draft(&document.content) else {
                continue;
            };
            chapter_materials.insert(
                number,
                optional_project_material(
                    document,
                    draft,
                    StoryContextSourceKindV2::ChapterDraft,
                    StoryContextSelectionReasonV2::Available,
                    false,
                    100,
                ),
            );
            continue;
        }
        let (kind, reason, default_selected, priority) = classify_story_source(&relative_path);
        other_materials.push(optional_project_material(
            document,
            String::new(),
            kind,
            reason,
            default_selected,
            priority,
        ));
    }
    let recent = chapter_materials
        .keys()
        .rev()
        .take(2)
        .copied()
        .collect::<BTreeSet<_>>();
    for (number, mut material) in chapter_materials {
        if recent.contains(&number) {
            material.entry.default_selected = true;
            material.entry.selection_reason = StoryContextSelectionReasonV2::RecentChapter;
            material.entry.priority = 50 + u16::try_from(number % 2).unwrap_or(0);
        }
        other_materials.push(material);
    }
    other_materials.sort_by(|left, right| {
        left.entry
            .priority
            .cmp(&right.entry.priority)
            .then_with(|| left.entry.source_id.cmp(&right.entry.source_id))
    });
    materials.extend(other_materials);
    Ok(materials)
}

fn builtin_material(
    source_id: &str,
    kind: StoryContextSourceKindV2,
    resource_id: &str,
    display_name: &str,
    priority: u16,
    content: &str,
) -> StoryCatalogMaterial {
    StoryCatalogMaterial {
        entry: StoryContextCatalogEntryV2 {
            source_id: source_id.to_owned(),
            kind,
            relative_path: None,
            builtin_resource_id: Some(resource_id.to_owned()),
            display_name: display_name.to_owned(),
            selection_reason: StoryContextSelectionReasonV2::Required,
            required: true,
            user_deselectable: false,
            default_selected: true,
            priority,
            byte_length: content.len() as u64,
            content_hash: sha256_hex(content.as_bytes()),
            modified_time_ms: 0,
        },
        content: content.to_owned(),
        source_byte_count: content.len() as u64,
    }
}

fn required_project_material(
    document: ProjectFileDocument,
    display_name: &str,
    priority: u16,
) -> StoryCatalogMaterial {
    StoryCatalogMaterial {
        entry: StoryContextCatalogEntryV2 {
            source_id: format!("project:{}", document.relative_path),
            kind: StoryContextSourceKindV2::ProjectRules,
            relative_path: Some(document.relative_path),
            builtin_resource_id: None,
            display_name: display_name.to_owned(),
            selection_reason: StoryContextSelectionReasonV2::Required,
            required: true,
            user_deselectable: false,
            default_selected: true,
            priority,
            byte_length: document.content.len() as u64,
            content_hash: document.content_hash,
            modified_time_ms: document.modified_time_ms,
        },
        source_byte_count: document.size_bytes,
        content: document.content,
    }
}

fn optional_project_material(
    document: ProjectFileDocument,
    extracted_content: String,
    kind: StoryContextSourceKindV2,
    selection_reason: StoryContextSelectionReasonV2,
    default_selected: bool,
    priority: u16,
) -> StoryCatalogMaterial {
    let content = if extracted_content.is_empty() {
        document.content.clone()
    } else {
        extracted_content
    };
    let display_name = source_display_name(&document.relative_path);
    StoryCatalogMaterial {
        entry: StoryContextCatalogEntryV2 {
            source_id: format!("project:{}", document.relative_path),
            kind,
            relative_path: Some(document.relative_path),
            builtin_resource_id: None,
            display_name,
            selection_reason,
            required: false,
            user_deselectable: true,
            default_selected,
            priority,
            byte_length: content.len() as u64,
            content_hash: document.content_hash,
            modified_time_ms: document.modified_time_ms,
        },
        source_byte_count: document.size_bytes,
        content,
    }
}

fn classify_story_source(
    relative_path: &str,
) -> (
    StoryContextSourceKindV2,
    StoryContextSelectionReasonV2,
    bool,
    u16,
) {
    match relative_path {
        "plot/main.md" => (
            StoryContextSourceKindV2::Plot,
            StoryContextSelectionReasonV2::DefaultPlot,
            true,
            30,
        ),
        "outlines/main.md" => (
            StoryContextSourceKindV2::Outline,
            StoryContextSelectionReasonV2::DefaultOutline,
            true,
            31,
        ),
        "style/main.md" => (
            StoryContextSourceKindV2::Style,
            StoryContextSelectionReasonV2::DefaultStyle,
            true,
            32,
        ),
        "materials/000.story-ledger.md" => (
            StoryContextSourceKindV2::Ledger,
            StoryContextSelectionReasonV2::DefaultLedger,
            true,
            40,
        ),
        _ if relative_path.starts_with("characters/") => (
            StoryContextSourceKindV2::Character,
            StoryContextSelectionReasonV2::Available,
            false,
            100,
        ),
        _ if relative_path.starts_with("world/") => (
            StoryContextSourceKindV2::World,
            StoryContextSelectionReasonV2::Available,
            false,
            100,
        ),
        _ if relative_path.starts_with("plot/") => (
            StoryContextSourceKindV2::Plot,
            StoryContextSelectionReasonV2::Available,
            false,
            100,
        ),
        _ if relative_path.starts_with("outlines/") => (
            StoryContextSourceKindV2::Outline,
            StoryContextSelectionReasonV2::Available,
            false,
            100,
        ),
        _ if relative_path.starts_with("style/") => (
            StoryContextSourceKindV2::Style,
            StoryContextSelectionReasonV2::Available,
            false,
            100,
        ),
        _ => (
            StoryContextSourceKindV2::Material,
            StoryContextSelectionReasonV2::Available,
            false,
            100,
        ),
    }
}

fn collect_story_markdown_paths(nodes: &[ProjectFileNode], output: &mut Vec<String>) {
    for node in nodes {
        if node.kind == ProjectFileKind::Markdown
            && node.size_bytes.is_some_and(|size| size > 0)
            && is_allowed_story_source_path(&node.relative_path)
        {
            output.push(node.relative_path.clone());
        }
        collect_story_markdown_paths(&node.children, output);
    }
}

fn is_allowed_story_source_path(relative_path: &str) -> bool {
    if chapter_number_from_path(relative_path).is_some() {
        return true;
    }
    [
        "characters/",
        "materials/",
        "outlines/",
        "plot/",
        "style/",
        "world/",
    ]
    .iter()
    .any(|prefix| {
        relative_path
            .strip_prefix(prefix)
            .is_some_and(safe_markdown_tail)
    })
}

fn safe_markdown_tail(tail: &str) -> bool {
    tail.ends_with(".md")
        && tail.len() > 3
        && tail.split('/').all(|component| {
            !component.is_empty()
                && component != "."
                && component != ".."
                && !component.starts_with('.')
                && !component.contains(['\\', ':', '\0'])
                && !component.chars().any(char::is_control)
        })
}

fn chapter_number_from_path(relative_path: &str) -> Option<u32> {
    let tail = relative_path.strip_prefix("chapters/")?;
    if tail.contains('/') {
        return None;
    }
    let number = tail.strip_suffix(".md")?.parse::<u32>().ok()?;
    (number > 0 && number <= 999_999).then_some(number)
}

fn extract_draft(content: &str) -> Option<String> {
    let normalized = content.replace("\r\n", "\n").replace('\r', "\n");
    let marker = "## Draft";
    let start = normalized
        .match_indices(marker)
        .find(|(index, _)| *index == 0 || normalized.as_bytes()[index - 1] == b'\n')?
        .0;
    let body_start = start + marker.len();
    let tail = &normalized[body_start..];
    let end = tail
        .match_indices("\n## ")
        .next()
        .map(|(index, _)| index)
        .unwrap_or(tail.len());
    let draft = tail[..end].trim();
    (!draft.is_empty()).then(|| draft.to_owned())
}

fn suggested_chapter_number(materials: &[StoryCatalogMaterial]) -> u32 {
    materials
        .iter()
        .filter_map(|material| {
            material
                .entry
                .relative_path
                .as_deref()
                .and_then(chapter_number_from_path)
        })
        .max()
        .unwrap_or(0)
        .saturating_add(1)
        .clamp(1, 999_999)
}

fn source_display_name(relative_path: &str) -> String {
    relative_path
        .rsplit('/')
        .next()
        .and_then(|name| name.strip_suffix(".md"))
        .unwrap_or(relative_path)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use app_core::coordination::try_acquire_project_write_lease;
    use app_core::project::{create_project, inspect_project_root, ProjectScanMode};
    use std::fs;
    use uuid::Uuid;

    #[test]
    fn ng_501_catalog_and_manifest_are_stable_bounded_and_zero_write() {
        let root = std::env::temp_dir().join(format!("novelgainker-ng501-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let inspection = inspect_project_root(&root, ProjectScanMode::Create).unwrap();
        let handle = create_project(&inspection).unwrap();
        fs::write(root.join("plot/main.md"), "# 主线\n\nPRIVATE_PLOT_CANARY\n").unwrap();
        fs::write(root.join("outlines/main.md"), "# 大纲\n\n第二章转折\n").unwrap();
        fs::write(root.join("style/main.md"), "# 风格\n\n克制、冷静\n").unwrap();
        fs::write(
            root.join("characters/林遥.md"),
            "---\ntitle: 林遥\nwritingLanguage: zh-CN\n---\n\nPRIVATE_CHARACTER_CANARY\n",
        )
        .unwrap();
        fs::write(
            root.join("chapters/001.md"),
            "---\ntitle: 第一章\nwritingLanguage: zh-CN\n---\n\n## Draft\n\n潮水退去。\n",
        )
        .unwrap();
        let ledger_path = root.join("materials/000.story-ledger.md");
        let ledger_before = fs::read(&ledger_path).unwrap();
        let chapter_before = fs::read(root.join("chapters/001.md")).unwrap();
        let app_state = root.with_extension("app-state-ng501");
        let registry = ProjectRegistry::new(app_state.clone()).unwrap();
        let lease = try_acquire_project_write_lease(&handle.coordination_key())
            .unwrap()
            .unwrap();
        let session = register_session(&registry, None, handle, Some(lease)).unwrap();

        let catalog = load_story_context_catalog(&registry, &session.session_id).unwrap();
        assert_eq!(catalog.schema_version, 2);
        assert_eq!(catalog.workflow_id, STORYTELLING_WORKFLOW_ID);
        assert_eq!(catalog.suggested_chapter_number, 2);
        assert!(catalog
            .sources
            .iter()
            .filter(|source| source.required)
            .all(|source| !source.user_deselectable && source.default_selected));
        let defaults = catalog
            .sources
            .iter()
            .filter(|source| source.default_selected && !source.required)
            .filter_map(|source| source.relative_path.clone())
            .collect::<Vec<_>>();
        assert!(defaults.contains(&"plot/main.md".to_owned()));
        assert!(defaults.contains(&"outlines/main.md".to_owned()));
        assert!(defaults.contains(&"style/main.md".to_owned()));
        assert!(defaults.contains(&"materials/000.story-ledger.md".to_owned()));
        assert!(defaults.contains(&"chapters/001.md".to_owned()));
        let catalog_json = serde_json::to_string(&catalog).unwrap();
        assert!(!catalog_json.contains("PRIVATE_PLOT_CANARY"));
        assert!(!catalog_json.contains("PRIVATE_CHARACTER_CANARY"));

        let request = StoryContextBuildRequestV2 {
            schema_version: 2,
            action: StoryActionV2::Create,
            chapter_number: 2,
            chapter_title: "潮痕".to_owned(),
            selected_source_paths: defaults,
        };
        let first = build_registered_story_context(&registry, &session.session_id, request.clone())
            .unwrap();
        let second =
            build_registered_story_context(&registry, &session.session_id, request.clone())
                .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.target_chapter.relative_path, "chapters/002.md");
        assert!(first
            .selected_sources
            .iter()
            .any(|source| source.relative_path.as_deref() == Some("chapters/001.md")));
        assert!(first
            .excluded_sources
            .iter()
            .any(|source| source.relative_path.as_deref() == Some("characters/林遥.md")));
        assert_eq!(fs::read(&ledger_path).unwrap(), ledger_before);
        assert_eq!(
            fs::read(root.join("chapters/001.md")).unwrap(),
            chapter_before
        );

        fs::write(root.join("plot/main.md"), "# 主线\n\n变化后的潮线\n").unwrap();
        let changed =
            build_registered_story_context(&registry, &session.session_id, request).unwrap();
        assert_ne!(first.aggregate_hash, changed.aggregate_hash);

        let attack = build_registered_story_context(
            &registry,
            &session.session_id,
            StoryContextBuildRequestV2 {
                schema_version: 2,
                action: StoryActionV2::Create,
                chapter_number: 2,
                chapter_title: "潮痕".to_owned(),
                selected_source_paths: vec!["../secret.md".to_owned()],
            },
        )
        .unwrap_err();
        assert_eq!(attack.code, "workflow.source_not_allowed");

        drop(registry);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
