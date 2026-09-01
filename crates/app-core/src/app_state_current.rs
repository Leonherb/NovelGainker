use crate::credential_vault::ProviderId;
use crate::project::ProjectHandle;
use crate::AppError;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const APP_STATE_SCHEMA_VERSION: u16 = 10;
pub const APP_STATE_DATABASE_NAME: &str = "app-state-v1.sqlite3";
const MAX_RECENT_PROJECTS: usize = 12;
const MIN_RIGHT_PANEL_WIDTH: u16 = 278;
const MAX_RIGHT_PANEL_WIDTH: u16 = 1_600;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSettingRecord {
    pub provider_id: ProviderId,
    pub preset_version: u16,
    pub credential_ref: Option<String>,
    pub is_selected: bool,
    pub connection_status: String,
    pub checked_at: Option<i64>,
}

enum Backend {
    Ready(Connection),
    Degraded,
}

pub struct AppStateRepository {
    database_path: PathBuf,
    backend: Mutex<Backend>,
}

impl AppStateRepository {
    pub fn open(app_state_root: &Path) -> Self {
        let database_path = app_state_root.join(APP_STATE_DATABASE_NAME);
        let backend = fs::create_dir_all(app_state_root)
            .ok()
            .and_then(|()| Connection::open(&database_path).ok())
            .and_then(|connection| initialize(connection).ok())
            .map_or(Backend::Degraded, Backend::Ready);
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
        let Backend::Ready(connection) = &mut *backend else {
            return AppStateSnapshot::defaults(true);
        };
        load_snapshot(connection).unwrap_or_else(|_| AppStateSnapshot::defaults(true))
    }

    pub fn save_ui(&self, preferences: UiPreferences) -> Result<(), AppError> {
        let preferences = preferences.validate()?;
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO ui_preferences
                 (singleton, left_collapsed, right_collapsed, api_card_expanded, right_panel_width)
                 VALUES (1, ?1, ?2, ?3, ?4)
                 ON CONFLICT(singleton) DO UPDATE SET
                    left_collapsed = excluded.left_collapsed,
                    right_collapsed = excluded.right_collapsed,
                    api_card_expanded = excluded.api_card_expanded,
                    right_panel_width = excluded.right_panel_width",
                params![
                    preferences.left_collapsed,
                    preferences.right_collapsed,
                    preferences.api_card_expanded,
                    preferences.right_panel_width
                ],
            )?;
            Ok(())
        })
    }

    pub fn record_project(&self, handle: &ProjectHandle) -> Result<(), AppError> {
        self.with_connection(|connection| {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let root_path = handle.root().to_string_lossy().into_owned();
            transaction.execute(
                "DELETE FROM project_files WHERE project_id IN (
                    SELECT project_id FROM recent_projects
                    WHERE root_path = ?1 AND project_id <> ?2
                 )",
                params![&root_path, handle.project_id()],
            )?;
            transaction.execute(
                "DELETE FROM recent_projects WHERE root_path = ?1 AND project_id <> ?2",
                params![&root_path, handle.project_id()],
            )?;
            let next_order: i64 = transaction.query_row(
                "SELECT COALESCE(MAX(last_opened_order), 0) + 1 FROM recent_projects",
                [],
                |row| row.get(0),
            )?;
            transaction.execute(
                "INSERT INTO recent_projects (project_id, root_path, display_name, last_opened_order)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(project_id) DO UPDATE SET root_path = excluded.root_path,
                    display_name = excluded.display_name,
                    last_opened_order = excluded.last_opened_order",
                params![
                    handle.project_id(),
                    &root_path,
                    handle.display_name(),
                    next_order
                ],
            )?;
            transaction.execute(
                "DELETE FROM recent_projects WHERE project_id IN (
                    SELECT project_id FROM recent_projects ORDER BY last_opened_order DESC
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
                "INSERT INTO project_files (project_id, current_relative_file) VALUES (?1, ?2)
                 ON CONFLICT(project_id) DO UPDATE SET
                    current_relative_file = excluded.current_relative_file",
                params![project_id, relative_path],
            )?;
            Ok(())
        })
    }

    pub fn current_file(&self, project_id: &str) -> Option<String> {
        self.query_string(
            "SELECT current_relative_file FROM project_files WHERE project_id = ?1",
            project_id,
        )
    }

    pub fn recent_project_root(&self, project_id: &str) -> Option<PathBuf> {
        self.query_string(
            "SELECT root_path FROM recent_projects WHERE project_id = ?1",
            project_id,
        )
        .map(PathBuf::from)
        .filter(|path| is_safe_directory(path))
    }

    pub fn forget_project(&self, project_id: &str) {
        let _ = self.with_connection(|connection| {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            transaction.execute(
                "DELETE FROM project_files WHERE project_id = ?1",
                [project_id],
            )?;
            transaction.execute(
                "DELETE FROM recent_projects WHERE project_id = ?1",
                [project_id],
            )?;
            transaction.commit()?;
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
                    let raw = row.get::<_, String>(0)?;
                    let provider_id = ProviderId::try_from(raw.as_str())
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
            require_one(connection.execute(
                "UPDATE provider_settings SET credential_ref = ?2,
                    connection_status = 'untested', checked_at = NULL WHERE provider_id = ?1",
                params![provider.as_str(), credential_ref],
            )?)
        })
    }

    pub fn clear_provider_credential_ref(&self, provider: ProviderId) -> Result<(), AppError> {
        self.with_connection(|connection| {
            require_one(connection.execute(
                "UPDATE provider_settings SET credential_ref = NULL,
                    connection_status = 'unconfigured', checked_at = NULL WHERE provider_id = ?1",
                [provider.as_str()],
            )?)
        })
    }

    pub fn update_provider_connection_status(
        &self,
        provider: ProviderId,
        status: &str,
        checked_at: i64,
    ) -> Result<(), AppError> {
        if !matches!(
            status,
            "connected" | "authentication_failed" | "unreachable"
        ) || checked_at < 0
        {
            return Err(AppError::app_state_unavailable());
        }
        self.with_connection(|connection| {
            require_one(connection.execute(
                "UPDATE provider_settings SET connection_status = ?2, checked_at = ?3
                 WHERE provider_id = ?1 AND credential_ref IS NOT NULL",
                params![provider.as_str(), status, checked_at],
            )?)
        })
    }

    pub fn select_provider(&self, provider: ProviderId) -> Result<(), AppError> {
        self.with_connection(|connection| {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            transaction.execute("UPDATE provider_settings SET is_selected = 0", [])?;
            require_one(transaction.execute(
                "UPDATE provider_settings SET is_selected = 1 WHERE provider_id = ?1",
                [provider.as_str()],
            )?)?;
            transaction.commit()?;
            Ok(())
        })
    }

    fn query_string(&self, query: &str, value: &str) -> Option<String> {
        let mut backend = self.backend.lock().ok()?;
        let Backend::Ready(connection) = &mut *backend else {
            return None;
        };
        connection
            .query_row(query, [value], |row| row.get(0))
            .optional()
            .ok()
            .flatten()
    }

    fn with_connection<T>(
        &self,
        operation: impl FnOnce(&mut Connection) -> rusqlite::Result<T>,
    ) -> Result<T, AppError> {
        let mut backend = self
            .backend
            .lock()
            .map_err(|_| AppError::app_state_unavailable())?;
        let Backend::Ready(connection) = &mut *backend else {
            return Err(AppError::app_state_unavailable());
        };
        operation(connection).map_err(|_| AppError::app_state_unavailable())
    }
}

fn require_one(changed: usize) -> rusqlite::Result<()> {
    if changed == 1 {
        Ok(())
    } else {
        Err(rusqlite::Error::InvalidQuery)
    }
}

fn initialize(connection: Connection) -> rusqlite::Result<Connection> {
    connection.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         CREATE TABLE IF NOT EXISTS ui_preferences (
            singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
            left_collapsed INTEGER NOT NULL, right_collapsed INTEGER NOT NULL,
            api_card_expanded INTEGER NOT NULL, right_panel_width INTEGER
         );
         CREATE TABLE IF NOT EXISTS recent_projects (
            project_id TEXT PRIMARY KEY, root_path TEXT NOT NULL,
            display_name TEXT NOT NULL, last_opened_order INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS project_files (
            project_id TEXT PRIMARY KEY, current_relative_file TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS provider_settings (
            provider_id TEXT PRIMARY KEY, preset_version INTEGER NOT NULL,
            credential_ref TEXT, is_selected INTEGER NOT NULL,
            connection_status TEXT NOT NULL, checked_at INTEGER
         );",
    )?;
    connection.execute(
        "INSERT OR IGNORE INTO ui_preferences VALUES (1, 0, 0, 1, NULL)",
        [],
    )?;
    for provider in ProviderId::ALL {
        connection.execute(
            "INSERT OR IGNORE INTO provider_settings VALUES (?1, 1, NULL, ?2, 'unconfigured', NULL)",
            params![provider.as_str(), provider == ProviderId::Openai],
        )?;
    }
    Ok(connection)
}

fn load_snapshot(connection: &Connection) -> rusqlite::Result<AppStateSnapshot> {
    let ui = connection.query_row(
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
    )?;
    let recent_projects = {
        let mut statement = connection.prepare(
            "SELECT project_id, display_name FROM recent_projects
             ORDER BY last_opened_order DESC LIMIT ?1",
        )?;
        let rows = statement
            .query_map([MAX_RECENT_PROJECTS as i64], |row| {
                Ok(RecentProject {
                    project_id: row.get(0)?,
                    display_name: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    };
    let project_files = {
        let mut statement = connection.prepare(
            "SELECT project_id, current_relative_file FROM project_files ORDER BY project_id",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok(ProjectFilePreference {
                    project_id: row.get(0)?,
                    current_relative_file: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    };
    Ok(AppStateSnapshot {
        schema_version: APP_STATE_SCHEMA_VERSION,
        ui,
        recent_projects,
        project_files,
        recovered_with_defaults: false,
    })
}

fn is_safe_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
        .unwrap_or(false)
}
