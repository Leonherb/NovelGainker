use crate::credential_vault::ProviderId;
use crate::project::ProjectHandle;
use crate::provider_core::Usage;
use crate::AppError;
use novelgainker_agent::action_contracts::RegisteredToolIdV1;
use novelgainker_agent::contracts::{ContentHashV1, ProjectIdV1, SessionIdV1, TurnIdV1};
use novelgainker_agent::persistence_contracts::{
    reject_sensitive_text, UndoRecordStatusV1, UndoRecordV1, MAX_UNDO_BYTES_PER_PROJECT,
    UNDO_RECORD_LIMIT,
};
use novelgainker_agent::runtime_registry::{is_am2_tool, runtime_skill};
use novelgainker_workflows::manager::{WorkflowRunRecord, WorkflowRunState};
use novelgainker_workflows::registry;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use uuid::{Uuid, Version};

pub const APP_STATE_SCHEMA_VERSION: u16 = 10;
pub const APP_STATE_DATABASE_NAME: &str = "app-state-v1.sqlite3";
pub const MAX_RECENT_PROJECTS: usize = 12;
pub const MIN_RIGHT_PANEL_WIDTH: u16 = 278;
pub const MAX_RIGHT_PANEL_WIDTH: u16 = 1_600;
pub const MAX_AGENT_TURN_METADATA: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSessionStateRecord {
    pub project_id: String,
    pub agent_session_id: String,
    pub provider_id: ProviderId,
    pub preset_version: u16,
    pub model: String,
    pub skill_pack_hash: String,
    pub policy: String,
    pub summary_text: Option<String>,
    pub summary_hash: Option<String>,
    pub summary_version: u32,
    pub clear_generation: u32,
    pub updated_at_ms: u64,
}

impl AgentSessionStateRecord {
    fn validate(&self) -> Result<(), ()> {
        validate_uuid(&self.project_id)?;
        let project_id = format!("prj_{}", self.project_id.replace('-', ""));
        ProjectIdV1::parse(project_id).map_err(|_| ())?;
        SessionIdV1::parse(self.agent_session_id.clone()).map_err(|_| ())?;
        if self.preset_version != 1
            || self.model != crate::provider_core::preset(self.provider_id).model
            || self.skill_pack_hash.len() != 64
            || !self
                .skill_pack_hash
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || !matches!(
                self.policy.as_str(),
                "plan_only" | "review_every_write" | "auto_apply_safe"
            )
            || self.updated_at_ms == 0
        {
            return Err(());
        }
        match (&self.summary_text, &self.summary_hash) {
            (None, None) if self.summary_version == 0 => Ok(()),
            (Some(text), Some(hash))
                if self.summary_version > 0
                    && !text.is_empty()
                    && text.len()
                        <= novelgainker_agent::contracts::SESSION_SUMMARY_MAX_UTF8_BYTES
                            as usize
                    && reject_sensitive_text(text).is_ok()
                    && ContentHashV1::digest(text.as_bytes()).as_str() == hash =>
            {
                Ok(())
            }
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTurnMetadataRecord {
    pub project_id: String,
    pub agent_session_id: String,
    pub turn_id: String,
    pub terminal_state: String,
    pub skill_ids: Vec<String>,
    pub tool_ids: Vec<String>,
    pub action_count: u16,
    pub tool_call_count: u16,
    pub action_plan_hash: String,
    pub updated_at_ms: u64,
}

impl AgentTurnMetadataRecord {
    fn validate(&self) -> Result<(), ()> {
        validate_uuid(&self.project_id)?;
        SessionIdV1::parse(self.agent_session_id.clone()).map_err(|_| ())?;
        TurnIdV1::parse(self.turn_id.clone()).map_err(|_| ())?;
        if !matches!(
            self.terminal_state.as_str(),
            "completed" | "cancelled" | "failed"
        ) || self.skill_ids.is_empty()
            || self.skill_ids.len() > novelgainker_agent::SKILL_IDS.len()
            || self
                .skill_ids
                .iter()
                .any(|skill| runtime_skill(skill).is_none())
            || self.tool_ids.len() > novelgainker_agent::contracts::MAX_TOOL_CALLS_PER_TURN as usize
            || self.tool_ids.iter().any(|tool| {
                RegisteredToolIdV1::parse(tool)
                    .map(|tool| !is_am2_tool(tool))
                    .unwrap_or(true)
            })
            || self.action_count > novelgainker_agent::contracts::MAX_AGENT_STEPS_PER_TURN
            || self.tool_call_count > novelgainker_agent::contracts::MAX_TOOL_CALLS_PER_TURN
            || usize::from(self.tool_call_count) != self.tool_ids.len()
            || self.action_plan_hash.len() != 64
            || !self
                .action_plan_hash
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || self.updated_at_ms == 0
        {
            return Err(());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentMessageIndexRecord {
    pub project_id: String,
    pub agent_session_id: String,
    pub message_id: String,
    pub turn_id: String,
    pub ordinal: u32,
    pub role: String,
    pub body_hash: String,
    pub utf8_bytes: u32,
    pub created_at_ms: u64,
}

impl AgentMessageIndexRecord {
    fn validate(&self) -> Result<(), ()> {
        validate_uuid(&self.project_id)?;
        SessionIdV1::parse(self.agent_session_id.clone()).map_err(|_| ())?;
        TurnIdV1::parse(self.turn_id.clone()).map_err(|_| ())?;
        if !self.message_id.starts_with("msg_")
            || self.message_id.len() < 12
            || !matches!(self.role.as_str(), "user" | "assistant" | "tool")
            || self.ordinal == 0
            || self.utf8_bytes == 0
            || self.body_hash.len() != 64
            || !self.body_hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            || self.created_at_ms == 0
        {
            return Err(());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentCheckpointRecord {
    pub session: AgentSessionStateRecord,
    pub turn: AgentTurnMetadataRecord,
    pub messages: Vec<AgentMessageIndexRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentClearCounts {
    pub sessions_removed: u16,
    pub turns_removed: u16,
    pub message_indexes_removed: u16,
    pub undo_records_removed: u16,
    pub canon_write_count: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouterRunStatus {
    Completed,
    Failed,
    Cancelled,
}

impl RouterRunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

impl TryFrom<&str> for RouterRunStatus {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouterRunMetadata {
    pub run_id: String,
    pub provider_id: ProviderId,
    pub workflow_id: Option<String>,
    pub confidence: Option<String>,
    pub status: RouterRunStatus,
    pub usage: Option<Usage>,
    pub updated_at: i64,
}

impl RouterRunMetadata {
    fn validate(&self) -> Result<(), ()> {
        let run_id = Uuid::parse_str(&self.run_id).map_err(|_| ())?;
        if run_id.get_version() != Some(Version::Random) || self.updated_at < 0 {
            return Err(());
        }
        let has_valid_decision =
            self.workflow_id.as_deref().is_some_and(|workflow_id| {
                registry().workflows.iter().any(|workflow| {
                    workflow.id == workflow_id && workflow.execution_available && workflow.routable
                })
            }) && matches!(self.confidence.as_deref(), Some("high" | "medium" | "low"));
        if (self.status == RouterRunStatus::Completed) != has_valid_decision
            || (self.status != RouterRunStatus::Completed
                && (self.workflow_id.is_some() || self.confidence.is_some()))
        {
            return Err(());
        }
        if self.usage.is_some_and(|usage| {
            usage.total_tokens < usage.input_tokens || usage.total_tokens < usage.output_tokens
        }) {
            return Err(());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiPreferences {
    pub left_collapsed: bool,
    pub right_collapsed: bool,
    pub api_card_expanded: bool,
    pub right_panel_width: Option<u16>,
}

impl Default for UiPreferences {
    fn default() -> Self {
        Self {
            left_collapsed: false,
            right_collapsed: false,
            api_card_expanded: true,
            right_panel_width: None,
        }
    }
}

impl UiPreferences {
    pub fn validate(self) -> Result<Self, AppError> {
        if self
            .right_panel_width
            .is_some_and(|width| !(MIN_RIGHT_PANEL_WIDTH..=MAX_RIGHT_PANEL_WIDTH).contains(&width))
        {
            return Err(AppError::app_state_unavailable());
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentProject {
    pub project_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFilePreference {
    pub project_id: String,
    pub current_relative_file: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStateSnapshot {
    pub schema_version: u16,
    pub ui: UiPreferences,
    pub recent_projects: Vec<RecentProject>,
    pub project_files: Vec<ProjectFilePreference>,
    pub recovered_with_defaults: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSettingRecord {
    pub provider_id: ProviderId,
    pub preset_version: u16,
    pub credential_ref: Option<String>,
    pub is_selected: bool,
    pub connection_status: String,
    pub checked_at: Option<i64>,
}

impl AppStateSnapshot {
    fn defaults(recovered_with_defaults: bool) -> Self {
        Self {
            schema_version: APP_STATE_SCHEMA_VERSION,
            ui: UiPreferences::default(),
            recent_projects: Vec::new(),
            project_files: Vec::new(),
            recovered_with_defaults,
        }
    }
}

enum AppStateBackend {
    Ready(Connection),
    Degraded,
}

pub struct AppStateRepository {
    database_path: PathBuf,
    backend: Mutex<AppStateBackend>,
}

impl AppStateRepository {
    pub fn open(app_state_root: &Path) -> Self {
        let database_path = app_state_root.join(APP_STATE_DATABASE_NAME);
        let backend = fs::create_dir_all(app_state_root)
            .ok()
            .and_then(|()| Connection::open(&database_path).ok())
            .and_then(|connection| initialize_connection(connection).ok())
            .map_or(AppStateBackend::Degraded, AppStateBackend::Ready);
        Self {
            database_path,
            backend: Mutex::new(backend),
        }
    }

    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub fn snapshot(&self) -> AppStateSnapshot {
        let Ok(mut backend) = self.backend.lock() else {
            return AppStateSnapshot::defaults(true);
        };
        let AppStateBackend::Ready(connection) = &mut *backend else {
            return AppStateSnapshot::defaults(true);
        };
        match load_snapshot(connection) {
            Ok(snapshot) => snapshot,
            Err(()) => {
                *backend = AppStateBackend::Degraded;
                AppStateSnapshot::defaults(true)
            }
        }
    }

    pub fn save_ui(&self, preferences: UiPreferences) -> Result<(), AppError> {
        let preferences = preferences.validate()?;
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO ui_preferences (
                    singleton, left_collapsed, right_collapsed, api_card_expanded, right_panel_width
                 ) VALUES (1, ?1, ?2, ?3, ?4)
                 ON CONFLICT(singleton) DO UPDATE SET
                    left_collapsed = excluded.left_collapsed,
                    right_collapsed = excluded.right_collapsed,
                    api_card_expanded = excluded.api_card_expanded,
                    right_panel_width = excluded.right_panel_width",
                params![
                    preferences.left_collapsed,
                    preferences.right_collapsed,
                    preferences.api_card_expanded,
                    preferences.right_panel_width,
                ],
            )?;
            Ok(())
        })
    }

    pub fn record_project(&self, handle: &ProjectHandle) -> Result<(), AppError> {
        self.with_connection(|connection| {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let next_order: i64 = transaction.query_row(
                "SELECT COALESCE(MAX(last_opened_order), 0) + 1 FROM recent_projects",
                [],
                |row| row.get(0),
            )?;
            transaction.execute(
                "INSERT INTO recent_projects (project_id, root_path, display_name, last_opened_order)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(project_id) DO UPDATE SET
                    root_path = excluded.root_path,
                    display_name = excluded.display_name,
                    last_opened_order = excluded.last_opened_order",
                params![
                    handle.project_id(),
                    handle.root().to_string_lossy(),
                    handle.display_name(),
                    next_order,
                ],
            )?;
            transaction.execute(
                "DELETE FROM recent_projects
                 WHERE project_id IN (
                    SELECT project_id FROM recent_projects
                    ORDER BY last_opened_order DESC
                    LIMIT -1 OFFSET ?1
                 )",
                [MAX_RECENT_PROJECTS as i64],
            )?;
            transaction.commit()?;
            Ok(())
        })
    }

    pub fn record_current_file(
        &self,
        project_id: &str,
        relative_path: &str,
    ) -> Result<(), AppError> {
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO project_files (project_id, current_relative_file)
                 VALUES (?1, ?2)
                 ON CONFLICT(project_id) DO UPDATE SET
                    current_relative_file = excluded.current_relative_file",
                params![project_id, relative_path],
            )?;
            Ok(())
        })
    }

    pub fn current_file(&self, project_id: &str) -> Option<String> {
        self.query_optional_string(
            "SELECT current_relative_file FROM project_files WHERE project_id = ?1",
            project_id,
        )
    }

    pub fn recent_project_root(&self, project_id: &str) -> Option<PathBuf> {
        self.query_optional_string(
            "SELECT root_path FROM recent_projects WHERE project_id = ?1",
            project_id,
        )
        .map(PathBuf::from)
        .filter(|path| is_available_directory(path))
    }

    pub fn forget_project(&self, project_id: &str) {
        let _ = self.with_connection(|connection| {
            connection.execute(
                "DELETE FROM recent_projects WHERE project_id = ?1",
                [project_id],
            )?;
            Ok(())
        });
    }

    pub fn provider_settings(&self) -> Result<Vec<ProviderSettingRecord>, AppError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT provider_id, preset_version, credential_ref, is_selected,
                        connection_status, checked_at
                 FROM provider_settings ORDER BY provider_id",
            )?;
            let records = statement
                .query_map([], |row| {
                    let provider_id = row.get::<_, String>(0)?;
                    let provider_id = ProviderId::try_from(provider_id.as_str())
                        .map_err(|_| rusqlite::Error::InvalidQuery)?;
                    Ok(ProviderSettingRecord {
                        provider_id,
                        preset_version: row.get(1)?,
                        credential_ref: row.get(2)?,
                        is_selected: row.get(3)?,
                        connection_status: row.get(4)?,
                        checked_at: row.get(5)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            if records.len() != ProviderId::ALL.len() {
                return Err(rusqlite::Error::InvalidQuery);
            }
            Ok(records)
        })
    }

    pub fn save_provider_credential_ref(
        &self,
        provider: ProviderId,
        credential_ref: &str,
    ) -> Result<(), AppError> {
        self.with_connection(|connection| {
            let changed = connection.execute(
                "UPDATE provider_settings
                 SET credential_ref = ?2, connection_status = 'untested', checked_at = NULL
                 WHERE provider_id = ?1",
                params![provider.as_str(), credential_ref],
            )?;
            if changed != 1 {
                return Err(rusqlite::Error::InvalidQuery);
            }
            Ok(())
        })
    }

    pub fn clear_provider_credential_ref(&self, provider: ProviderId) -> Result<(), AppError> {
        self.with_connection(|connection| {
            let changed = connection.execute(
                "UPDATE provider_settings
                 SET credential_ref = NULL, connection_status = 'unconfigured', checked_at = NULL
                 WHERE provider_id = ?1",
                [provider.as_str()],
            )?;
            if changed != 1 {
                return Err(rusqlite::Error::InvalidQuery);
            }
            Ok(())
        })
    }

    pub fn update_provider_connection_status(
        &self,
        provider: ProviderId,
        connection_status: &str,
        checked_at: i64,
    ) -> Result<(), AppError> {
        if !matches!(
            connection_status,
            "connected" | "authentication_failed" | "unreachable"
        ) || checked_at < 0
        {
            return Err(AppError::app_state_unavailable());
        }
        self.with_connection(|connection| {
            let changed = connection.execute(
                "UPDATE provider_settings
                 SET connection_status = ?2, checked_at = ?3
                 WHERE provider_id = ?1 AND credential_ref IS NOT NULL",
                params![provider.as_str(), connection_status, checked_at],
            )?;
            if changed != 1 {
                return Err(rusqlite::Error::InvalidQuery);
            }
            Ok(())
        })
    }

    pub fn select_provider(&self, provider: ProviderId) -> Result<(), AppError> {
        self.with_connection(|connection| {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            transaction.execute(
                "UPDATE provider_settings SET is_selected = 0 WHERE is_selected = 1",
                [],
            )?;
            let changed = transaction.execute(
                "UPDATE provider_settings SET is_selected = 1 WHERE provider_id = ?1",
                [provider.as_str()],
            )?;
            if changed != 1 {
                return Err(rusqlite::Error::InvalidQuery);
            }
            transaction.commit()?;
            Ok(())
        })
    }

    pub fn router_runs(&self) -> Result<Vec<RouterRunMetadata>, AppError> {
        let mut backend = self
            .backend
            .lock()
            .map_err(|_| AppError::app_state_unavailable())?;
        let AppStateBackend::Ready(connection) = &mut *backend else {
            return Err(AppError::app_state_unavailable());
        };
        let mut statement = connection
            .prepare(
                "SELECT run_id, provider_id, workflow_id, confidence, status,
                        input_tokens, output_tokens, total_tokens, updated_at
                 FROM router_runs ORDER BY updated_at, run_id",
            )
            .map_err(|_| AppError::app_state_unavailable())?;
        let rows = statement
            .query_map([], |row| {
                let provider = row.get::<_, String>(1)?;
                let provider_id = ProviderId::try_from(provider.as_str())
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                let status = row.get::<_, String>(4)?;
                let status = RouterRunStatus::try_from(status.as_str())
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                let input_tokens = row.get::<_, Option<u64>>(5)?;
                let output_tokens = row.get::<_, Option<u64>>(6)?;
                let total_tokens = row.get::<_, Option<u64>>(7)?;
                let usage = match (input_tokens, output_tokens, total_tokens) {
                    (Some(input_tokens), Some(output_tokens), Some(total_tokens)) => Some(Usage {
                        input_tokens,
                        output_tokens,
                        total_tokens,
                    }),
                    (None, None, None) => None,
                    _ => return Err(rusqlite::Error::InvalidQuery),
                };
                let record = RouterRunMetadata {
                    run_id: row.get(0)?,
                    provider_id,
                    workflow_id: row.get(2)?,
                    confidence: row.get(3)?,
                    status,
                    usage,
                    updated_at: row.get(8)?,
                };
                record
                    .validate()
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                Ok(record)
            })
            .map_err(|_| AppError::app_state_unavailable())?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|_| AppError::app_state_unavailable())
    }

    pub fn save_router_run(&self, record: &RouterRunMetadata) -> Result<(), AppError> {
        record
            .validate()
            .map_err(|_| AppError::app_state_unavailable())?;
        self.with_connection(|connection| {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let usage = record.usage;
            let changed = transaction.execute(
                "INSERT INTO router_runs (
                    run_id, provider_id, workflow_id, confidence, status,
                    input_tokens, output_tokens, total_tokens, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    record.run_id,
                    record.provider_id.as_str(),
                    record.workflow_id,
                    record.confidence,
                    record.status.as_str(),
                    usage.map(|value| value.input_tokens),
                    usage.map(|value| value.output_tokens),
                    usage.map(|value| value.total_tokens),
                    record.updated_at,
                ],
            )?;
            if changed != 1 {
                return Err(rusqlite::Error::InvalidQuery);
            }
            transaction.execute(
                "DELETE FROM router_runs WHERE run_id IN (
                    SELECT run_id FROM router_runs
                    ORDER BY updated_at DESC, run_id DESC
                    LIMIT -1 OFFSET 20
                 )",
                [],
            )?;
            transaction.commit()?;
            Ok(())
        })
    }
    pub fn workflow_runs(&self) -> Result<Vec<WorkflowRunRecord>, AppError> {
        let mut backend = self
            .backend
            .lock()
            .map_err(|_| AppError::app_state_unavailable())?;
        let AppStateBackend::Ready(connection) = &mut *backend else {
            return Err(AppError::app_state_unavailable());
        };
        let mut statement = connection
            .prepare(
                "SELECT run_id, project_id, workflow_id, workflow_version, pack_hash, state,
                        sequence, provider_id, preset_version, model, manifest_hash,
                        proposal_hash, target_relative_path, target_source_hash, confirmed_at,
                        updated_at
                 FROM workflow_runs ORDER BY updated_at, sequence, run_id",
            )
            .map_err(|_| AppError::app_state_unavailable())?;
        let rows = statement
            .query_map([], |row| {
                let state = row.get::<_, String>(5)?;
                let state = WorkflowRunState::try_from(state.as_str())
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                let record = WorkflowRunRecord {
                    run_id: row.get(0)?,
                    project_id: row.get(1)?,
                    workflow_id: row.get(2)?,
                    workflow_version: row.get(3)?,
                    pack_hash: row.get(4)?,
                    state,
                    sequence: row.get(6)?,
                    provider_id: row.get(7)?,
                    preset_version: row.get(8)?,
                    model: row.get(9)?,
                    manifest_hash: row.get(10)?,
                    proposal_hash: row.get(11)?,
                    target_relative_path: row.get(12)?,
                    target_source_hash: row.get(13)?,
                    confirmed_at: row.get(14)?,
                    updated_at: row.get(15)?,
                };
                record
                    .validate_persisted_checkpoint()
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                Ok(record)
            })
            .map_err(|_| AppError::app_state_unavailable())?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|_| AppError::app_state_unavailable())
    }

    pub fn save_workflow_run(&self, record: &WorkflowRunRecord) -> Result<(), AppError> {
        record
            .validate()
            .map_err(|_| AppError::app_state_unavailable())?;
        let mut backend = self
            .backend
            .lock()
            .map_err(|_| AppError::app_state_unavailable())?;
        let AppStateBackend::Ready(connection) = &mut *backend else {
            return Err(AppError::app_state_unavailable());
        };
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| AppError::app_state_unavailable())?;
        let changed = transaction
            .execute(
                "INSERT INTO workflow_runs (
                    run_id, project_id, workflow_id, workflow_version, pack_hash, state, sequence,
                    provider_id, preset_version, model, manifest_hash, proposal_hash,
                    target_relative_path, target_source_hash, confirmed_at, updated_at
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16
                 )
                 ON CONFLICT(run_id) DO UPDATE SET
                    state = excluded.state,
                    sequence = excluded.sequence,
                    manifest_hash = excluded.manifest_hash,
                    proposal_hash = excluded.proposal_hash,
                    target_relative_path = excluded.target_relative_path,
                    target_source_hash = excluded.target_source_hash,
                    confirmed_at = excluded.confirmed_at,
                    updated_at = excluded.updated_at
                 WHERE workflow_runs.project_id = excluded.project_id
                   AND workflow_runs.workflow_id = excluded.workflow_id
                   AND workflow_runs.workflow_version = excluded.workflow_version
                   AND workflow_runs.pack_hash = excluded.pack_hash
                   AND workflow_runs.provider_id IS excluded.provider_id
                   AND workflow_runs.preset_version IS excluded.preset_version
                   AND workflow_runs.model IS excluded.model
                   AND excluded.sequence = workflow_runs.sequence + 1",
                params![
                    record.run_id,
                    record.project_id,
                    record.workflow_id,
                    record.workflow_version,
                    record.pack_hash,
                    record.state.as_str(),
                    record.sequence,
                    record.provider_id,
                    record.preset_version,
                    record.model,
                    record.manifest_hash,
                    record.proposal_hash,
                    record.target_relative_path,
                    record.target_source_hash,
                    record.confirmed_at,
                    record.updated_at,
                ],
            )
            .map_err(|_| AppError::app_state_unavailable())?;
        if changed != 1 {
            return Err(AppError::app_state_unavailable());
        }
        transaction
            .execute(
                "DELETE FROM workflow_runs WHERE run_id IN (
                    SELECT run_id FROM workflow_runs
                    WHERE state IN ('completed', 'failed', 'cancelled')
                    ORDER BY updated_at DESC, sequence DESC, run_id DESC
                    LIMIT -1 OFFSET 20
                 )",
                [],
            )
            .map_err(|_| AppError::app_state_unavailable())?;
        transaction
            .commit()
            .map_err(|_| AppError::app_state_unavailable())
    }

    pub fn agent_session(
        &self,
        project_id: &str,
    ) -> Result<Option<AgentSessionStateRecord>, AppError> {
        validate_uuid(project_id).map_err(|_| AppError::app_state_unavailable())?;
        self.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT project_id, agent_session_id, provider_id, preset_version, model,
                            skill_pack_hash, policy, summary_text, summary_hash, summary_version,
                            clear_generation, updated_at_ms
                     FROM agent_sessions WHERE project_id = ?1",
                    [project_id],
                    |row| {
                        let provider = row.get::<_, String>(2)?;
                        let provider_id = ProviderId::try_from(provider.as_str())
                            .map_err(|_| rusqlite::Error::InvalidQuery)?;
                        let record = AgentSessionStateRecord {
                            project_id: row.get(0)?,
                            agent_session_id: row.get(1)?,
                            provider_id,
                            preset_version: row.get(3)?,
                            model: row.get(4)?,
                            skill_pack_hash: row.get(5)?,
                            policy: row.get(6)?,
                            summary_text: row.get(7)?,
                            summary_hash: row.get(8)?,
                            summary_version: row.get(9)?,
                            clear_generation: row.get(10)?,
                            updated_at_ms: row.get(11)?,
                        };
                        record
                            .validate()
                            .map_err(|_| rusqlite::Error::InvalidQuery)?;
                        Ok(record)
                    },
                )
                .optional()
        })
    }

    pub fn agent_turns(&self, project_id: &str) -> Result<Vec<AgentTurnMetadataRecord>, AppError> {
        validate_uuid(project_id).map_err(|_| AppError::app_state_unavailable())?;
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT project_id, agent_session_id, turn_id, terminal_state, skill_ids_json,
                        tool_ids_json, action_count, tool_call_count, action_plan_hash, updated_at_ms
                 FROM agent_turns WHERE project_id = ?1
                 ORDER BY updated_at_ms DESC, turn_id DESC LIMIT 100",
            )?;
            let records = statement
                .query_map([project_id], |row| {
                    let skill_ids_json = row.get::<_, String>(4)?;
                    let tool_ids_json = row.get::<_, String>(5)?;
                    let record = AgentTurnMetadataRecord {
                        project_id: row.get(0)?,
                        agent_session_id: row.get(1)?,
                        turn_id: row.get(2)?,
                        terminal_state: row.get(3)?,
                        skill_ids: serde_json::from_str(&skill_ids_json)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        tool_ids: serde_json::from_str(&tool_ids_json)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        action_count: row.get(6)?,
                        tool_call_count: row.get(7)?,
                        action_plan_hash: row.get(8)?,
                        updated_at_ms: row.get(9)?,
                    };
                    record
                        .validate()
                        .map_err(|_| rusqlite::Error::InvalidQuery)?;
                    Ok(record)
                })?
                .collect();
            records
        })
    }

    pub fn agent_message_indexes(
        &self,
        project_id: &str,
    ) -> Result<Vec<AgentMessageIndexRecord>, AppError> {
        validate_uuid(project_id).map_err(|_| AppError::app_state_unavailable())?;
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT project_id, agent_session_id, message_id, turn_id, ordinal, role,
                        body_hash, utf8_bytes, created_at_ms
                 FROM agent_msg_index WHERE project_id = ?1
                 ORDER BY ordinal ASC LIMIT 200",
            )?;
            let records = statement
                .query_map([project_id], |row| {
                    let record = AgentMessageIndexRecord {
                        project_id: row.get(0)?,
                        agent_session_id: row.get(1)?,
                        message_id: row.get(2)?,
                        turn_id: row.get(3)?,
                        ordinal: row.get(4)?,
                        role: row.get(5)?,
                        body_hash: row.get(6)?,
                        utf8_bytes: row.get(7)?,
                        created_at_ms: row.get(8)?,
                    };
                    record
                        .validate()
                        .map_err(|_| rusqlite::Error::InvalidQuery)?;
                    Ok(record)
                })?
                .collect();
            records
        })
    }

    pub fn save_agent_session(&self, record: &AgentSessionStateRecord) -> Result<(), AppError> {
        record
            .validate()
            .map_err(|_| AppError::app_state_unavailable())?;
        self.with_connection(|connection| {
            save_agent_session_row(connection, record)?;
            Ok(())
        })
    }

    pub fn save_agent_checkpoint(
        &self,
        checkpoint: &AgentCheckpointRecord,
    ) -> Result<(), AppError> {
        checkpoint
            .session
            .validate()
            .map_err(|_| AppError::app_state_unavailable())?;
        checkpoint
            .turn
            .validate()
            .map_err(|_| AppError::app_state_unavailable())?;
        if checkpoint.turn.project_id != checkpoint.session.project_id
            || checkpoint.turn.agent_session_id != checkpoint.session.agent_session_id
            || checkpoint.messages.is_empty()
            || checkpoint.messages.iter().any(|message| {
                message.validate().is_err()
                    || message.project_id != checkpoint.session.project_id
                    || message.agent_session_id != checkpoint.session.agent_session_id
                    || message.turn_id != checkpoint.turn.turn_id
            })
        {
            return Err(AppError::app_state_unavailable());
        }
        self.with_connection(|connection| {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            save_agent_session_row(&transaction, &checkpoint.session)?;
            let skill_ids = serde_json::to_string(&checkpoint.turn.skill_ids)
                .map_err(|_| rusqlite::Error::InvalidQuery)?;
            let tool_ids = serde_json::to_string(&checkpoint.turn.tool_ids)
                .map_err(|_| rusqlite::Error::InvalidQuery)?;
            transaction.execute(
                "INSERT INTO agent_turns (
                    project_id, agent_session_id, turn_id, terminal_state, skill_ids_json,
                    tool_ids_json, action_count, tool_call_count, action_plan_hash, updated_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    checkpoint.turn.project_id,
                    checkpoint.turn.agent_session_id,
                    checkpoint.turn.turn_id,
                    checkpoint.turn.terminal_state,
                    skill_ids,
                    tool_ids,
                    checkpoint.turn.action_count,
                    checkpoint.turn.tool_call_count,
                    checkpoint.turn.action_plan_hash,
                    checkpoint.turn.updated_at_ms,
                ],
            )?;
            for message in &checkpoint.messages {
                transaction.execute(
                    "INSERT INTO agent_msg_index (
                        project_id, agent_session_id, message_id, turn_id, ordinal, role,
                        body_hash, utf8_bytes, created_at_ms
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        message.project_id,
                        message.agent_session_id,
                        message.message_id,
                        message.turn_id,
                        message.ordinal,
                        message.role,
                        message.body_hash,
                        message.utf8_bytes,
                        message.created_at_ms,
                    ],
                )?;
            }
            transaction.execute(
                "DELETE FROM agent_turns WHERE turn_id IN (
                    SELECT turn_id FROM agent_turns WHERE project_id = ?1
                    ORDER BY updated_at_ms DESC, turn_id DESC LIMIT -1 OFFSET 100
                 )",
                [&checkpoint.session.project_id],
            )?;
            transaction.execute(
                "DELETE FROM agent_msg_index WHERE message_id IN (
                    SELECT message_id FROM agent_msg_index WHERE project_id = ?1
                    ORDER BY ordinal DESC, message_id DESC LIMIT -1 OFFSET 200
                 )",
                [&checkpoint.session.project_id],
            )?;
            transaction.commit()
        })
    }

    pub fn agent_undo_records(&self, project_id: &str) -> Result<Vec<UndoRecordV1>, AppError> {
        validate_uuid(project_id).map_err(|_| AppError::app_state_unavailable())?;
        let expected_project_id = agent_project_id(project_id)?;
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT record_json FROM agent_undo_records
                 WHERE project_id = ?1 ORDER BY created_at_ms DESC, transaction_hash DESC",
            )?;
            let records = statement
                .query_map([project_id], |row| {
                    let json = row.get::<_, String>(0)?;
                    let record: UndoRecordV1 =
                        serde_json::from_str(&json).map_err(|_| rusqlite::Error::InvalidQuery)?;
                    record
                        .validate()
                        .map_err(|_| rusqlite::Error::InvalidQuery)?;
                    if record.project_id.as_str() != expected_project_id {
                        return Err(rusqlite::Error::InvalidQuery);
                    }
                    Ok(record)
                })?
                .collect();
            records
        })
    }

    pub fn save_agent_undo_record(
        &self,
        project_id: &str,
        record: &UndoRecordV1,
    ) -> Result<(), AppError> {
        validate_uuid(project_id).map_err(|_| AppError::app_state_unavailable())?;
        record
            .validate()
            .map_err(|_| AppError::app_state_unavailable())?;
        if record.project_id.as_str() != agent_project_id(project_id)?
            || record.status != UndoRecordStatusV1::Ready
        {
            return Err(AppError::app_state_unavailable());
        }
        let json = serde_json::to_string(record).map_err(|_| AppError::app_state_unavailable())?;
        self.with_connection(|connection| {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            transaction.execute(
                "DELETE FROM agent_undo_records
                 WHERE project_id = ?1 AND status != 'ready'",
                [project_id],
            )?;
            let (count, total): (u32, u32) = transaction.query_row(
                "SELECT COUNT(*), COALESCE(SUM(encrypted_bytes), 0)
                 FROM agent_undo_records WHERE project_id = ?1 AND status = 'ready'",
                [project_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            if count >= UNDO_RECORD_LIMIT as u32
                || total
                    .checked_add(record.encrypted_bytes)
                    .is_none_or(|value| value > MAX_UNDO_BYTES_PER_PROJECT)
            {
                return Err(rusqlite::Error::InvalidQuery);
            }
            transaction.execute(
                "INSERT INTO agent_undo_records (
                    transaction_hash, project_id, turn_id, blob_id, status, encrypted_bytes,
                    created_at_ms, expires_at_ms, clear_generation, record_json
                 ) VALUES (?1, ?2, ?3, ?4, 'ready', ?5, ?6, ?7, ?8, ?9)",
                params![
                    record.transaction_hash.as_str(),
                    project_id,
                    record.turn_id.as_str(),
                    record.blob_id.as_str(),
                    record.encrypted_bytes,
                    record.created_at_ms.0,
                    record.expires_at_ms.0,
                    record.clear_generation,
                    json,
                ],
            )?;
            transaction.commit()
        })
    }

    pub fn set_agent_undo_status(
        &self,
        project_id: &str,
        transaction_hash: &str,
        status: UndoRecordStatusV1,
    ) -> Result<UndoRecordV1, AppError> {
        validate_uuid(project_id).map_err(|_| AppError::app_state_unavailable())?;
        if transaction_hash.len() != 64
            || !transaction_hash
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || status == UndoRecordStatusV1::Ready
        {
            return Err(AppError::app_state_unavailable());
        }
        self.with_connection(|connection| {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let json: String = transaction.query_row(
                "SELECT record_json FROM agent_undo_records
                 WHERE project_id = ?1 AND transaction_hash = ?2 AND status = 'ready'",
                params![project_id, transaction_hash],
                |row| row.get(0),
            )?;
            let mut record: UndoRecordV1 =
                serde_json::from_str(&json).map_err(|_| rusqlite::Error::InvalidQuery)?;
            record.status = status;
            record
                .validate()
                .map_err(|_| rusqlite::Error::InvalidQuery)?;
            let updated_json =
                serde_json::to_string(&record).map_err(|_| rusqlite::Error::InvalidQuery)?;
            let changed = transaction.execute(
                "UPDATE agent_undo_records SET status = ?3, record_json = ?4
                 WHERE project_id = ?1 AND transaction_hash = ?2 AND status = 'ready'",
                params![
                    project_id,
                    transaction_hash,
                    undo_status(status),
                    updated_json
                ],
            )?;
            if changed != 1 {
                return Err(rusqlite::Error::InvalidQuery);
            }
            transaction.commit()?;
            Ok(record)
        })
    }

    pub fn clear_agent_state(&self, project_id: &str) -> Result<AgentClearCounts, AppError> {
        validate_uuid(project_id).map_err(|_| AppError::app_state_unavailable())?;
        self.with_connection(|connection| {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let undo_records_removed = transaction.execute(
                "DELETE FROM agent_undo_records WHERE project_id = ?1",
                [project_id],
            )?;
            let message_indexes_removed = transaction.execute(
                "DELETE FROM agent_msg_index WHERE project_id = ?1",
                [project_id],
            )?;
            let turns_removed = transaction.execute(
                "DELETE FROM agent_turns WHERE project_id = ?1",
                [project_id],
            )?;
            let sessions_removed = transaction.execute(
                "DELETE FROM agent_sessions WHERE project_id = ?1",
                [project_id],
            )?;
            transaction.commit()?;
            Ok(AgentClearCounts {
                sessions_removed: u16::try_from(sessions_removed).unwrap_or(u16::MAX),
                turns_removed: u16::try_from(turns_removed).unwrap_or(u16::MAX),
                message_indexes_removed: u16::try_from(message_indexes_removed).unwrap_or(u16::MAX),
                undo_records_removed: u16::try_from(undo_records_removed).unwrap_or(u16::MAX),
                canon_write_count: 0,
            })
        })
    }

    fn query_optional_string(&self, sql: &str, parameter: &str) -> Option<String> {
        let mut backend = self.backend.lock().ok()?;
        let AppStateBackend::Ready(connection) = &mut *backend else {
            return None;
        };
        match connection
            .query_row(sql, [parameter], |row| row.get::<_, String>(0))
            .optional()
        {
            Ok(value) => value,
            Err(_) => {
                *backend = AppStateBackend::Degraded;
                None
            }
        }
    }

    fn with_connection<T>(
        &self,
        operation: impl FnOnce(&mut Connection) -> rusqlite::Result<T>,
    ) -> Result<T, AppError> {
        let mut backend = self
            .backend
            .lock()
            .map_err(|_| AppError::app_state_unavailable())?;
        let AppStateBackend::Ready(connection) = &mut *backend else {
            return Err(AppError::app_state_unavailable());
        };
        match operation(connection) {
            Ok(value) => Ok(value),
            Err(_) => {
                *backend = AppStateBackend::Degraded;
                Err(AppError::app_state_unavailable())
            }
        }
    }
}

fn validate_uuid(value: &str) -> Result<(), ()> {
    let parsed = Uuid::parse_str(value).map_err(|_| ())?;
    if parsed.get_version() != Some(Version::Random) {
        return Err(());
    }
    Ok(())
}

fn undo_status(status: UndoRecordStatusV1) -> &'static str {
    match status {
        UndoRecordStatusV1::Ready => "ready",
        UndoRecordStatusV1::Consumed => "consumed",
        UndoRecordStatusV1::Expired => "expired",
        UndoRecordStatusV1::HashConflict => "hash_conflict",
        UndoRecordStatusV1::DecryptFailed => "decrypt_failed",
        UndoRecordStatusV1::Cleared => "cleared",
    }
}

pub fn agent_project_id(project_id: &str) -> Result<String, AppError> {
    validate_uuid(project_id).map_err(|_| AppError::app_state_unavailable())?;
    Ok(format!("prj_{}", project_id.replace('-', "")))
}

fn save_agent_session_row(
    connection: &Connection,
    record: &AgentSessionStateRecord,
) -> rusqlite::Result<()> {
    let changed = connection.execute(
        "INSERT INTO agent_sessions (
            project_id, agent_session_id, provider_id, preset_version, model, skill_pack_hash,
            policy, summary_text, summary_hash, summary_version, clear_generation, updated_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(project_id) DO UPDATE SET
            agent_session_id = excluded.agent_session_id,
            provider_id = excluded.provider_id,
            preset_version = excluded.preset_version,
            model = excluded.model,
            skill_pack_hash = excluded.skill_pack_hash,
            policy = excluded.policy,
            summary_text = excluded.summary_text,
            summary_hash = excluded.summary_hash,
            summary_version = excluded.summary_version,
            clear_generation = excluded.clear_generation,
            updated_at_ms = excluded.updated_at_ms",
        params![
            record.project_id,
            record.agent_session_id,
            record.provider_id.as_str(),
            record.preset_version,
            record.model,
            record.skill_pack_hash,
            record.policy,
            record.summary_text,
            record.summary_hash,
            record.summary_version,
            record.clear_generation,
            record.updated_at_ms,
        ],
    )?;
    if changed != 1 {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(())
}

fn initialize_connection(mut connection: Connection) -> rusqlite::Result<Connection> {
    connection.pragma_update(None, "foreign_keys", "ON")?;
    let schema_version: u16 =
        connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    match schema_version {
        0 => {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            transaction.execute_batch(
                "CREATE TABLE ui_preferences (
                singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                left_collapsed INTEGER NOT NULL CHECK (left_collapsed IN (0, 1)),
                right_collapsed INTEGER NOT NULL CHECK (right_collapsed IN (0, 1)),
                api_card_expanded INTEGER NOT NULL CHECK (api_card_expanded IN (0, 1)),
                right_panel_width INTEGER NULL CHECK (
                    right_panel_width IS NULL OR right_panel_width BETWEEN 278 AND 1600
                )
                );
                CREATE TABLE recent_projects (
                project_id TEXT PRIMARY KEY NOT NULL,
                root_path TEXT UNIQUE NOT NULL,
                display_name TEXT NOT NULL,
                last_opened_order INTEGER NOT NULL
                );
                CREATE TABLE project_files (
                project_id TEXT PRIMARY KEY NOT NULL,
                current_relative_file TEXT NOT NULL,
                FOREIGN KEY (project_id) REFERENCES recent_projects(project_id) ON DELETE CASCADE
                );",
            )?;
            create_provider_settings(&transaction)?;
            create_workflow_runs(&transaction)?;
            create_router_runs(&transaction)?;
            create_agent_state(&transaction)?;
            transaction.pragma_update(None, "user_version", APP_STATE_SCHEMA_VERSION)?;
            transaction.commit()?;
        }
        1 => {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            create_provider_settings(&transaction)?;
            create_workflow_runs(&transaction)?;
            create_router_runs(&transaction)?;
            create_agent_state(&transaction)?;
            transaction.pragma_update(None, "user_version", APP_STATE_SCHEMA_VERSION)?;
            transaction.commit()?;
        }
        2 => {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            create_workflow_runs(&transaction)?;
            create_router_runs(&transaction)?;
            create_agent_state(&transaction)?;
            transaction.pragma_update(None, "user_version", APP_STATE_SCHEMA_VERSION)?;
            transaction.commit()?;
        }
        3 => {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            migrate_workflow_runs_v4(&transaction)?;
            create_router_runs(&transaction)?;
            create_agent_state(&transaction)?;
            transaction.pragma_update(None, "user_version", APP_STATE_SCHEMA_VERSION)?;
            transaction.commit()?;
        }
        4 => {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            migrate_workflow_runs_v5(&transaction)?;
            create_router_runs(&transaction)?;
            create_agent_state(&transaction)?;
            transaction.pragma_update(None, "user_version", APP_STATE_SCHEMA_VERSION)?;
            transaction.commit()?;
        }
        5 => {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            migrate_workflow_runs_v6(&transaction)?;
            create_router_runs(&transaction)?;
            create_agent_state(&transaction)?;
            transaction.pragma_update(None, "user_version", APP_STATE_SCHEMA_VERSION)?;
            transaction.commit()?;
        }
        6 => {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            migrate_workflow_runs_v7(&transaction)?;
            create_router_runs(&transaction)?;
            create_agent_state(&transaction)?;
            transaction.pragma_update(None, "user_version", APP_STATE_SCHEMA_VERSION)?;
            transaction.commit()?;
        }
        7 => {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            create_router_runs(&transaction)?;
            create_agent_state(&transaction)?;
            transaction.pragma_update(None, "user_version", APP_STATE_SCHEMA_VERSION)?;
            transaction.commit()?;
        }
        8 => {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            create_agent_state(&transaction)?;
            transaction.pragma_update(None, "user_version", APP_STATE_SCHEMA_VERSION)?;
            transaction.commit()?;
        }
        9 => {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            migrate_agent_state_v10(&transaction)?;
            transaction.pragma_update(None, "user_version", APP_STATE_SCHEMA_VERSION)?;
            transaction.commit()?;
        }
        APP_STATE_SCHEMA_VERSION => {}
        _ => return Err(rusqlite::Error::InvalidQuery),
    }
    Ok(connection)
}

fn create_provider_settings(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "CREATE TABLE provider_settings (
            provider_id TEXT PRIMARY KEY NOT NULL CHECK (provider_id IN ('deepseek', 'openai')),
            preset_version INTEGER NOT NULL CHECK (preset_version = 1),
            credential_ref TEXT NULL CHECK (
                credential_ref IS NULL OR length(credential_ref) BETWEEN 1 AND 180
            ),
            is_selected INTEGER NOT NULL CHECK (is_selected IN (0, 1)),
            connection_status TEXT NOT NULL CHECK (
                connection_status IN (
                    'unconfigured', 'untested', 'connected', 'authentication_failed',
                    'unreachable'
                )
            ),
            checked_at INTEGER NULL CHECK (checked_at IS NULL OR checked_at >= 0)
        );
        CREATE UNIQUE INDEX provider_settings_one_selected
            ON provider_settings(is_selected) WHERE is_selected = 1;
        INSERT INTO provider_settings (
            provider_id, preset_version, credential_ref, is_selected, connection_status, checked_at
        ) VALUES
            ('deepseek', 1, NULL, 0, 'unconfigured', NULL),
            ('openai', 1, NULL, 1, 'unconfigured', NULL);",
    )
}

fn create_workflow_runs(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "CREATE TABLE workflow_runs (
            run_id TEXT PRIMARY KEY NOT NULL CHECK (length(run_id) = 36),
            project_id TEXT NOT NULL CHECK (length(project_id) = 36),
            workflow_id TEXT NOT NULL CHECK (workflow_id IN ('novelgainker.character-create', 'novelgainker.setting-create', 'novelgainker.material-create', 'novelgainker.plot-create')),
            workflow_version INTEGER NOT NULL CHECK (
                (workflow_id = 'novelgainker.character-create' AND workflow_version IN (1, 2))
                OR (workflow_id != 'novelgainker.character-create' AND workflow_version = 1)
            ),
            pack_hash TEXT NOT NULL CHECK (length(pack_hash) = 64),
            state TEXT NOT NULL CHECK (state IN (
                'created', 'routed', 'awaiting_input', 'context_ready', 'generating',
                'proposal_ready', 'awaiting_confirmation', 'applying', 'validating',
                'completed', 'failed', 'cancelled', 'recovery_required'
            )),
            sequence INTEGER NOT NULL CHECK (sequence >= 0),
            provider_id TEXT NULL CHECK (provider_id IN ('deepseek', 'openai')),
            preset_version INTEGER NULL CHECK (preset_version = 1),
            model TEXT NULL CHECK (model IN ('deepseek-v4-pro', 'gpt-5.6-terra')),
            manifest_hash TEXT NULL CHECK (manifest_hash IS NULL OR length(manifest_hash) = 64),
            proposal_hash TEXT NULL CHECK (proposal_hash IS NULL OR length(proposal_hash) = 64),
            target_relative_path TEXT NULL CHECK (
                target_relative_path IS NULL OR length(target_relative_path) BETWEEN 1 AND 240
            ),
            target_source_hash TEXT NULL CHECK (
                target_source_hash IS NULL OR length(target_source_hash) = 64
            ),
            confirmed_at INTEGER NULL CHECK (confirmed_at IS NULL OR confirmed_at >= 0),
            updated_at INTEGER NOT NULL CHECK (updated_at >= 0),
            CHECK (
                (provider_id IS NULL AND preset_version IS NULL AND model IS NULL)
                OR (provider_id = 'deepseek' AND preset_version = 1 AND model = 'deepseek-v4-pro')
                OR (provider_id = 'openai' AND preset_version = 1 AND model = 'gpt-5.6-terra')
            )
        );
        CREATE UNIQUE INDEX workflow_runs_one_active
            ON workflow_runs((1)) WHERE state NOT IN ('completed', 'failed', 'cancelled');",
    )
}

fn create_router_runs(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS router_runs (
            run_id TEXT PRIMARY KEY NOT NULL CHECK (length(run_id) = 36),
            provider_id TEXT NOT NULL CHECK (provider_id IN ('deepseek', 'openai')),
            workflow_id TEXT NULL CHECK (workflow_id IN (
                'novelgainker.character-create', 'novelgainker.project-create',
                'novelgainker.setting-create', 'novelgainker.material-create',
                'novelgainker.plot-create'
            )),
            confidence TEXT NULL CHECK (confidence IN ('high', 'medium', 'low')),
            status TEXT NOT NULL CHECK (status IN ('completed', 'failed', 'cancelled')),
            input_tokens INTEGER NULL CHECK (input_tokens IS NULL OR input_tokens >= 0),
            output_tokens INTEGER NULL CHECK (output_tokens IS NULL OR output_tokens >= 0),
            total_tokens INTEGER NULL CHECK (total_tokens IS NULL OR total_tokens >= 0),
            updated_at INTEGER NOT NULL CHECK (updated_at >= 0),
            CHECK (
                (status = 'completed' AND workflow_id IS NOT NULL AND confidence IS NOT NULL)
                OR (status != 'completed' AND workflow_id IS NULL AND confidence IS NULL)
            ),
            CHECK (
                (input_tokens IS NULL AND output_tokens IS NULL AND total_tokens IS NULL)
                OR (input_tokens IS NOT NULL AND output_tokens IS NOT NULL AND total_tokens IS NOT NULL)
            )
        );",
    )
}

fn create_agent_state(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS agent_sessions (
            project_id TEXT PRIMARY KEY NOT NULL CHECK (length(project_id) = 36),
            agent_session_id TEXT UNIQUE NOT NULL CHECK (
                length(agent_session_id) BETWEEN 12 AND 96 AND agent_session_id LIKE 'ses_%'
            ),
            provider_id TEXT NOT NULL CHECK (provider_id IN ('deepseek', 'openai')),
            preset_version INTEGER NOT NULL CHECK (preset_version = 1),
            model TEXT NOT NULL CHECK (model IN ('deepseek-v4-pro', 'gpt-5.6-terra')),
            skill_pack_hash TEXT NOT NULL CHECK (length(skill_pack_hash) = 64),
            policy TEXT NOT NULL CHECK (
                policy IN ('plan_only', 'review_every_write', 'auto_apply_safe')
            ),
            summary_text TEXT NULL CHECK (
                summary_text IS NULL OR length(CAST(summary_text AS BLOB)) BETWEEN 1 AND 32768
            ),
            summary_hash TEXT NULL CHECK (summary_hash IS NULL OR length(summary_hash) = 64),
            summary_version INTEGER NOT NULL CHECK (summary_version >= 0),
            clear_generation INTEGER NOT NULL CHECK (clear_generation >= 0),
            updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms > 0),
            CHECK (
                (summary_text IS NULL AND summary_hash IS NULL AND summary_version = 0)
                OR (summary_text IS NOT NULL AND summary_hash IS NOT NULL AND summary_version > 0)
            ),
            CHECK (
                (provider_id = 'deepseek' AND model = 'deepseek-v4-pro')
                OR (provider_id = 'openai' AND model = 'gpt-5.6-terra')
            ),
            FOREIGN KEY (project_id) REFERENCES recent_projects(project_id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS agent_turns (
            turn_id TEXT PRIMARY KEY NOT NULL CHECK (
                length(turn_id) BETWEEN 12 AND 96 AND turn_id LIKE 'trn_%'
            ),
            project_id TEXT NOT NULL CHECK (length(project_id) = 36),
            agent_session_id TEXT NOT NULL CHECK (agent_session_id LIKE 'ses_%'),
            terminal_state TEXT NOT NULL CHECK (
                terminal_state IN ('completed', 'cancelled', 'failed')
            ),
            skill_ids_json TEXT NOT NULL CHECK (length(skill_ids_json) BETWEEN 3 AND 1024),
            tool_ids_json TEXT NOT NULL CHECK (length(tool_ids_json) BETWEEN 2 AND 4096),
            action_count INTEGER NOT NULL CHECK (action_count BETWEEN 0 AND 8),
            tool_call_count INTEGER NOT NULL CHECK (tool_call_count BETWEEN 0 AND 16),
            action_plan_hash TEXT NOT NULL CHECK (length(action_plan_hash) = 64),
            updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms > 0),
            FOREIGN KEY (project_id) REFERENCES recent_projects(project_id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS agent_turns_project_updated
            ON agent_turns(project_id, updated_at_ms DESC);
        CREATE TABLE IF NOT EXISTS agent_msg_index (
            message_id TEXT PRIMARY KEY NOT NULL CHECK (
                length(message_id) BETWEEN 12 AND 96 AND message_id LIKE 'msg_%'
            ),
            project_id TEXT NOT NULL CHECK (length(project_id) = 36),
            agent_session_id TEXT NOT NULL CHECK (agent_session_id LIKE 'ses_%'),
            turn_id TEXT NOT NULL CHECK (turn_id LIKE 'trn_%'),
            ordinal INTEGER NOT NULL CHECK (ordinal > 0),
            role TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'tool')),
            body_hash TEXT NOT NULL CHECK (length(body_hash) = 64),
            utf8_bytes INTEGER NOT NULL CHECK (utf8_bytes > 0),
            created_at_ms INTEGER NOT NULL CHECK (created_at_ms > 0),
            UNIQUE(project_id, ordinal),
            FOREIGN KEY (project_id) REFERENCES recent_projects(project_id) ON DELETE CASCADE,
            FOREIGN KEY (turn_id) REFERENCES agent_turns(turn_id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS agent_undo_records (
            transaction_hash TEXT PRIMARY KEY NOT NULL CHECK (length(transaction_hash) = 64),
            project_id TEXT NOT NULL CHECK (length(project_id) = 36),
            turn_id TEXT UNIQUE NOT NULL CHECK (turn_id LIKE 'trn_%'),
            blob_id TEXT UNIQUE NOT NULL CHECK (blob_id LIKE 'ubl_%'),
            status TEXT NOT NULL CHECK (status IN (
                'ready', 'consumed', 'expired', 'hash_conflict', 'decrypt_failed', 'cleared'
            )),
            encrypted_bytes INTEGER NOT NULL CHECK (encrypted_bytes BETWEEN 1 AND 2097152),
            created_at_ms INTEGER NOT NULL CHECK (created_at_ms > 0),
            expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms > created_at_ms),
            clear_generation INTEGER NOT NULL CHECK (clear_generation >= 0),
            record_json TEXT NOT NULL CHECK (length(record_json) BETWEEN 64 AND 16384),
            FOREIGN KEY (project_id) REFERENCES recent_projects(project_id) ON DELETE CASCADE
        );",
    )
}

fn migrate_agent_state_v10(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch("ALTER TABLE agent_sessions RENAME TO agent_sessions_v9_archive;")?;
    create_agent_state(connection)?;
    connection.execute_batch(
        "INSERT INTO agent_sessions (
            project_id, agent_session_id, provider_id, preset_version, model, skill_pack_hash,
            policy, summary_text, summary_hash, summary_version, clear_generation, updated_at_ms
         )
         SELECT project_id, agent_session_id, provider_id, preset_version, model, skill_pack_hash,
                policy, summary_text, summary_hash, summary_version, clear_generation, updated_at_ms
         FROM agent_sessions_v9_archive;
         DROP TABLE agent_sessions_v9_archive;",
    )
}
fn migrate_workflow_runs_v4(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "DROP INDEX workflow_runs_one_active;
         ALTER TABLE workflow_runs RENAME TO workflow_runs_v3_archive;",
    )?;
    create_workflow_runs(connection)?;
    connection.execute_batch(
        "INSERT INTO workflow_runs (
            run_id, project_id, workflow_id, workflow_version, pack_hash, state, sequence,
            provider_id, preset_version, model, manifest_hash, proposal_hash,
            target_relative_path, target_source_hash, confirmed_at, updated_at
         )
         SELECT
            run_id, project_id, workflow_id, workflow_version, pack_hash,
            CASE
                WHEN state IN ('completed', 'failed', 'cancelled') THEN state
                ELSE 'cancelled'
            END,
            CASE
                WHEN state IN ('completed', 'failed', 'cancelled') THEN sequence
                ELSE sequence + 1
            END,
            provider_id, preset_version, model, manifest_hash, proposal_hash,
            target_relative_path, target_source_hash, confirmed_at, updated_at
         FROM workflow_runs_v3_archive;",
    )
}

fn migrate_workflow_runs_v5(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "DROP INDEX workflow_runs_one_active;
         ALTER TABLE workflow_runs RENAME TO workflow_runs_v4_archive;",
    )?;
    create_workflow_runs(connection)?;
    connection.execute_batch(
        "INSERT INTO workflow_runs (
            run_id, project_id, workflow_id, workflow_version, pack_hash, state, sequence,
            provider_id, preset_version, model, manifest_hash, proposal_hash,
            target_relative_path, target_source_hash, confirmed_at, updated_at
         )
         SELECT
            run_id, project_id, workflow_id, workflow_version, pack_hash,
            CASE
                WHEN state IN ('completed', 'failed', 'cancelled') THEN state
                ELSE 'cancelled'
            END,
            CASE
                WHEN state IN ('completed', 'failed', 'cancelled') THEN sequence
                ELSE sequence + 1
            END,
            provider_id, preset_version, model, manifest_hash, proposal_hash,
            target_relative_path, target_source_hash, confirmed_at, updated_at
         FROM workflow_runs_v4_archive;",
    )
}

fn migrate_workflow_runs_v6(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "DROP INDEX workflow_runs_one_active;
         ALTER TABLE workflow_runs RENAME TO workflow_runs_v5_archive;",
    )?;
    create_workflow_runs(connection)?;
    connection.execute_batch(
        "INSERT INTO workflow_runs (
            run_id, project_id, workflow_id, workflow_version, pack_hash, state, sequence,
            provider_id, preset_version, model, manifest_hash, proposal_hash,
            target_relative_path, target_source_hash, confirmed_at, updated_at
         )
         SELECT
            run_id, project_id, workflow_id, workflow_version, pack_hash,
            CASE
                WHEN state IN ('completed', 'failed', 'cancelled') THEN state
                ELSE 'cancelled'
            END,
            CASE
                WHEN state IN ('completed', 'failed', 'cancelled') THEN sequence
                ELSE sequence + 1
            END,
            provider_id, preset_version, model, manifest_hash, proposal_hash,
            target_relative_path, target_source_hash, confirmed_at, updated_at
         FROM workflow_runs_v5_archive;",
    )
}
fn migrate_workflow_runs_v7(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "DROP INDEX workflow_runs_one_active;
         ALTER TABLE workflow_runs RENAME TO workflow_runs_v6_archive;",
    )?;
    create_workflow_runs(connection)?;
    connection.execute_batch(
        "INSERT INTO workflow_runs (
            run_id, project_id, workflow_id, workflow_version, pack_hash, state, sequence,
            provider_id, preset_version, model, manifest_hash, proposal_hash,
            target_relative_path, target_source_hash, confirmed_at, updated_at
         )
         SELECT
            run_id, project_id, workflow_id, workflow_version, pack_hash,
            CASE
                WHEN state IN ('completed', 'failed', 'cancelled') THEN state
                ELSE 'cancelled'
            END,
            CASE
                WHEN state IN ('completed', 'failed', 'cancelled') THEN sequence
                ELSE sequence + 1
            END,
            provider_id, preset_version, model, manifest_hash, proposal_hash,
            target_relative_path, target_source_hash, confirmed_at, updated_at
         FROM workflow_runs_v6_archive;",
    )
}

fn load_snapshot(connection: &mut Connection) -> Result<AppStateSnapshot, ()> {
    prune_unavailable_projects(connection).map_err(|_| ())?;
    let ui = connection
        .query_row(
            "SELECT left_collapsed, right_collapsed, api_card_expanded, right_panel_width
             FROM ui_preferences WHERE singleton = 1",
            [],
            |row| {
                Ok(UiPreferences {
                    left_collapsed: row.get(0)?,
                    right_collapsed: row.get(1)?,
                    api_card_expanded: row.get(2)?,
                    right_panel_width: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(|_| ())?
        .unwrap_or_default()
        .validate()
        .map_err(|_| ())?;

    let mut recent_statement = connection
        .prepare(
            "SELECT project_id, display_name FROM recent_projects
             ORDER BY last_opened_order DESC LIMIT 12",
        )
        .map_err(|_| ())?;
    let recent_projects = recent_statement
        .query_map([], |row| {
            Ok(RecentProject {
                project_id: row.get(0)?,
                display_name: row.get(1)?,
            })
        })
        .map_err(|_| ())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| ())?;
    drop(recent_statement);
    let mut file_statement = connection
        .prepare(
            "SELECT project_id, current_relative_file FROM project_files
             ORDER BY project_id",
        )
        .map_err(|_| ())?;
    let project_files = file_statement
        .query_map([], |row| {
            Ok(ProjectFilePreference {
                project_id: row.get(0)?,
                current_relative_file: row.get(1)?,
            })
        })
        .map_err(|_| ())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| ())?;
    Ok(AppStateSnapshot {
        schema_version: APP_STATE_SCHEMA_VERSION,
        ui,
        recent_projects,
        project_files,
        recovered_with_defaults: false,
    })
}

fn prune_unavailable_projects(connection: &mut Connection) -> rusqlite::Result<()> {
    let stale_ids = {
        let mut statement =
            connection.prepare("SELECT project_id, root_path FROM recent_projects")?;
        let stale_ids = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .filter_map(Result::ok)
            .filter_map(|(project_id, root_path)| {
                (!is_available_directory(&PathBuf::from(root_path))).then_some(project_id)
            })
            .collect::<Vec<_>>();
        stale_ids
    };
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    for project_id in stale_ids {
        transaction.execute(
            "DELETE FROM recent_projects WHERE project_id = ?1",
            [project_id],
        )?;
    }
    transaction.commit()
}

fn is_available_directory(path: &Path) -> bool {
    if !path.is_absolute() {
        return false;
    }
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::{
        create_provider_settings, AgentCheckpointRecord, AgentMessageIndexRecord,
        AgentSessionStateRecord, AgentTurnMetadataRecord, AppStateRepository, RouterRunMetadata,
        RouterRunStatus, UiPreferences, APP_STATE_DATABASE_NAME, APP_STATE_SCHEMA_VERSION,
        MAX_RECENT_PROJECTS,
    };
    use crate::credential_vault::ProviderId;
    use crate::project::{create_project, inspect_project_root, ProjectHandle, ProjectScanMode};
    use crate::provider_core::Usage;
    use crate::workspace::{read_project_file, save_project_file};
    use novelgainker_workflows::manager::{WorkflowRunRecord, WorkflowRunState};
    use novelgainker_workflows::{registry, CHARACTER_CREATE_WORKFLOW_ID};
    use rusqlite::{params, Connection};
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn fixture() -> (PathBuf, PathBuf, ProjectHandle) {
        let root =
            std::env::temp_dir().join(format!("novelgainker-ng106-project-{}", Uuid::new_v4()));
        let app_state =
            std::env::temp_dir().join(format!("novelgainker-ng106-state-{}", Uuid::new_v4()));
        fs::create_dir(&root).expect("project root must create");
        let inspection =
            inspect_project_root(&root, ProjectScanMode::Create).expect("project must scan");
        let handle = create_project(&inspection).expect("project must create");
        (root, app_state, handle)
    }

    fn create_v1_database(
        database_path: &PathBuf,
        handle: &ProjectHandle,
        preferences: UiPreferences,
    ) {
        fs::create_dir_all(database_path.parent().unwrap()).unwrap();
        let connection = Connection::open(database_path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE ui_preferences (
                    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                    left_collapsed INTEGER NOT NULL,
                    right_collapsed INTEGER NOT NULL,
                    api_card_expanded INTEGER NOT NULL,
                    right_panel_width INTEGER NULL
                );
                CREATE TABLE recent_projects (
                    project_id TEXT PRIMARY KEY NOT NULL,
                    root_path TEXT UNIQUE NOT NULL,
                    display_name TEXT NOT NULL,
                    last_opened_order INTEGER NOT NULL
                );
                CREATE TABLE project_files (
                    project_id TEXT PRIMARY KEY NOT NULL,
                    current_relative_file TEXT NOT NULL,
                    FOREIGN KEY (project_id) REFERENCES recent_projects(project_id) ON DELETE CASCADE
                );
                PRAGMA user_version = 1;",
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO ui_preferences VALUES (1, ?1, ?2, ?3, ?4)",
                params![
                    preferences.left_collapsed,
                    preferences.right_collapsed,
                    preferences.api_card_expanded,
                    preferences.right_panel_width
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO recent_projects VALUES (?1, ?2, ?3, 1)",
                params![
                    handle.project_id(),
                    handle.root().to_string_lossy(),
                    handle.display_name()
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO project_files VALUES (?1, 'chapters/001.md')",
                [handle.project_id()],
            )
            .unwrap();
    }

    fn create_v2_database(
        database_path: &PathBuf,
        handle: &ProjectHandle,
        preferences: UiPreferences,
    ) {
        create_v1_database(database_path, handle, preferences);
        let connection = Connection::open(database_path).unwrap();
        create_provider_settings(&connection).unwrap();
        connection.pragma_update(None, "user_version", 2).unwrap();
    }

    fn workflow_record(project_id: &str) -> WorkflowRunRecord {
        WorkflowRunRecord {
            run_id: Uuid::new_v4().to_string(),
            project_id: project_id.to_owned(),
            workflow_id: CHARACTER_CREATE_WORKFLOW_ID.to_owned(),
            workflow_version: 2,
            pack_hash: registry().pack_hash.to_owned(),
            state: WorkflowRunState::Created,
            sequence: 0,
            provider_id: Some("deepseek".to_owned()),
            preset_version: Some(1),
            model: Some("deepseek-v4-pro".to_owned()),
            manifest_hash: None,
            proposal_hash: None,
            target_relative_path: None,
            target_source_hash: None,
            confirmed_at: None,
            updated_at: 1,
        }
    }

    #[test]
    fn migrates_v2_to_v4_transactionally_without_touching_existing_state() {
        let (root, app_state, handle) = fixture();
        let preferences = UiPreferences {
            left_collapsed: true,
            right_collapsed: true,
            api_card_expanded: false,
            right_panel_width: Some(900),
        };
        let database_path = app_state.join(APP_STATE_DATABASE_NAME);
        create_v2_database(&database_path, &handle, preferences);

        let repository = AppStateRepository::open(&app_state);
        assert_eq!(
            repository.snapshot().schema_version,
            APP_STATE_SCHEMA_VERSION
        );
        assert_eq!(repository.snapshot().ui, preferences);
        assert_eq!(repository.provider_settings().unwrap().len(), 2);
        assert!(repository.workflow_runs().unwrap().is_empty());
        let version: u16 = Connection::open(repository.database_path())
            .unwrap()
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, APP_STATE_SCHEMA_VERSION);

        drop(repository);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn v2_to_v4_failure_rolls_back_and_keeps_provider_state_readable() {
        let (root, app_state, handle) = fixture();
        let database_path = app_state.join(APP_STATE_DATABASE_NAME);
        create_v2_database(&database_path, &handle, UiPreferences::default());
        let connection = Connection::open(&database_path).unwrap();
        connection
            .execute("CREATE TABLE workflow_runs (sentinel TEXT)", [])
            .unwrap();
        drop(connection);

        let degraded = AppStateRepository::open(&app_state);
        assert!(degraded.snapshot().recovered_with_defaults);
        drop(degraded);
        let connection = Connection::open(&database_path).unwrap();
        let version: u16 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        let provider_rows: u16 = connection
            .query_row("SELECT COUNT(*) FROM provider_settings", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(version, 2);
        assert_eq!(provider_rows, 2);

        drop(connection);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn migrates_v3_workflow_rows_without_deleting_history() {
        let (root, app_state, handle) = fixture();
        let repository = AppStateRepository::open(&app_state);
        let active = workflow_record(handle.project_id());
        repository.save_workflow_run(&active).unwrap();
        drop(repository);

        let database_path = app_state.join(APP_STATE_DATABASE_NAME);
        let connection = Connection::open(&database_path).unwrap();
        connection.pragma_update(None, "user_version", 3).unwrap();
        drop(connection);

        let migrated = AppStateRepository::open(&app_state);
        assert_eq!(migrated.snapshot().schema_version, APP_STATE_SCHEMA_VERSION);
        let records = migrated.workflow_runs().unwrap();
        assert_eq!(records.len(), 1);
        let retained = &records[0];
        assert_eq!(retained.run_id, active.run_id);
        assert_eq!(retained.workflow_id, active.workflow_id);
        assert_eq!(retained.pack_hash, active.pack_hash);
        assert_eq!(retained.provider_id, active.provider_id);
        assert_eq!(retained.state, WorkflowRunState::Cancelled);
        assert_eq!(retained.sequence, active.sequence + 1);

        let connection = Connection::open(migrated.database_path()).unwrap();
        let archived_rows: u16 = connection
            .query_row("SELECT COUNT(*) FROM workflow_runs_v3_archive", [], |row| {
                row.get(0)
            })
            .unwrap();
        let archived_state: String = connection
            .query_row(
                "SELECT state FROM workflow_runs_v3_archive WHERE run_id = ?1",
                [&active.run_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(archived_rows, 1);
        assert_eq!(archived_state, "created");

        drop(connection);
        drop(migrated);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn migrates_v5_workflow_rows_to_v6_without_deleting_history() {
        let (root, app_state, handle) = fixture();
        let repository = AppStateRepository::open(&app_state);
        let active = workflow_record(handle.project_id());
        repository.save_workflow_run(&active).unwrap();
        drop(repository);

        let database_path = app_state.join(APP_STATE_DATABASE_NAME);
        let connection = Connection::open(&database_path).unwrap();
        connection.pragma_update(None, "user_version", 5).unwrap();
        drop(connection);

        let migrated = AppStateRepository::open(&app_state);
        let retained = migrated.workflow_runs().unwrap().remove(0);
        assert_eq!(retained.run_id, active.run_id);
        assert_eq!(retained.state, WorkflowRunState::Cancelled);
        assert_eq!(retained.sequence, active.sequence + 1);
        let connection = Connection::open(migrated.database_path()).unwrap();
        let archived_state: String = connection
            .query_row(
                "SELECT state FROM workflow_runs_v5_archive WHERE run_id = ?1",
                [&active.run_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(archived_state, "created");

        drop(connection);
        drop(migrated);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn workflow_checkpoints_preserve_snapshots_reject_mutation_and_bound_terminal_history() {
        let (root, app_state, handle) = fixture();
        let repository = AppStateRepository::open(&app_state);
        let initial = workflow_record(handle.project_id());
        repository.save_workflow_run(&initial).unwrap();
        let mut routed = initial.clone();
        routed.state = WorkflowRunState::Routed;
        routed.sequence = 1;
        routed.updated_at = 2;
        repository.save_workflow_run(&routed).unwrap();

        let mut mutated_snapshot = routed.clone();
        mutated_snapshot.provider_id = Some("openai".to_owned());
        mutated_snapshot.model = Some("gpt-5.6-terra".to_owned());
        mutated_snapshot.sequence = 2;
        assert!(repository.save_workflow_run(&mutated_snapshot).is_err());
        assert_eq!(repository.workflow_runs().unwrap(), vec![routed.clone()]);
        assert_eq!(repository.provider_settings().unwrap().len(), 2);

        for index in 0..=20 {
            let mut terminal = workflow_record(handle.project_id());
            terminal.state = WorkflowRunState::Completed;
            terminal.sequence = 9;
            terminal.updated_at = index + 10;
            repository.save_workflow_run(&terminal).unwrap();
        }
        let records = repository.workflow_runs().unwrap();
        assert_eq!(
            records
                .iter()
                .filter(|record| record.state.is_terminal())
                .count(),
            20
        );
        assert!(records.contains(&routed));

        drop(repository);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn migrates_v6_history_to_v7_and_preserves_character_v1_and_v2_metadata() {
        let (root, app_state, handle) = fixture();
        let repository = AppStateRepository::open(&app_state);
        let current = workflow_record(handle.project_id());
        repository.save_workflow_run(&current).unwrap();
        drop(repository);

        let mut legacy = workflow_record(handle.project_id());
        legacy.workflow_version = 1;
        legacy.pack_hash = "b".repeat(64);
        legacy.state = WorkflowRunState::Completed;
        legacy.sequence = 8;
        legacy.updated_at = 2;
        let database_path = app_state.join(APP_STATE_DATABASE_NAME);
        let connection = Connection::open(&database_path).unwrap();
        connection
            .execute(
                "INSERT INTO workflow_runs (
                    run_id, project_id, workflow_id, workflow_version, pack_hash, state, sequence,
                    provider_id, preset_version, model, manifest_hash, proposal_hash,
                    target_relative_path, target_source_hash, confirmed_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                params![
                    legacy.run_id,
                    legacy.project_id,
                    legacy.workflow_id,
                    legacy.workflow_version,
                    legacy.pack_hash,
                    legacy.state.as_str(),
                    legacy.sequence,
                    legacy.provider_id,
                    legacy.preset_version,
                    legacy.model,
                    legacy.manifest_hash,
                    legacy.proposal_hash,
                    legacy.target_relative_path,
                    legacy.target_source_hash,
                    legacy.confirmed_at,
                    legacy.updated_at,
                ],
            )
            .unwrap();
        connection.pragma_update(None, "user_version", 6).unwrap();
        drop(connection);

        let migrated = AppStateRepository::open(&app_state);
        let records = migrated.workflow_runs().unwrap();
        assert_eq!(records.len(), 2);
        let retained_legacy = records
            .iter()
            .find(|record| record.run_id == legacy.run_id)
            .expect("v1 history retained");
        assert_eq!(retained_legacy.workflow_version, 1);
        assert_eq!(retained_legacy.state, WorkflowRunState::Completed);
        let retained_current = records
            .iter()
            .find(|record| record.run_id == current.run_id)
            .expect("v2 checkpoint retained");
        assert_eq!(retained_current.workflow_version, 2);
        assert_eq!(retained_current.state, WorkflowRunState::Cancelled);

        let connection = Connection::open(migrated.database_path()).unwrap();
        let archived_rows: u16 = connection
            .query_row("SELECT COUNT(*) FROM workflow_runs_v6_archive", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(archived_rows, 2);

        drop(connection);
        drop(migrated);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn schema_v7_accepts_plot_checkpoint_metadata_without_bodies() {
        let (root, app_state, handle) = fixture();
        let repository = AppStateRepository::open(&app_state);
        let mut plot = workflow_record(handle.project_id());
        plot.workflow_id = "novelgainker.plot-create".to_owned();
        plot.workflow_version = 1;
        plot.target_relative_path = Some("plot/main.md".to_owned());
        plot.target_source_hash = Some("a".repeat(64));
        repository.save_workflow_run(&plot).unwrap();
        assert_eq!(repository.workflow_runs().unwrap(), vec![plot]);

        let database = fs::read(repository.database_path()).unwrap();
        let database = String::from_utf8_lossy(&database);
        for forbidden in [
            "PRIVATE_PLOT_BRIEF",
            "PRIVATE_PLOT_CONTEXT",
            "PRIVATE_PROVIDER_RESPONSE",
            "PRIVATE_PLOT_PROPOSAL",
            "PRIVATE_PLOT_REVEAL",
            "PRIVATE_PLOT_PAYOFF",
        ] {
            assert!(!database.contains(forbidden));
        }

        drop(repository);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn migrates_v1_to_v4_transactionally_and_preserves_all_sprint_one_data() {
        let (root, app_state, handle) = fixture();
        let preferences = UiPreferences {
            left_collapsed: true,
            right_collapsed: false,
            api_card_expanded: false,
            right_panel_width: Some(1_200),
        };
        create_v1_database(
            &app_state.join(APP_STATE_DATABASE_NAME),
            &handle,
            preferences,
        );

        let repository = AppStateRepository::open(&app_state);
        let snapshot = repository.snapshot();
        assert_eq!(snapshot.schema_version, APP_STATE_SCHEMA_VERSION);
        assert_eq!(snapshot.ui, preferences);
        assert_eq!(snapshot.recent_projects.len(), 1);
        assert_eq!(snapshot.project_files.len(), 1);
        let providers = repository.provider_settings().unwrap();
        assert_eq!(providers.len(), 2);
        assert!(providers.iter().all(|record| {
            record.preset_version == 1
                && record.credential_ref.is_none()
                && record.checked_at.is_none()
        }));
        assert!(
            providers
                .iter()
                .find(|record| record.provider_id == ProviderId::Openai)
                .unwrap()
                .is_selected
        );
        drop(repository);

        let reopened = AppStateRepository::open(&app_state);
        assert_eq!(reopened.provider_settings().unwrap(), providers);
        let connection = Connection::open(reopened.database_path()).unwrap();
        let version: u16 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, APP_STATE_SCHEMA_VERSION);

        drop(connection);
        drop(reopened);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn v1_to_v4_migration_failure_rolls_back_without_touching_sprint_one_data() {
        let (root, app_state, handle) = fixture();
        let database_path = app_state.join(APP_STATE_DATABASE_NAME);
        create_v1_database(&database_path, &handle, UiPreferences::default());
        let connection = Connection::open(&database_path).unwrap();
        connection
            .execute("CREATE TABLE provider_settings (sentinel TEXT)", [])
            .unwrap();
        drop(connection);

        let degraded = AppStateRepository::open(&app_state);
        assert!(degraded.snapshot().recovered_with_defaults);
        drop(degraded);

        let connection = Connection::open(&database_path).unwrap();
        let version: u16 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        let ui_rows: u16 = connection
            .query_row("SELECT COUNT(*) FROM ui_preferences", [], |row| row.get(0))
            .unwrap();
        let sentinel_columns: u16 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('provider_settings')
                 WHERE name = 'sentinel'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, 1);
        assert_eq!(ui_rows, 1);
        assert_eq!(sentinel_columns, 1);

        drop(connection);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn persists_versioned_ui_recent_project_and_relative_file_across_restart() {
        let (root, app_state, handle) = fixture();
        let preferences = UiPreferences {
            left_collapsed: true,
            right_collapsed: true,
            api_card_expanded: false,
            right_panel_width: Some(920),
        };
        {
            let repository = AppStateRepository::open(&app_state);
            repository
                .save_ui(preferences)
                .expect("preferences must save");
            repository
                .record_project(&handle)
                .expect("recent project must save");
            repository
                .record_current_file(handle.project_id(), "chapters/001.md")
                .expect("relative file must save");
        }
        let reopened = AppStateRepository::open(&app_state);
        let snapshot = reopened.snapshot();
        assert_eq!(snapshot.schema_version, APP_STATE_SCHEMA_VERSION);
        assert_eq!(snapshot.ui, preferences);
        assert_eq!(snapshot.recent_projects.len(), 1);
        assert_eq!(snapshot.recent_projects[0].project_id, handle.project_id());
        assert_eq!(
            snapshot.project_files[0].current_relative_file,
            "chapters/001.md"
        );
        assert!(!snapshot.recovered_with_defaults);
        assert_eq!(
            reopened.recent_project_root(handle.project_id()),
            Some(handle.root().to_path_buf())
        );

        drop(reopened);
        fs::remove_dir_all(app_state).expect("state fixture must remove");
        fs::remove_dir_all(root).expect("project fixture must remove");
    }

    #[test]
    fn corrupted_or_newer_schema_falls_back_without_blocking_manual_project_access() {
        let (root, app_state, handle) = fixture();
        fs::create_dir_all(&app_state).expect("state root must create");
        fs::write(app_state.join(APP_STATE_DATABASE_NAME), b"not a database")
            .expect("corrupt fixture must write");
        let corrupted = AppStateRepository::open(&app_state);
        assert!(corrupted.snapshot().recovered_with_defaults);
        assert_eq!(
            corrupted
                .save_ui(UiPreferences::default())
                .unwrap_err()
                .code,
            "app_state.unavailable"
        );
        assert!(read_project_file(&handle, "project.md").is_ok());
        drop(corrupted);

        fs::remove_file(app_state.join(APP_STATE_DATABASE_NAME))
            .expect("corrupt fixture must remove");
        let connection = Connection::open(app_state.join(APP_STATE_DATABASE_NAME)).unwrap();
        connection.pragma_update(None, "user_version", 99).unwrap();
        drop(connection);
        let newer = AppStateRepository::open(&app_state);
        assert!(newer.snapshot().recovered_with_defaults);
        assert!(read_project_file(&handle, "project.md").is_ok());

        drop(newer);
        fs::remove_dir_all(app_state).expect("state fixture must remove");
        fs::remove_dir_all(root).expect("project fixture must remove");
    }

    #[test]
    fn deleting_state_preserves_canon_and_recreates_clean_defaults() {
        let (root, app_state, handle) = fixture();
        let repository = AppStateRepository::open(&app_state);
        repository
            .record_project(&handle)
            .expect("recent project must save");
        let document = read_project_file(&handle, "project.md").unwrap();
        let saved = save_project_file(
            &handle,
            "project.md",
            "---
title: 离线项目
writingLanguage: zh-CN
---

正文
",
            &document.content_hash,
        )
        .expect("canon must save independently");
        drop(repository);
        fs::remove_dir_all(&app_state).expect("state root must be independently deletable");
        let recreated = AppStateRepository::open(&app_state);
        assert_eq!(
            recreated.snapshot(),
            super::AppStateSnapshot::defaults(false)
        );
        assert_eq!(
            read_project_file(&handle, "project.md")
                .unwrap()
                .content_hash,
            saved.content_hash
        );

        drop(recreated);
        fs::remove_dir_all(app_state).expect("state fixture must remove");
        fs::remove_dir_all(root).expect("project fixture must remove");
    }

    #[test]
    fn migrates_v7_to_v9_without_touching_v1_or_v2_workflow_history() {
        let (root, app_state, handle) = fixture();
        let repository = AppStateRepository::open(&app_state);
        let current = workflow_record(handle.project_id());
        repository.save_workflow_run(&current).unwrap();
        drop(repository);

        let database_path = app_state.join(APP_STATE_DATABASE_NAME);
        let connection = Connection::open(&database_path).unwrap();
        connection.execute("DROP TABLE router_runs", []).unwrap();
        connection.pragma_update(None, "user_version", 7).unwrap();
        drop(connection);

        let migrated = AppStateRepository::open(&app_state);
        assert_eq!(migrated.snapshot().schema_version, APP_STATE_SCHEMA_VERSION);
        assert!(migrated.router_runs().unwrap().is_empty());
        let records = migrated.workflow_runs().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].workflow_version, 2);
        assert_eq!(records[0].run_id, current.run_id);

        drop(migrated);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn router_terminal_metadata_is_strict_bounded_and_contains_no_original_text() {
        let (root, app_state, _handle) = fixture();
        let repository = AppStateRepository::open(&app_state);
        for index in 0..=20 {
            let completed = RouterRunMetadata {
                run_id: Uuid::new_v4().to_string(),
                provider_id: if index % 2 == 0 {
                    ProviderId::Deepseek
                } else {
                    ProviderId::Openai
                },
                workflow_id: Some("novelgainker.material-create".to_owned()),
                confidence: Some(if index % 3 == 0 { "medium" } else { "high" }.to_owned()),
                status: RouterRunStatus::Completed,
                usage: Some(Usage {
                    input_tokens: index + 10,
                    output_tokens: 2,
                    total_tokens: index + 12,
                }),
                updated_at: i64::try_from(index).unwrap(),
            };
            repository.save_router_run(&completed).unwrap();
        }
        let records = repository.router_runs().unwrap();
        assert_eq!(records.len(), 20);
        assert!(records
            .iter()
            .all(|record| record.status == RouterRunStatus::Completed));

        let invalid = RouterRunMetadata {
            run_id: Uuid::new_v4().to_string(),
            provider_id: ProviderId::Deepseek,
            workflow_id: Some("novelgainker.chapter-create".to_owned()),
            confidence: Some("high".to_owned()),
            status: RouterRunStatus::Completed,
            usage: None,
            updated_at: 99,
        };
        assert!(repository.save_router_run(&invalid).is_err());

        let database =
            String::from_utf8_lossy(&fs::read(repository.database_path()).unwrap()).into_owned();
        for forbidden in [
            "ROUTER_PRIVATE_CANARY_7D8F",
            "original_text",
            "provider_response",
            "prompt",
            "project_session_id",
        ] {
            assert!(!database.contains(forbidden));
        }

        drop(repository);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn schema_v10_contains_only_bounded_agent_indexes_and_never_secrets_or_canon() {
        let (root, app_state, handle) = fixture();
        let repository = AppStateRepository::open(&app_state);
        repository
            .record_project(&handle)
            .expect("recent project must save");
        for index in 0..(MAX_RECENT_PROJECTS + 2) {
            let other_root = root.join(format!("recent-{index}"));
            fs::create_dir(&other_root).expect("recent root must create");
            let inspection = inspect_project_root(&other_root, ProjectScanMode::Create).unwrap();
            let other = create_project(&inspection).unwrap();
            repository.record_project(&other).unwrap();
        }
        assert_eq!(
            repository.snapshot().recent_projects.len(),
            MAX_RECENT_PROJECTS
        );
        let database_path = repository.database_path().to_path_buf();
        drop(repository);
        let connection = Connection::open(database_path).unwrap();
        let schema: String = connection
            .query_row(
                "SELECT group_concat(sql, ' ') FROM sqlite_master WHERE type = 'table'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let tables = {
            let mut statement = connection
                .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
                .unwrap();
            statement
                .query_map([], |row| row.get::<_, String>(0))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap()
        };
        assert_eq!(
            tables,
            [
                "agent_msg_index",
                "agent_sessions",
                "agent_turns",
                "agent_undo_records",
                "project_files",
                "provider_settings",
                "recent_projects",
                "router_runs",
                "ui_preferences",
                "workflow_runs"
            ]
        );
        let normalized = schema.to_lowercase();
        for forbidden in [
            "api_key",
            "secret",
            "prompt",
            "query",
            "api_token",
            "conversation",
            "authorization",
            "provider_response",
            "raw_response",
            "message_body",
            "full_chat",
            "credential_blob",
        ] {
            assert!(
                !normalized.contains(forbidden),
                "forbidden state field: {forbidden}"
            );
        }

        assert!(normalized.contains("router_runs"));
        let router_columns = {
            let mut statement = connection
                .prepare("SELECT name FROM pragma_table_info('router_runs') ORDER BY cid")
                .unwrap();
            statement
                .query_map([], |row| row.get::<_, String>(0))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap()
        };
        assert_eq!(
            router_columns,
            [
                "run_id",
                "provider_id",
                "workflow_id",
                "confidence",
                "status",
                "input_tokens",
                "output_tokens",
                "total_tokens",
                "updated_at"
            ]
        );
        for forbidden in [
            "original", "text", "prompt", "response", "project", "session", "key",
        ] {
            assert!(!router_columns
                .iter()
                .any(|column| column.contains(forbidden)));
        }
        assert!(normalized.contains("workflow_runs"));
        assert!(normalized.contains("novelgainker.material-create"));
        let workflow_columns = {
            let mut statement = connection
                .prepare("SELECT name FROM pragma_table_info('workflow_runs') ORDER BY cid")
                .unwrap();
            statement
                .query_map([], |row| row.get::<_, String>(0))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap()
        };
        for forbidden in [
            "brief",
            "prompt",
            "message",
            "context_body",
            "provider_response",
            "proposal_markdown",
            "diff",
            "api_key",
            "credential_blob",
        ] {
            assert!(
                !workflow_columns.iter().any(|column| column == forbidden),
                "forbidden workflow checkpoint column: {forbidden}"
            );
        }

        assert!(normalized.contains("provider_settings"));
        assert!(normalized.contains("credential_ref"));
        let agent_session_columns = table_columns(&connection, "agent_sessions");
        assert_eq!(
            agent_session_columns,
            [
                "project_id",
                "agent_session_id",
                "provider_id",
                "preset_version",
                "model",
                "skill_pack_hash",
                "policy",
                "summary_text",
                "summary_hash",
                "summary_version",
                "clear_generation",
                "updated_at_ms"
            ]
        );
        assert_eq!(
            table_columns(&connection, "agent_msg_index"),
            [
                "message_id",
                "project_id",
                "agent_session_id",
                "turn_id",
                "ordinal",
                "role",
                "body_hash",
                "utf8_bytes",
                "created_at_ms"
            ]
        );
        assert_eq!(
            table_columns(&connection, "agent_undo_records"),
            [
                "transaction_hash",
                "project_id",
                "turn_id",
                "blob_id",
                "status",
                "encrypted_bytes",
                "created_at_ms",
                "expires_at_ms",
                "clear_generation",
                "record_json"
            ]
        );
        drop(connection);
        fs::remove_dir_all(app_state).expect("state fixture must remove");
        fs::remove_dir_all(root).expect("project fixture must remove");
    }

    #[test]
    fn migrates_v8_to_v9_and_preserves_prior_checkpoint_history() {
        let (root, app_state, handle) = fixture();
        let repository = AppStateRepository::open(&app_state);
        repository.record_project(&handle).unwrap();
        let workflow = workflow_record(handle.project_id());
        repository.save_workflow_run(&workflow).unwrap();
        drop(repository);

        let database_path = app_state.join(APP_STATE_DATABASE_NAME);
        let connection = Connection::open(&database_path).unwrap();
        connection
            .execute_batch(
                "DROP TABLE agent_msg_index; DROP TABLE agent_turns; DROP TABLE agent_sessions;",
            )
            .unwrap();
        connection.pragma_update(None, "user_version", 8).unwrap();
        drop(connection);

        let migrated = AppStateRepository::open(&app_state);
        assert_eq!(migrated.snapshot().schema_version, APP_STATE_SCHEMA_VERSION);
        assert_eq!(migrated.workflow_runs().unwrap()[0].run_id, workflow.run_id);
        assert!(migrated
            .agent_session(handle.project_id())
            .unwrap()
            .is_none());
        drop(migrated);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn migrates_v9_to_v10_preserves_session_and_enables_review_policy() {
        let (root, app_state, handle) = fixture();
        let repository = AppStateRepository::open(&app_state);
        repository.record_project(&handle).unwrap();
        let session_id = format!("ses_{}", Uuid::new_v4().simple());
        repository
            .save_agent_session(&AgentSessionStateRecord {
                project_id: handle.project_id().to_owned(),
                agent_session_id: session_id.clone(),
                provider_id: ProviderId::Openai,
                preset_version: 1,
                model: "gpt-5.6-terra".to_owned(),
                skill_pack_hash: novelgainker_agent::SKILL_PACK_SHA256.to_owned(),
                policy: "auto_apply_safe".to_owned(),
                summary_text: None,
                summary_hash: None,
                summary_version: 0,
                clear_generation: 0,
                updated_at_ms: 1_800_000_000_000,
            })
            .unwrap();
        drop(repository);

        let database_path = app_state.join(APP_STATE_DATABASE_NAME);
        let connection = Connection::open(&database_path).unwrap();
        connection
            .execute_batch("DROP TABLE agent_undo_records;")
            .unwrap();
        connection.pragma_update(None, "user_version", 9).unwrap();
        drop(connection);

        let migrated = AppStateRepository::open(&app_state);
        assert_eq!(migrated.snapshot().schema_version, APP_STATE_SCHEMA_VERSION);
        assert!(migrated
            .agent_undo_records(handle.project_id())
            .unwrap()
            .is_empty());
        let mut session = migrated
            .agent_session(handle.project_id())
            .unwrap()
            .expect("v9 Agent session must survive migration");
        assert_eq!(session.agent_session_id, session_id);
        assert_eq!(session.policy, "auto_apply_safe");
        session.policy = "review_every_write".to_owned();
        session.updated_at_ms += 1;
        migrated.save_agent_session(&session).unwrap();
        assert_eq!(
            migrated
                .agent_session(handle.project_id())
                .unwrap()
                .unwrap()
                .policy,
            "review_every_write"
        );
        drop(migrated);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn agent_checkpoint_reopens_without_chat_bodies_and_clear_never_writes_canon() {
        let (root, app_state, handle) = fixture();
        let repository = AppStateRepository::open(&app_state);
        repository.record_project(&handle).unwrap();
        let session_id = format!("ses_{}", Uuid::new_v4().simple());
        let turn_id = format!("trn_{}", Uuid::new_v4().simple());
        let summary = "人物与世界规则需要继续核对。".to_owned();
        let now = 1_800_000_000_000u64;
        let session = AgentSessionStateRecord {
            project_id: handle.project_id().to_owned(),
            agent_session_id: session_id.clone(),
            provider_id: ProviderId::Openai,
            preset_version: 1,
            model: "gpt-5.6-terra".to_owned(),
            skill_pack_hash: novelgainker_agent::SKILL_PACK_SHA256.to_owned(),
            policy: "auto_apply_safe".to_owned(),
            summary_hash: Some(
                novelgainker_agent::contracts::ContentHashV1::digest(summary.as_bytes())
                    .as_str()
                    .to_owned(),
            ),
            summary_text: Some(summary.clone()),
            summary_version: 1,
            clear_generation: 0,
            updated_at_ms: now,
        };
        let private_body = "AM1_PRIVATE_CHAT_CANARY_2B17";
        repository
            .save_agent_checkpoint(&AgentCheckpointRecord {
                session,
                turn: AgentTurnMetadataRecord {
                    project_id: handle.project_id().to_owned(),
                    agent_session_id: session_id.clone(),
                    turn_id: turn_id.clone(),
                    terminal_state: "completed".to_owned(),
                    skill_ids: vec!["create-character".to_owned()],
                    tool_ids: vec!["project.read_markdown".to_owned()],
                    action_count: 2,
                    tool_call_count: 1,
                    action_plan_hash: novelgainker_agent::contracts::ContentHashV1::digest(b"plan")
                        .as_str()
                        .to_owned(),
                    updated_at_ms: now,
                },
                messages: vec![AgentMessageIndexRecord {
                    project_id: handle.project_id().to_owned(),
                    agent_session_id: session_id,
                    message_id: format!("msg_{}", Uuid::new_v4().simple()),
                    turn_id,
                    ordinal: 1,
                    role: "user".to_owned(),
                    body_hash: novelgainker_agent::contracts::ContentHashV1::digest(
                        private_body.as_bytes(),
                    )
                    .as_str()
                    .to_owned(),
                    utf8_bytes: private_body.len() as u32,
                    created_at_ms: now,
                }],
            })
            .unwrap();
        let canon_before = read_project_file(&handle, "project.md")
            .unwrap()
            .content_hash;
        drop(repository);

        let reopened = AppStateRepository::open(&app_state);
        assert_eq!(
            reopened
                .agent_session(handle.project_id())
                .unwrap()
                .unwrap()
                .summary_text,
            Some(summary)
        );
        assert_eq!(reopened.agent_turns(handle.project_id()).unwrap().len(), 1);
        assert_eq!(
            reopened
                .agent_message_indexes(handle.project_id())
                .unwrap()
                .len(),
            1
        );
        let database = fs::read(reopened.database_path()).unwrap();
        assert!(!String::from_utf8_lossy(&database).contains(private_body));
        let cleared = reopened.clear_agent_state(handle.project_id()).unwrap();
        assert_eq!(cleared.canon_write_count, 0);
        assert_eq!(cleared.sessions_removed, 1);
        assert_eq!(
            read_project_file(&handle, "project.md")
                .unwrap()
                .content_hash,
            canon_before
        );
        drop(reopened);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    fn table_columns(connection: &Connection, table: &str) -> Vec<String> {
        let sql = format!("SELECT name FROM pragma_table_info('{table}') ORDER BY cid");
        let mut statement = connection.prepare(&sql).unwrap();
        statement
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    }
}
