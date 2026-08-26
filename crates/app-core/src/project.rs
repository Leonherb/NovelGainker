use crate::AppError;
use novelgainker_domain::{
    ProjectMetadata, DEFAULT_PROVIDER_ID, DEFAULT_WRITING_LANGUAGE, PROJECT_SCHEMA_VERSION,
    WORKFLOW_PACK_VERSION,
};
use novelgainker_workflows::{workflow_resources, PROJECT_CREATE_WORKFLOW_ID};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::hash_map::DefaultHasher;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const MAX_METADATA_BYTES: u64 = 32 * 1024;
const PROJECT_CREATE_INPUT_SCHEMA_VERSION: u16 = 1;
const PROJECT_CREATE_PREVIEW_SCHEMA_VERSION: u16 = 1;
const MAX_PROJECT_TITLE_CHARS: usize = 120;
const MAX_PROJECT_TITLE_BYTES: usize = 384;
const MAX_AUTHOR_DISPLAY_NAME_CHARS: usize = 120;
const MAX_AUTHOR_DISPLAY_NAME_BYTES: usize = 384;
const MAX_GENRE_CHARS: usize = 80;
const MAX_GENRE_BYTES: usize = 256;
const MAX_PREMISE_CHARS: usize = 300;
const MAX_PREMISE_BYTES: usize = 1024;
const PROJECT_DIRECTORIES: &[&str] = &[
    "characters",
    "materials",
    "macguffins",
    "plot",
    "outlines",
    "world",
    "style",
    "assets",
    "chapters",
    "published",
    ".novelgainker",
];
const SOURCE_DIRECTORIES: &[&str] = &[
    "characters",
    "materials",
    "macguffins",
    "plot",
    "outlines",
    "world",
    "style",
    "chapters",
];
const PROJECT_FILES: &[&str] = &[
    "project.md",
    "AGENTS.md",
    "materials/000.story-ledger.md",
    ".novelgainker/project.json",
];
const AGENTS_MD: &str = r#"# NovelGainker 项目约定

- 默认写作语言为简体中文（zh-CN）。
- Markdown 文件与项目资源是内容的唯一权威来源。
- 修改已有正文前先确认作者意图，不静默覆盖文件。
- `published/` 用于生成物，不作为可编辑源目录。
"#;
const STORY_LEDGER_MD: &str = r#"---
title: 故事状态账本
writingLanguage: zh-CN
status: active
---

# 故事状态账本

在这里记录时间线、角色状态、线索、承诺与回收等连续性事实。
"#;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectCreateInputV1 {
    pub schema_version: u16,
    pub title: String,
    pub author_display_name: Option<String>,
    pub genre: Option<String>,
    pub premise: Option<String>,
}

impl ProjectCreateInputV1 {
    pub fn default_local() -> Self {
        Self {
            schema_version: PROJECT_CREATE_INPUT_SCHEMA_VERSION,
            title: "NovelGainker 项目".to_owned(),
            author_display_name: None,
            genre: None,
            premise: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCreateFilePreview {
    pub relative_path: String,
    pub byte_length: u64,
    pub content_sha256: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCreatePreview {
    pub schema_version: u16,
    pub preview_hash: String,
    pub project_id: String,
    pub writing_language: String,
    pub workflow_pack_version: String,
    pub default_provider_id: String,
    pub existing_unrelated_entry_count: u32,
    pub directories: Vec<String>,
    pub files: Vec<ProjectCreateFilePreview>,
}

#[derive(Debug, Clone)]
pub struct PreparedProjectCreate {
    inspection: ProjectInspection,
    metadata: ProjectMetadata,
    files: Vec<PreparedProjectFile>,
    preview: ProjectCreatePreview,
}

impl PreparedProjectCreate {
    pub fn preview(&self) -> &ProjectCreatePreview {
        &self.preview
    }

    pub fn coordination_key(&self) -> String {
        self.inspection.coordination_key()
    }
}

#[derive(Debug, Clone)]
struct PreparedProjectFile {
    relative_path: &'static str,
    content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectScanMode {
    Create,
    Open,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PlannedEntryKind {
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedEntry {
    pub relative_path: String,
    pub kind: PlannedEntryKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectConflictKind {
    ExistingTarget,
    WrongType,
    SymbolicLink,
    NotMarkdownProject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectConflict {
    pub relative_path: String,
    pub kind: ProjectConflictKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectScanPreview {
    pub mode: ProjectScanMode,
    pub display_name: String,
    pub existing_entry_count: u32,
    pub markdown_file_count: u32,
    pub has_project_metadata: bool,
    pub requires_metadata_confirmation: bool,
    pub can_confirm: bool,
    pub planned_entries: Vec<PlannedEntry>,
    pub conflicts: Vec<ProjectConflict>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectInspection {
    root: PathBuf,
    preview: ProjectScanPreview,
    snapshot: Vec<SnapshotEntry>,
    metadata: Option<ProjectMetadata>,
    metadata_hash: Option<u64>,
}

impl ProjectInspection {
    pub fn preview(&self) -> &ProjectScanPreview {
        &self.preview
    }

    pub fn coordination_key(&self) -> String {
        project_root_coordination_key(&self.root)
    }
}

#[derive(Debug, Clone)]
pub struct ProjectHandle {
    root: PathBuf,
    metadata: ProjectMetadata,
    display_name: String,
    markdown_file_count: u32,
}

impl ProjectHandle {
    pub fn project_id(&self) -> &str {
        &self.metadata.project_id
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn schema_version(&self) -> u16 {
        self.metadata.schema_version
    }

    pub fn writing_language(&self) -> &str {
        &self.metadata.writing_language
    }

    pub fn markdown_file_count(&self) -> u32 {
        self.markdown_file_count
    }

    pub fn is_bound_to_root(&self) -> bool {
        self.root.is_absolute()
    }

    pub fn coordination_key(&self) -> String {
        project_root_coordination_key(&self.root)
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }
}

fn project_root_coordination_key(root: &Path) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write as _;

    let normalized = root.to_string_lossy().to_lowercase();
    let digest = Sha256::digest(normalized.as_bytes());
    let mut output = String::with_capacity(64);
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct SnapshotEntry {
    name: OsString,
    kind: SnapshotKind,
    length: u64,
    modified: Option<SystemTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SnapshotKind {
    File,
    Directory,
    SymbolicLink,
    Other,
}

struct ProjectTransaction {
    created: Vec<(PathBuf, PlannedEntryKind)>,
}

impl ProjectTransaction {
    fn new() -> Self {
        Self {
            created: Vec::new(),
        }
    }

    fn create_directory(&mut self, path: &Path) -> Result<(), AppError> {
        fs::create_dir(path).map_err(|_| project_io("project-create-directory-failed"))?;
        self.created
            .push((path.to_path_buf(), PlannedEntryKind::Directory));
        Ok(())
    }

    fn write_new_file(&mut self, path: &Path, contents: &[u8]) -> Result<(), AppError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|_| project_io("project-create-file-failed"))?;
        self.created
            .push((path.to_path_buf(), PlannedEntryKind::File));
        file.write_all(contents)
            .map_err(|_| project_io("project-write-file-failed"))?;
        file.sync_all()
            .map_err(|_| project_io("project-sync-file-failed"))?;
        Ok(())
    }

    fn rollback(&mut self) -> Result<(), AppError> {
        let mut rollback_failed = false;
        for (path, kind) in self.created.drain(..).rev() {
            let result = match kind {
                PlannedEntryKind::File => fs::remove_file(path),
                PlannedEntryKind::Directory => fs::remove_dir(path),
            };
            rollback_failed |= result.is_err();
        }

        if rollback_failed {
            Err(project_io("project-rollback-incomplete"))
        } else {
            Ok(())
        }
    }
}

pub fn inspect_project_root(
    selected_root: &Path,
    mode: ProjectScanMode,
) -> Result<ProjectInspection, AppError> {
    let root = selected_root
        .canonicalize()
        .map_err(|_| AppError::project_invalid_root())?;
    let root_metadata = fs::metadata(&root).map_err(|_| AppError::project_invalid_root())?;
    if !root_metadata.is_dir() {
        return Err(AppError::project_invalid_root());
    }

    let snapshot = root_snapshot(&root)?;
    let display_name = safe_display_name(&root);
    let mut conflicts = Vec::new();
    let markdown_file_count = count_markdown_files(&root, &mut conflicts)?;
    let existing_entry_count = u32::try_from(snapshot.len()).unwrap_or(u32::MAX);
    let (metadata, metadata_hash) = inspect_metadata(&root, &mut conflicts)?;

    let (planned_entries, requires_metadata_confirmation) = match mode {
        ProjectScanMode::Create => {
            collect_create_conflicts(&root, &mut conflicts)?;
            (scaffold_plan(), false)
        }
        ProjectScanMode::Open => {
            collect_open_structure_conflicts(&root, &mut conflicts)?;
            if metadata.is_none() {
                if markdown_file_count == 0 {
                    conflicts.push(ProjectConflict {
                        relative_path: "project.md".to_owned(),
                        kind: ProjectConflictKind::NotMarkdownProject,
                    });
                }
                (metadata_plan(&root)?, true)
            } else {
                (Vec::new(), false)
            }
        }
    };

    let mut unique_conflicts = Vec::with_capacity(conflicts.len());
    for conflict in conflicts {
        if !unique_conflicts.contains(&conflict) {
            unique_conflicts.push(conflict);
        }
    }

    let preview = ProjectScanPreview {
        mode,
        display_name,
        existing_entry_count,
        markdown_file_count,
        has_project_metadata: metadata.is_some(),
        requires_metadata_confirmation,
        can_confirm: unique_conflicts.is_empty(),
        planned_entries,
        conflicts: unique_conflicts,
    };

    Ok(ProjectInspection {
        root,
        preview,
        snapshot,
        metadata,
        metadata_hash,
    })
}

pub fn create_project(inspection: &ProjectInspection) -> Result<ProjectHandle, AppError> {
    let prepared = prepare_project_create(inspection, ProjectCreateInputV1::default_local())?;
    let preview_hash = prepared.preview.preview_hash.clone();
    create_prepared_project(&prepared, &preview_hash)
}

pub fn prepare_project_create(
    inspection: &ProjectInspection,
    input: ProjectCreateInputV1,
) -> Result<PreparedProjectCreate, AppError> {
    if inspection.preview.mode != ProjectScanMode::Create {
        return Err(AppError::project_scaffold_changed());
    }
    let current = inspect_project_root(&inspection.root, ProjectScanMode::Create)?;
    ensure_inspection_unchanged(inspection, &current)
        .map_err(|_| AppError::project_scaffold_changed())?;
    if !current.preview.can_confirm {
        return Err(AppError::project_conflict());
    }

    let input = normalize_project_create_input(input)?;
    let metadata = ProjectMetadata::v1(Uuid::new_v4().to_string());
    let files = vec![
        PreparedProjectFile {
            relative_path: "project.md",
            content: render_project_markdown(&input),
        },
        PreparedProjectFile {
            relative_path: "AGENTS.md",
            content: AGENTS_MD.to_owned(),
        },
        PreparedProjectFile {
            relative_path: "materials/000.story-ledger.md",
            content: STORY_LEDGER_MD.to_owned(),
        },
        PreparedProjectFile {
            relative_path: ".novelgainker/project.json",
            content: serialize_metadata(&metadata)?,
        },
    ];
    let preview_hash = project_create_preview_hash(&current, &input, &files)?;
    let preview = ProjectCreatePreview {
        schema_version: PROJECT_CREATE_PREVIEW_SCHEMA_VERSION,
        preview_hash,
        project_id: metadata.project_id.clone(),
        writing_language: DEFAULT_WRITING_LANGUAGE.to_owned(),
        workflow_pack_version: WORKFLOW_PACK_VERSION.to_owned(),
        default_provider_id: DEFAULT_PROVIDER_ID.to_owned(),
        existing_unrelated_entry_count: current.preview.existing_entry_count,
        directories: PROJECT_DIRECTORIES
            .iter()
            .map(|path| (*path).to_owned())
            .collect(),
        files: files
            .iter()
            .map(|file| ProjectCreateFilePreview {
                relative_path: file.relative_path.to_owned(),
                byte_length: u64::try_from(file.content.len()).unwrap_or(u64::MAX),
                content_sha256: sha256_hex(file.content.as_bytes()),
                content: file.content.clone(),
            })
            .collect(),
    };
    Ok(PreparedProjectCreate {
        inspection: current,
        metadata,
        files,
        preview,
    })
}

pub fn create_prepared_project(
    prepared: &PreparedProjectCreate,
    expected_preview_hash: &str,
) -> Result<ProjectHandle, AppError> {
    create_prepared_project_impl(prepared, expected_preview_hash, None)
}

pub fn open_project(
    inspection: &ProjectInspection,
    confirm_metadata: bool,
) -> Result<ProjectHandle, AppError> {
    if inspection.preview.mode != ProjectScanMode::Open {
        return Err(AppError::project_candidate_changed());
    }
    let current = inspect_project_root(&inspection.root, ProjectScanMode::Open)?;
    ensure_inspection_unchanged(inspection, &current)?;
    if !current.preview.can_confirm {
        return Err(AppError::project_conflict());
    }

    if current.metadata.is_none() {
        if !confirm_metadata {
            return Err(AppError::project_confirmation_required());
        }
        create_metadata_only(&current.root)?;
    }

    let opened = inspect_project_root(&current.root, ProjectScanMode::Open)?;
    if !opened.preview.can_confirm {
        return Err(AppError::project_conflict());
    }
    project_handle_from_inspection(opened)
}

fn create_prepared_project_impl(
    prepared: &PreparedProjectCreate,
    expected_preview_hash: &str,
    fail_after_operations: Option<usize>,
) -> Result<ProjectHandle, AppError> {
    if expected_preview_hash != prepared.preview.preview_hash {
        return Err(AppError::project_scaffold_changed());
    }
    let current = inspect_project_root(&prepared.inspection.root, ProjectScanMode::Create)?;
    ensure_inspection_unchanged(&prepared.inspection, &current)
        .map_err(|_| AppError::project_scaffold_changed())?;
    if !current.preview.can_confirm {
        return Err(AppError::project_conflict());
    }

    let mut transaction = ProjectTransaction::new();
    let mut operation_count = 0_usize;

    let result: Result<ProjectHandle, AppError> = (|| {
        for relative_path in PROJECT_DIRECTORIES {
            transaction.create_directory(&prepared.inspection.root.join(relative_path))?;
            operation_count += 1;
            inject_failure(fail_after_operations, operation_count)?;
        }

        for file in &prepared.files {
            transaction.write_new_file(
                &prepared.inspection.root.join(file.relative_path),
                file.content.as_bytes(),
            )?;
            operation_count += 1;
            inject_failure(fail_after_operations, operation_count)?;
        }

        let opened = inspect_project_root(&prepared.inspection.root, ProjectScanMode::Open)?;
        let handle = project_handle_from_inspection(opened)?;
        if handle.project_id() != prepared.metadata.project_id {
            return Err(AppError::project_invalid_metadata());
        }
        Ok(handle)
    })();

    match result {
        Ok(handle) => Ok(handle),
        Err(error) => {
            transaction.rollback()?;
            Err(error)
        }
    }
}

fn normalize_project_create_input(
    input: ProjectCreateInputV1,
) -> Result<ProjectCreateInputV1, AppError> {
    if input.schema_version != PROJECT_CREATE_INPUT_SCHEMA_VERSION {
        return Err(AppError::project_metadata_invalid());
    }
    Ok(ProjectCreateInputV1 {
        schema_version: PROJECT_CREATE_INPUT_SCHEMA_VERSION,
        title: normalize_required_field(
            &input.title,
            MAX_PROJECT_TITLE_CHARS,
            MAX_PROJECT_TITLE_BYTES,
        )?,
        author_display_name: normalize_optional_field(
            input.author_display_name.as_deref(),
            MAX_AUTHOR_DISPLAY_NAME_CHARS,
            MAX_AUTHOR_DISPLAY_NAME_BYTES,
        )?,
        genre: normalize_optional_field(input.genre.as_deref(), MAX_GENRE_CHARS, MAX_GENRE_BYTES)?,
        premise: normalize_optional_field(
            input.premise.as_deref(),
            MAX_PREMISE_CHARS,
            MAX_PREMISE_BYTES,
        )?,
    })
}

fn normalize_required_field(
    value: &str,
    maximum_characters: usize,
    maximum_bytes: usize,
) -> Result<String, AppError> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(AppError::project_metadata_invalid());
    }
    validate_project_create_field(normalized, maximum_characters, maximum_bytes)?;
    Ok(normalized.to_owned())
}

fn normalize_optional_field(
    value: Option<&str>,
    maximum_characters: usize,
    maximum_bytes: usize,
) -> Result<Option<String>, AppError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let normalized = value.trim();
    if normalized.is_empty() {
        return Ok(None);
    }
    validate_project_create_field(normalized, maximum_characters, maximum_bytes)?;
    Ok(Some(normalized.to_owned()))
}

fn validate_project_create_field(
    value: &str,
    maximum_characters: usize,
    maximum_bytes: usize,
) -> Result<(), AppError> {
    if value.len() > maximum_bytes {
        return Err(AppError::project_metadata_invalid());
    }
    let mut length = 0_usize;
    for character in value.chars() {
        length += 1;
        let code_point = character as u32;
        if character.is_control()
            || matches!(code_point, 0x202a..=0x202e | 0x2066..=0x2069)
            || length > maximum_characters
        {
            return Err(AppError::project_metadata_invalid());
        }
    }
    Ok(())
}

fn render_project_markdown(input: &ProjectCreateInputV1) -> String {
    let template = workflow_resources(PROJECT_CREATE_WORKFLOW_ID)
        .expect("validated project-create resources must be embedded")
        .template_resource;
    let author_frontmatter = input
        .author_display_name
        .as_ref()
        .map(|author| format!("authorDisplayName: {}\n", yaml_quoted(author)))
        .unwrap_or_default();
    let genre_frontmatter = input
        .genre
        .as_ref()
        .map(|genre| format!("genre: {}\n", yaml_quoted(genre)))
        .unwrap_or_default();
    let premise_section = input
        .premise
        .as_ref()
        .map(|premise| format!("## 一句话前提\n\n{premise}\n\n"))
        .unwrap_or_default();
    template
        .replace("{{titleYaml}}", &yaml_quoted(&input.title))
        .replace("{{authorFrontmatter}}", &author_frontmatter)
        .replace("{{genreFrontmatter}}", &genre_frontmatter)
        .replace("{{title}}", &input.title)
        .replace("{{premiseSection}}", &premise_section)
}

fn yaml_quoted(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 2);
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            _ => output.push(character),
        }
    }
    output.push('"');
    output
}

fn project_create_preview_hash(
    inspection: &ProjectInspection,
    input: &ProjectCreateInputV1,
    files: &[PreparedProjectFile],
) -> Result<String, AppError> {
    let mut hasher = Sha256::new();
    hash_segment(&mut hasher, b"novelgainker.project-create.preview.v1");
    hash_segment(&mut hasher, inspection.coordination_key().as_bytes());
    let input_json = serde_json::to_vec(input).map_err(|_| AppError::project_metadata_invalid())?;
    hash_segment(&mut hasher, &input_json);
    for entry in &inspection.snapshot {
        hash_segment(&mut hasher, entry.name.to_string_lossy().as_bytes());
        hash_segment(
            &mut hasher,
            match entry.kind {
                SnapshotKind::File => b"file",
                SnapshotKind::Directory => b"directory",
                SnapshotKind::SymbolicLink => b"symbolic-link",
                SnapshotKind::Other => b"other",
            },
        );
        hash_segment(&mut hasher, &entry.length.to_be_bytes());
        let modified_nanos = entry
            .modified
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        hash_segment(&mut hasher, &modified_nanos.to_be_bytes());
    }
    for directory in PROJECT_DIRECTORIES {
        hash_segment(&mut hasher, b"directory");
        hash_segment(&mut hasher, directory.as_bytes());
    }
    for file in files {
        hash_segment(&mut hasher, b"file");
        hash_segment(&mut hasher, file.relative_path.as_bytes());
        hash_segment(&mut hasher, file.content.as_bytes());
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn hash_segment(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn create_metadata_only(root: &Path) -> Result<(), AppError> {
    let metadata = ProjectMetadata::v1(Uuid::new_v4().to_string());
    let metadata_json = serialize_metadata(&metadata)?;
    let metadata_directory = root.join(".novelgainker");
    let metadata_path = metadata_directory.join("project.json");
    let mut transaction = ProjectTransaction::new();

    let result = (|| {
        if !metadata_directory.exists() {
            transaction.create_directory(&metadata_directory)?;
        }
        transaction.write_new_file(&metadata_path, metadata_json.as_bytes())
    })();

    if let Err(error) = result {
        transaction.rollback()?;
        return Err(error);
    }
    Ok(())
}

fn serialize_metadata(metadata: &ProjectMetadata) -> Result<String, AppError> {
    let mut json =
        serde_json::to_string_pretty(metadata).map_err(|_| AppError::project_invalid_metadata())?;
    json.push('\n');
    Ok(json)
}

fn ensure_inspection_unchanged(
    expected: &ProjectInspection,
    current: &ProjectInspection,
) -> Result<(), AppError> {
    if expected.root != current.root
        || expected.preview != current.preview
        || expected.snapshot != current.snapshot
        || expected.metadata != current.metadata
        || expected.metadata_hash != current.metadata_hash
    {
        return Err(AppError::project_candidate_changed());
    }
    Ok(())
}

fn project_handle_from_inspection(
    inspection: ProjectInspection,
) -> Result<ProjectHandle, AppError> {
    let metadata = inspection
        .metadata
        .ok_or_else(AppError::project_invalid_metadata)?;
    Ok(ProjectHandle {
        root: inspection.root,
        metadata,
        display_name: inspection.preview.display_name,
        markdown_file_count: inspection.preview.markdown_file_count,
    })
}

fn inspect_metadata(
    root: &Path,
    conflicts: &mut Vec<ProjectConflict>,
) -> Result<(Option<ProjectMetadata>, Option<u64>), AppError> {
    let metadata_directory = root.join(".novelgainker");
    let directory_metadata = match fs::symlink_metadata(&metadata_directory) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok((None, None)),
        Err(_) => return Err(project_io("project-metadata-inspection-failed")),
    };

    if directory_metadata.file_type().is_symlink() {
        conflicts.push(ProjectConflict {
            relative_path: ".novelgainker".to_owned(),
            kind: ProjectConflictKind::SymbolicLink,
        });
        return Ok((None, None));
    }
    if !directory_metadata.is_dir() {
        conflicts.push(ProjectConflict {
            relative_path: ".novelgainker".to_owned(),
            kind: ProjectConflictKind::WrongType,
        });
        return Ok((None, None));
    }

    let metadata_path = metadata_directory.join("project.json");
    let file_metadata = match fs::symlink_metadata(&metadata_path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok((None, None)),
        Err(_) => return Err(project_io("project-metadata-inspection-failed")),
    };
    if file_metadata.file_type().is_symlink() {
        conflicts.push(ProjectConflict {
            relative_path: ".novelgainker/project.json".to_owned(),
            kind: ProjectConflictKind::SymbolicLink,
        });
        return Ok((None, None));
    }
    if !file_metadata.is_file() || file_metadata.len() > MAX_METADATA_BYTES {
        return Err(AppError::project_invalid_metadata());
    }

    let mut bytes = Vec::with_capacity(file_metadata.len() as usize);
    File::open(&metadata_path)
        .and_then(|mut file| file.read_to_end(&mut bytes))
        .map_err(|_| project_io("project-metadata-read-failed"))?;
    let metadata: ProjectMetadata =
        serde_json::from_slice(&bytes).map_err(|_| AppError::project_invalid_metadata())?;
    validate_metadata(&metadata)?;

    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    Ok((Some(metadata), Some(hasher.finish())))
}

fn validate_metadata(metadata: &ProjectMetadata) -> Result<(), AppError> {
    if metadata.schema_version != PROJECT_SCHEMA_VERSION {
        return Err(AppError::project_unsupported_schema());
    }
    if Uuid::parse_str(&metadata.project_id).is_err()
        || metadata.workflow_pack_version != WORKFLOW_PACK_VERSION
        || metadata.default_provider_id != DEFAULT_PROVIDER_ID
        || metadata.writing_language != DEFAULT_WRITING_LANGUAGE
        || !metadata.strict_chapter_markup
    {
        return Err(AppError::project_invalid_metadata());
    }
    Ok(())
}

fn root_snapshot(root: &Path) -> Result<Vec<SnapshotEntry>, AppError> {
    let mut entries = Vec::new();
    let directory = fs::read_dir(root).map_err(|_| project_io("project-root-scan-failed"))?;
    for entry in directory {
        let entry = entry.map_err(|_| project_io("project-root-scan-failed"))?;
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|_| project_io("project-root-scan-failed"))?;
        let file_type = metadata.file_type();
        let kind = if file_type.is_symlink() {
            SnapshotKind::SymbolicLink
        } else if metadata.is_file() {
            SnapshotKind::File
        } else if metadata.is_dir() {
            SnapshotKind::Directory
        } else {
            SnapshotKind::Other
        };
        entries.push(SnapshotEntry {
            name: entry.file_name(),
            kind,
            length: metadata.len(),
            modified: metadata.modified().ok(),
        });
    }
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(entries)
}

fn count_markdown_files(
    root: &Path,
    conflicts: &mut Vec<ProjectConflict>,
) -> Result<u32, AppError> {
    let mut count = 0_u32;
    for entry in fs::read_dir(root).map_err(|_| project_io("project-root-scan-failed"))? {
        let entry = entry.map_err(|_| project_io("project-root-scan-failed"))?;
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|_| project_io("project-root-scan-failed"))?;
        if !metadata.file_type().is_symlink()
            && metadata.is_file()
            && is_markdown_path(&entry.path())
        {
            count = count.saturating_add(1);
        }
    }

    for relative_directory in SOURCE_DIRECTORIES {
        let directory = root.join(relative_directory);
        let metadata = match fs::symlink_metadata(&directory) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(project_io("project-source-scan-failed")),
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            continue;
        }
        for entry in
            fs::read_dir(&directory).map_err(|_| project_io("project-source-scan-failed"))?
        {
            let entry = entry.map_err(|_| project_io("project-source-scan-failed"))?;
            let child_metadata = fs::symlink_metadata(entry.path())
                .map_err(|_| project_io("project-source-scan-failed"))?;
            if child_metadata.file_type().is_symlink() {
                conflicts.push(ProjectConflict {
                    relative_path: format!(
                        "{relative_directory}/{}",
                        safe_component_name(&entry.file_name())
                    ),
                    kind: ProjectConflictKind::SymbolicLink,
                });
            } else if child_metadata.is_file() && is_markdown_path(&entry.path()) {
                count = count.saturating_add(1);
            }
        }
    }
    Ok(count)
}

fn collect_create_conflicts(
    root: &Path,
    conflicts: &mut Vec<ProjectConflict>,
) -> Result<(), AppError> {
    for relative_path in PROJECT_DIRECTORIES.iter().chain(PROJECT_FILES.iter()) {
        match fs::symlink_metadata(root.join(relative_path)) {
            Ok(_) => conflicts.push(ProjectConflict {
                relative_path: (*relative_path).to_owned(),
                kind: ProjectConflictKind::ExistingTarget,
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(project_io("project-preflight-failed")),
        }
    }
    Ok(())
}

fn collect_open_structure_conflicts(
    root: &Path,
    conflicts: &mut Vec<ProjectConflict>,
) -> Result<(), AppError> {
    for relative_directory in PROJECT_DIRECTORIES {
        let path = root.join(relative_directory);
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => conflicts.push(ProjectConflict {
                relative_path: (*relative_directory).to_owned(),
                kind: ProjectConflictKind::SymbolicLink,
            }),
            Ok(metadata) if !metadata.is_dir() => conflicts.push(ProjectConflict {
                relative_path: (*relative_directory).to_owned(),
                kind: ProjectConflictKind::WrongType,
            }),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(project_io("project-structure-scan-failed")),
        }
    }
    Ok(())
}

fn metadata_plan(root: &Path) -> Result<Vec<PlannedEntry>, AppError> {
    let mut entries = Vec::new();
    match fs::symlink_metadata(root.join(".novelgainker")) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => entries.push(PlannedEntry {
            relative_path: ".novelgainker".to_owned(),
            kind: PlannedEntryKind::Directory,
        }),
        Err(_) => return Err(project_io("project-metadata-plan-failed")),
    }
    entries.push(PlannedEntry {
        relative_path: ".novelgainker/project.json".to_owned(),
        kind: PlannedEntryKind::File,
    });
    Ok(entries)
}

fn scaffold_plan() -> Vec<PlannedEntry> {
    let mut entries = Vec::with_capacity(PROJECT_DIRECTORIES.len() + PROJECT_FILES.len());
    entries.extend(PROJECT_FILES[..2].iter().map(|path| PlannedEntry {
        relative_path: (*path).to_owned(),
        kind: PlannedEntryKind::File,
    }));
    entries.extend(PROJECT_DIRECTORIES.iter().map(|path| PlannedEntry {
        relative_path: (*path).to_owned(),
        kind: PlannedEntryKind::Directory,
    }));
    entries.extend(PROJECT_FILES[2..].iter().map(|path| PlannedEntry {
        relative_path: (*path).to_owned(),
        kind: PlannedEntryKind::File,
    }));
    entries
}

fn safe_display_name(root: &Path) -> String {
    root.file_name()
        .map(safe_component_name)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "本地项目".to_owned())
}

fn safe_component_name(name: &std::ffi::OsStr) -> String {
    name.to_string_lossy()
        .chars()
        .filter(|character| !character.is_control())
        .take(80)
        .collect::<String>()
        .trim()
        .to_owned()
}

fn is_markdown_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

fn inject_failure(
    fail_after_operations: Option<usize>,
    operation_count: usize,
) -> Result<(), AppError> {
    if fail_after_operations == Some(operation_count) {
        Err(project_io("project-test-injected-failure"))
    } else {
        Ok(())
    }
}

fn project_io(diagnostic: &'static str) -> AppError {
    AppError::project_io().with_diagnostic(diagnostic)
}

#[cfg(test)]
mod tests {
    use super::{
        create_prepared_project, create_prepared_project_impl, create_project,
        inspect_project_root, open_project, prepare_project_create, ProjectCreateInputV1,
        ProjectScanMode, AGENTS_MD, STORY_LEDGER_MD,
    };
    use novelgainker_domain::ProjectMetadata;
    use std::fs;
    use std::path::{Path, PathBuf};
    use uuid::Uuid;

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new(label: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("novelgainker-{label}-{}", Uuid::new_v4()));
            fs::create_dir(&root).expect("test root must be created");
            Self(root)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let temporary_root = std::env::temp_dir();
            let is_verified_test_root = self.0.parent() == Some(temporary_root.as_path())
                && self
                    .0
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("novelgainker-"));
            if is_verified_test_root {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
    }

    #[test]
    fn creates_the_complete_v1_scaffold_with_lf_and_no_bom() {
        let root = TestRoot::new("scaffold");
        let inspection = inspect_project_root(root.path(), ProjectScanMode::Create)
            .expect("empty root must scan");

        let handle = create_project(&inspection).expect("scaffold must be created");

        for directory in super::PROJECT_DIRECTORIES {
            assert!(root.path().join(directory).is_dir(), "missing {directory}");
        }
        for file in super::PROJECT_FILES {
            assert!(root.path().join(file).is_file(), "missing {file}");
        }
        let project_markdown =
            fs::read_to_string(root.path().join("project.md")).expect("project.md must read");
        assert_eq!(
            project_markdown,
            super::render_project_markdown(&ProjectCreateInputV1::default_local())
        );
        assert!(!project_markdown.contains("## 一句话前提"));
        assert!(!project_markdown.contains("authorDisplayName:"));
        assert!(!project_markdown.contains("genre:"));
        assert_eq!(
            fs::read_to_string(root.path().join("AGENTS.md")).expect("AGENTS.md must read"),
            AGENTS_MD
        );
        assert_eq!(
            fs::read_to_string(root.path().join("materials/000.story-ledger.md"))
                .expect("ledger must read"),
            STORY_LEDGER_MD
        );
        let metadata_bytes =
            fs::read(root.path().join(".novelgainker/project.json")).expect("metadata must read");
        assert!(!metadata_bytes.starts_with(&[0xef, 0xbb, 0xbf]));
        assert!(!metadata_bytes.windows(2).any(|bytes| bytes == b"\r\n"));
        let metadata: ProjectMetadata =
            serde_json::from_slice(&metadata_bytes).expect("metadata must match schema");
        assert_eq!(metadata.project_id, handle.project_id());
        assert_eq!(metadata.schema_version, 1);
        assert_eq!(metadata.default_provider_id, "openai");
        assert_eq!(metadata.writing_language, "zh-CN");
        assert!(metadata.strict_chapter_markup);
    }

    #[test]
    fn prepares_a_complete_offline_preview_and_writes_only_after_hash_confirmation() {
        let root = TestRoot::new("prepared-preview");
        fs::write(root.path().join("notes.txt"), b"keep me").expect("fixture must write");
        let inspection = inspect_project_root(root.path(), ProjectScanMode::Create)
            .expect("safe non-empty root must scan");
        let input = ProjectCreateInputV1 {
            schema_version: 1,
            title: "雾港来信".to_owned(),
            author_display_name: Some("林舟".to_owned()),
            genre: Some("悬疑".to_owned()),
            premise: Some("一封迟到十年的信改变了港城。".to_owned()),
        };

        let prepared =
            prepare_project_create(&inspection, input).expect("preview must be prepared offline");
        let preview = prepared.preview();

        assert_eq!(preview.schema_version, 1);
        assert_eq!(preview.directories.len(), 11);
        assert_eq!(preview.files.len(), 4);
        assert_eq!(preview.existing_unrelated_entry_count, 1);
        assert_eq!(preview.preview_hash.len(), 64);
        assert!(Uuid::parse_str(&preview.project_id).is_ok());
        let project_markdown = preview
            .files
            .iter()
            .find(|file| file.relative_path == "project.md")
            .expect("project.md preview must exist");
        assert!(project_markdown.content.contains("title: \"雾港来信\""));
        assert!(project_markdown
            .content
            .contains("authorDisplayName: \"林舟\""));
        assert!(project_markdown.content.contains("genre: \"悬疑\""));
        assert!(project_markdown
            .content
            .contains("一封迟到十年的信改变了港城。"));
        assert!(project_markdown.content.contains("## 一句话前提"));
        assert!(!root.path().join("project.md").exists());
        assert!(!root.path().join(".novelgainker").exists());
        assert_eq!(
            fs::read(root.path().join("notes.txt")).expect("original must remain"),
            b"keep me"
        );

        let handle = create_prepared_project(&prepared, &preview.preview_hash)
            .expect("matching preview must create");
        assert_eq!(handle.project_id(), preview.project_id);
        assert_eq!(
            fs::read_to_string(root.path().join("project.md")).expect("project.md must read"),
            project_markdown.content
        );
        let metadata_text = fs::read_to_string(root.path().join(".novelgainker/project.json"))
            .expect("metadata must read");
        assert!(!metadata_text.contains("林舟"));
        assert!(!metadata_text.contains("港城"));
        assert!(!metadata_text.contains("apiKey"));
    }

    #[test]
    fn rejects_stale_preview_hash_and_changed_root_without_writing_scaffold() {
        let hash_root = TestRoot::new("stale-preview-hash");
        let hash_inspection = inspect_project_root(hash_root.path(), ProjectScanMode::Create)
            .expect("empty root must scan");
        let hash_prepared =
            prepare_project_create(&hash_inspection, ProjectCreateInputV1::default_local())
                .expect("preview must prepare");
        let error = create_prepared_project(&hash_prepared, &"0".repeat(64))
            .expect_err("stale hash must be rejected");
        assert_eq!(error.code, "project.scaffold_changed");
        assert!(!hash_root.path().join("project.md").exists());

        let changed_root = TestRoot::new("prepared-root-change");
        let changed_inspection = inspect_project_root(changed_root.path(), ProjectScanMode::Create)
            .expect("empty root must scan");
        let changed_prepared =
            prepare_project_create(&changed_inspection, ProjectCreateInputV1::default_local())
                .expect("preview must prepare");
        fs::write(changed_root.path().join("notes.txt"), b"arrived later")
            .expect("fixture must write");
        let error =
            create_prepared_project(&changed_prepared, &changed_prepared.preview().preview_hash)
                .expect_err("changed root must be rejected");
        assert_eq!(error.code, "project.scaffold_changed");
        assert!(!changed_root.path().join("project.md").exists());
        assert_eq!(
            fs::read(changed_root.path().join("notes.txt")).expect("new file must remain"),
            b"arrived later"
        );
    }

    #[test]
    fn rejects_uncontrolled_or_oversized_project_fields_without_writing() {
        for title in ["", "危险\n标题", &"长".repeat(121), &"😀".repeat(100)] {
            let root = TestRoot::new("invalid-create-input");
            let inspection = inspect_project_root(root.path(), ProjectScanMode::Create)
                .expect("empty root must scan");
            let mut input = ProjectCreateInputV1::default_local();
            input.title = title.to_owned();
            let error =
                prepare_project_create(&inspection, input).expect_err("input must be rejected");
            assert_eq!(error.code, "project.metadata_invalid");
            assert_eq!(
                fs::read_dir(root.path()).expect("root must read").count(),
                0
            );
        }
    }

    #[test]
    fn non_empty_scan_is_read_only_and_reports_only_target_conflicts() {
        let root = TestRoot::new("preflight");
        fs::write(root.path().join("notes.txt"), b"keep me").expect("fixture must write");
        fs::write(root.path().join("project.md"), b"original").expect("fixture must write");

        let inspection = inspect_project_root(root.path(), ProjectScanMode::Create)
            .expect("non-empty root must scan");

        assert!(!inspection.preview().can_confirm);
        assert!(inspection
            .preview()
            .conflicts
            .iter()
            .any(|conflict| conflict.relative_path == "project.md"));
        assert_eq!(
            fs::read(root.path().join("project.md")).expect("fixture must remain"),
            b"original"
        );
        assert!(!root.path().join(".novelgainker").exists());
    }

    #[test]
    fn a_changed_candidate_cannot_overwrite_a_new_target() {
        let root = TestRoot::new("race");
        let inspection = inspect_project_root(root.path(), ProjectScanMode::Create)
            .expect("empty root must scan");
        fs::write(root.path().join("project.md"), b"arrived later").expect("fixture must write");

        let error = create_project(&inspection).expect_err("changed root must be rejected");

        assert_eq!(error.code, "project.scaffold_changed");
        assert_eq!(
            fs::read(root.path().join("project.md")).expect("fixture must remain"),
            b"arrived later"
        );
    }

    #[test]
    fn every_injected_transaction_step_rolls_back_only_created_entries() {
        let total_operations = super::PROJECT_DIRECTORIES.len() + super::PROJECT_FILES.len();
        for failed_operation in 1..=total_operations {
            let root = TestRoot::new(&format!("rollback-{failed_operation}"));
            fs::write(root.path().join("notes.txt"), b"keep me").expect("fixture must write");
            let inspection = inspect_project_root(root.path(), ProjectScanMode::Create)
                .expect("safe non-empty root must scan");

            let prepared =
                prepare_project_create(&inspection, ProjectCreateInputV1::default_local())
                    .expect("preview must prepare");
            let preview_hash = prepared.preview().preview_hash.clone();
            create_prepared_project_impl(&prepared, &preview_hash, Some(failed_operation))
                .expect_err("failure must be injected");

            assert_eq!(
                fs::read(root.path().join("notes.txt")).expect("original must remain"),
                b"keep me"
            );
            for directory in super::PROJECT_DIRECTORIES {
                assert!(!root.path().join(directory).exists());
            }
            for file in super::PROJECT_FILES {
                assert!(!root.path().join(file).exists());
            }
        }
    }

    #[test]
    fn opens_a_valid_project_and_imports_metadata_only_after_confirmation() {
        let native_root = TestRoot::new("native-open");
        let create_inspection = inspect_project_root(native_root.path(), ProjectScanMode::Create)
            .expect("native root must scan");
        create_project(&create_inspection).expect("native project must create");
        let open_inspection = inspect_project_root(native_root.path(), ProjectScanMode::Open)
            .expect("native project must scan");
        assert!(!open_inspection.preview().requires_metadata_confirmation);
        assert!(open_project(&open_inspection, false).is_ok());

        let imported_root = TestRoot::new("import-open");
        fs::write(imported_root.path().join("draft.md"), b"# Draft\n").expect("fixture must write");
        let import_inspection = inspect_project_root(imported_root.path(), ProjectScanMode::Open)
            .expect("markdown project must scan");
        assert!(import_inspection.preview().requires_metadata_confirmation);
        assert!(!imported_root.path().join(".novelgainker").exists());
        let error = open_project(&import_inspection, false)
            .expect_err("metadata write must require confirmation");
        assert_eq!(error.code, "project.confirmation_required");
        let handle = open_project(&import_inspection, true).expect("confirmed import must open");
        assert_eq!(handle.writing_language(), "zh-CN");
        assert!(imported_root
            .path()
            .join(".novelgainker/project.json")
            .is_file());
        assert_eq!(
            fs::read(imported_root.path().join("draft.md")).expect("draft must remain"),
            b"# Draft\n"
        );
    }

    #[test]
    fn rejects_unsupported_corrupt_and_sensitive_metadata() {
        for (label, json, expected_code) in [
            (
                "unsupported",
                r#"{"schemaVersion":2,"projectId":"00000000-0000-4000-8000-000000000000","workflowPackVersion":"1.0.0","defaultProviderId":"openai","writingLanguage":"zh-CN","strictChapterMarkup":true}"#,
                "project.unsupported_schema",
            ),
            ("corrupt", "{", "project.invalid_metadata"),
            (
                "secret-field",
                r#"{"schemaVersion":1,"projectId":"00000000-0000-4000-8000-000000000000","workflowPackVersion":"1.0.0","defaultProviderId":"openai","writingLanguage":"zh-CN","strictChapterMarkup":true,"apiKey":"sk-secret"}"#,
                "project.invalid_metadata",
            ),
        ] {
            let root = TestRoot::new(label);
            fs::create_dir(root.path().join(".novelgainker")).expect("fixture dir must write");
            fs::write(root.path().join(".novelgainker/project.json"), json)
                .expect("fixture metadata must write");

            let error = inspect_project_root(root.path(), ProjectScanMode::Open)
                .expect_err("invalid metadata must be rejected");
            assert_eq!(error.code, expected_code);
        }
    }
}
