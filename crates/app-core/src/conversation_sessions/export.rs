//! Explicit visible-text export. Host chooses the path through a native dialog;
//! no tool or IPC caller supplies paths or replacement bytes.
use super::*;
use crate::conversation::{ConversationRole, ConversationTurnStatus};

const MAX_EXPORT_BYTES: usize = 32 * 1024 * 1024;
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationExportFormat {
    Markdown,
    Json,
}
impl ConversationExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Markdown => "md",
            Self::Json => "json",
        }
    }
}
pub struct PreparedConversationExport {
    handle: ProjectHandle,
    source_hash: String,
    bytes: Vec<u8>,
    format: ConversationExportFormat,
}
impl PreparedConversationExport {
    pub fn bytes_len(&self) -> usize {
        self.bytes.len()
    }
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationExportReceipt {
    pub schema_version: u16,
    pub file_name: String,
    pub bytes: usize,
    pub sha256: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VisibleExport<'a> {
    schema_version: u16,
    turns: Vec<VisibleTurn<'a>>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VisibleTurn<'a> {
    status: ConversationTurnStatus,
    messages: Vec<VisibleMessage<'a>>,
}
#[derive(Serialize)]
struct VisibleMessage<'a> {
    role: ConversationRole,
    content: &'a str,
}

fn snapshot(handle: &ProjectHandle) -> Result<ConversationSession, AppError> {
    let session = crate::conversation::read_conversation(handle)?;
    if session
        .turns
        .iter()
        .any(|turn| turn.status == ConversationTurnStatus::Running)
    {
        return Err(error("sessions.run_active"));
    }
    Ok(session)
}
fn fingerprint(session: &ConversationSession) -> Result<String, AppError> {
    Ok(sha256_hex(
        &serde_json::to_vec(session).map_err(|_| error("sessions.invalid"))?,
    ))
}
pub fn prepare_export(
    handle: &ProjectHandle,
    session_id: &str,
    format: ConversationExportFormat,
) -> Result<PreparedConversationExport, AppError> {
    let handle = bind_existing_session(handle, Some(session_id))?;
    let session = snapshot(&handle)?;
    let bytes = match format {
        ConversationExportFormat::Json => serde_json::to_vec_pretty(&VisibleExport {
            schema_version: 1,
            turns: session
                .turns
                .iter()
                .map(|turn| VisibleTurn {
                    status: turn.status,
                    messages: turn
                        .messages
                        .iter()
                        .map(|message| VisibleMessage {
                            role: message.role,
                            content: &message.content,
                        })
                        .collect(),
                })
                .collect(),
        })
        .map_err(|_| error("sessions.invalid"))?,
        ConversationExportFormat::Markdown => {
            let mut output = String::from("# 可见聊天记录\n\n");
            for turn in &session.turns {
                let status = match turn.status {
                    ConversationTurnStatus::Completed => "已完成",
                    ConversationTurnStatus::Cancelled => "已停止",
                    ConversationTurnStatus::Failed => "失败",
                    ConversationTurnStatus::NotGenerated => "未生成",
                    ConversationTurnStatus::Running => return Err(error("sessions.run_active")),
                };
                output.push_str(&format!("## {status}\n\n"));
                for message in &turn.messages {
                    output.push_str(match message.role {
                        ConversationRole::User => "### 用户\n\n",
                        ConversationRole::Assistant => "### AI\n\n",
                    });
                    output.push_str(&message.content);
                    output.push_str("\n\n");
                }
            }
            output.into_bytes()
        }
    };
    if bytes.len() > MAX_EXPORT_BYTES {
        return Err(error("sessions.export_full"));
    }
    Ok(PreparedConversationExport {
        handle,
        source_hash: fingerprint(&session)?,
        bytes,
        format,
    })
}

fn checked_target(target: &Path, roots: &[PathBuf], extension: &str) -> Result<PathBuf, AppError> {
    #[cfg(windows)]
    if !matches!(target.components().next(), Some(std::path::Component::Prefix(prefix))
        if matches!(prefix.kind(), std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)))
    {
        // Local files only. Reject UNC/device namespaces before any filesystem lookup.
        return Err(error("sessions.export_boundary"));
    }
    if !target.is_absolute()
        || target.components().any(|part| {
            matches!(
                part,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(error("sessions.export_boundary"));
    }
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| error("sessions.export_boundary"))?;
    if name.len() > 180
        || name.starts_with('.')
        || name.ends_with([' ', '.'])
        || name
            .chars()
            .any(|c| c.is_control() || ":/\\<>|?*".contains(c))
        || target.extension().and_then(|ext| ext.to_str()) != Some(extension)
    {
        return Err(error("sessions.export_boundary"));
    }
    let parent = target
        .parent()
        .ok_or_else(|| error("sessions.export_boundary"))?;
    for ancestor in parent.ancestors() {
        let meta = fs::symlink_metadata(ancestor).map_err(|_| error("sessions.export_boundary"))?;
        if !meta.is_dir() || is_link_or_reparse(&meta) {
            return Err(error("sessions.export_boundary"));
        }
        // Also reject unregistered NovelGainker projects, not just this window's roots.
        if fs::symlink_metadata(ancestor.join(".novelgainker")).is_ok() {
            return Err(error("sessions.export_boundary"));
        }
    }
    let parent = fs::canonicalize(parent).map_err(|_| error("sessions.export_boundary"))?;
    for root in roots {
        let root = fs::canonicalize(root).map_err(|_| error("sessions.export_boundary"))?;
        if parent.starts_with(root) {
            return Err(error("sessions.export_boundary"));
        }
    }
    let path = parent.join(name);
    match fs::symlink_metadata(&path) {
        Err(failure) if failure.kind() == std::io::ErrorKind::NotFound => Ok(path),
        _ => Err(error("sessions.export_exists")),
    }
}

/// Consumes one prepared snapshot. The caller must consume its opaque token
/// before calling, and supplies only a native-dialog-approved exact target.
pub fn commit_export(
    prepared: PreparedConversationExport,
    target: &Path,
    registered_projects: &[ProjectHandle],
) -> Result<ConversationExportReceipt, AppError> {
    let roots = registered_projects
        .iter()
        .chain(std::iter::once(&prepared.handle))
        .map(|handle| handle.root().to_path_buf())
        .collect::<Vec<_>>();
    let path = checked_target(target, &roots, prepared.format.extension())?;
    if fingerprint(&snapshot(&prepared.handle)?)? != prepared.source_hash {
        return Err(error("sessions.export_stale"));
    }
    let temporary = path.with_file_name(format!(".novelgainker-export-{}.tmp", Uuid::new_v4()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|_| error("sessions.export_unavailable"))?;
    let result = (|| {
        file.write_all(&prepared.bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| error("sessions.export_unavailable"))?;
        drop(file);
        if checked_target(target, &roots, prepared.format.extension())? != path
            || fingerprint(&snapshot(&prepared.handle)?)? != prepared.source_hash
        {
            return Err(error("sessions.export_stale"));
        }
        crate::publication::store::promote(&temporary, &path)
            .map_err(|_| error("sessions.export_unavailable"))?;
        Ok(ConversationExportReceipt {
            schema_version: 1,
            file_name: path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| error("sessions.export_boundary"))?
                .into(),
            bytes: prepared.bytes.len(),
            sha256: sha256_hex(&prepared.bytes),
        })
    })();
    // Only our UUID temporary, never the destination or existing user data.
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
