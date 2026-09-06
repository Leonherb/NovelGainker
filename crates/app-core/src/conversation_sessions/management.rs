use super::*;
use crate::conversation::{read_or_new_uncoordinated, ConversationTurnStatus};
use crate::coordination::ProjectWriteLease;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(
    tag = "action",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SessionManagement {
    Branch {
        conversation_session_id: String,
        request_id: String,
        boundary_turn_id: String,
        name: String,
        expected_revision: u64,
    },
    Clear {
        conversation_session_id: String,
        request_id: String,
        name: String,
        expected_revision: u64,
    },
    Archive {
        conversation_session_id: String,
        expected_revision: u64,
    },
    Restore {
        conversation_session_id: String,
        expected_revision: u64,
    },
}
impl SessionManagement {
    fn action(&self) -> &'static str {
        match self {
            Self::Branch { .. } => "branch",
            Self::Clear { .. } => "clear",
            Self::Archive { .. } => "archive",
            Self::Restore { .. } => "restore",
        }
    }
    fn source(&self) -> &str {
        match self {
            Self::Branch {
                conversation_session_id,
                ..
            }
            | Self::Clear {
                conversation_session_id,
                ..
            }
            | Self::Archive {
                conversation_session_id,
                ..
            }
            | Self::Restore {
                conversation_session_id,
                ..
            } => conversation_session_id,
        }
    }
    fn revision(&self) -> u64 {
        match self {
            Self::Branch {
                expected_revision, ..
            }
            | Self::Clear {
                expected_revision, ..
            }
            | Self::Archive {
                expected_revision, ..
            }
            | Self::Restore {
                expected_revision, ..
            } => *expected_revision,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionManagementResult {
    pub schema_version: u16,
    pub action: String,
    pub source_session_id: String,
    pub target_session_id: String,
    pub changed: bool,
    pub catalog: SessionPage,
}
fn response(
    catalog: &Catalog,
    operation: &SessionManagement,
    target: &str,
    changed: bool,
) -> Result<SessionManagementResult, AppError> {
    Ok(SessionManagementResult {
        schema_version: 1,
        action: operation.action().into(),
        source_session_id: operation.source().into(),
        target_session_id: target.into(),
        changed,
        catalog: page(catalog, false, 0)?,
    })
}

// A bounded, Host-derived ID also fingerprints the operation on replay, without
// putting raw content or an executable history into the catalog.
fn prepared_id(handle: &ProjectHandle, operation: &SessionManagement) -> Result<String, AppError> {
    let (action, source, request, boundary, name) = match operation {
        SessionManagement::Branch {
            conversation_session_id,
            request_id,
            boundary_turn_id,
            name,
            ..
        } => (
            "branch",
            conversation_session_id,
            request_id,
            boundary_turn_id.as_str(),
            name,
        ),
        SessionManagement::Clear {
            conversation_session_id,
            request_id,
            name,
            ..
        } => ("clear", conversation_session_id, request_id, "", name),
        _ => return Err(error("sessions.invalid")),
    };
    let bytes = serde_json::to_vec(&(handle.project_id(), action, source, request, boundary, name))
        .map_err(|_| error("sessions.invalid"))?;
    Ok(derived_id(&bytes))
}
fn derived_id(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut id: [u8; 16] = [0; 16];
    id.copy_from_slice(&digest[..16]);
    id[6] = (id[6] & 15) | 64;
    id[8] = (id[8] & 63) | 128;
    Uuid::from_bytes(id).to_string()
}

pub fn manage_session(
    handle: &ProjectHandle,
    lease: &ProjectWriteLease,
    operation: &SessionManagement,
    now: u64,
) -> Result<SessionManagementResult, AppError> {
    if !lease.is_for_project(&handle.coordination_key()) {
        return Err(AppError::project_writer_required());
    }
    if !valid_id(operation.source()) || now == 0 {
        return Err(error("sessions.invalid"));
    }
    let lock = coordination(handle)?;
    let _guard = lock.lock().map_err(|_| error("sessions.unavailable"))?;
    let mut catalog = ensure_catalog(handle, now)?;
    if catalog.run.is_some() {
        return Err(error("sessions.run_active"));
    }
    let index = catalog
        .entries
        .iter()
        .position(|entry| entry.session_id == operation.source())
        .ok_or_else(|| error("sessions.not_found"))?;
    match operation {
        SessionManagement::Archive { .. } | SessionManagement::Restore { .. } => {
            let archive = matches!(operation, SessionManagement::Archive { .. });
            if catalog.entries[index].archived == archive {
                return response(&catalog, operation, operation.source(), false);
            }
            compare_revision(&catalog, operation.revision())?;
            if !archive
                && catalog
                    .entries
                    .iter()
                    .filter(|entry| !entry.archived)
                    .count()
                    >= MAX_ACTIVE
            {
                return Err(error("sessions.full"));
            }
            catalog.entries[index].archived = archive;
            catalog.entries[index].updated_at_ms = now.max(catalog.entries[index].created_at_ms);
            if !archive {
                catalog.active_session_id = Some(operation.source().into());
            } else if catalog.active_session_id.as_deref() == Some(operation.source()) {
                catalog.active_session_id = catalog
                    .entries
                    .iter()
                    .find(|entry| !entry.archived)
                    .map(|entry| entry.session_id.clone());
            }
            catalog.revision += 1;
            write_catalog(handle, &catalog)?;
            response(&catalog, operation, operation.source(), true)
        }
        SessionManagement::Branch {
            request_id, name, ..
        }
        | SessionManagement::Clear {
            request_id, name, ..
        } => {
            if !valid_id(request_id) || !valid_name(name) {
                return Err(error("sessions.invalid"));
            }
            let id = prepared_id(handle, operation)?;
            if let Some(existing) = catalog
                .entries
                .iter()
                .find(|entry| entry.creation_request_id.as_deref() == Some(request_id))
            {
                return if existing.session_id == id {
                    response(&catalog, operation, &existing.session_id, false)
                } else {
                    Err(error("sessions.revision_conflict"))
                };
            }
            compare_revision(&catalog, operation.revision())?;
            let clearing = matches!(operation, SessionManagement::Clear { .. });
            if clearing && catalog.entries[index].archived {
                return Err(error("sessions.archived"));
            }
            let active = catalog
                .entries
                .iter()
                .filter(|entry| !entry.archived)
                .count();
            if catalog.entries.len() >= MAX_ENTRIES || (!clearing && active >= MAX_ACTIVE) {
                return Err(error("sessions.full"));
            }
            let source = binding(handle, &catalog, operation.source())?;
            let mut copied = read_or_new_uncoordinated(&source)?;
            if copied
                .turns
                .iter()
                .any(|turn| turn.status == ConversationTurnStatus::Running)
            {
                return Err(error("sessions.run_active"));
            }
            if let SessionManagement::Branch {
                boundary_turn_id, ..
            } = operation
            {
                if !valid_id(boundary_turn_id) {
                    return Err(error("sessions.invalid"));
                }
                let boundary = copied
                    .turns
                    .iter()
                    .position(|turn| turn.turn_id == *boundary_turn_id)
                    .ok_or_else(|| error("sessions.boundary_invalid"))?;
                copied.turns.truncate(boundary + 1);
                for turn in &mut copied.turns {
                    turn.retry_state = None;
                    turn.turn_id = derived_id(format!("{id}:turn:{}", turn.turn_id).as_bytes());
                    for message in &mut turn.messages {
                        message.message_id =
                            derived_id(format!("{id}:message:{}", message.message_id).as_bytes());
                    }
                }
            } else {
                copied.turns.clear();
            }
            copied.session_id = id.clone();
            let bound = handle.with_session_binding(SessionBinding {
                id: id.clone(),
                legacy: false,
            });
            let bytes =
                serde_json::to_vec_pretty(&copied).map_err(|_| error("sessions.invalid"))?;
            quota::check_write_locked(
                &bound,
                &catalog,
                StoreKind::Conversation,
                bytes.len() as u64 + 1,
            )?;
            let target = store_path(&bound, StoreKind::Conversation, true)?;
            if target
                .try_exists()
                .map_err(|_| error("sessions.unavailable"))?
            {
                // Exact prepared bytes are the only orphan that may be reused.
                let existing = read_bounded(&target, StoreKind::Conversation.single_limit())?;
                let mut expected = bytes.clone();
                expected.push(b'\n');
                if existing != expected {
                    return Err(error("sessions.revision_conflict"));
                }
            } else {
                write_new(&target, &bytes)?;
            }
            if clearing {
                catalog.entries[index].archived = true;
                catalog.entries[index].updated_at_ms =
                    now.max(catalog.entries[index].created_at_ms);
            }
            catalog.entries.push(SessionEntry {
                session_id: id.clone(),
                name: name.clone(),
                archived: false,
                created_at_ms: now,
                updated_at_ms: now,
                creation_request_id: Some(request_id.clone()),
            });
            catalog.active_session_id = Some(id.clone());
            catalog.revision += 1;
            write_catalog(handle, &catalog)?;
            response(&catalog, operation, &id, true)
        }
    }
}
