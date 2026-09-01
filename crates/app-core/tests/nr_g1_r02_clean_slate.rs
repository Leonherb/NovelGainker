use app_core::app_state::{AppStateRepository, UiPreferences};
use app_core::credential_vault::ProviderId;
use app_core::project::{create_project, inspect_project_root, open_project, ProjectScanMode};
use rusqlite::{params, Connection};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

struct TestRoot(PathBuf);

impl TestRoot {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("novelgainker-nr-g1-r02-state-{}", Uuid::new_v4()));
        fs::create_dir(&path).expect("test root must create");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let temp = std::env::temp_dir();
        let safe = self.0.parent() == Some(temp.as_path())
            && self
                .0
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("novelgainker-nr-g1-r02-state-"));
        if safe {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

#[test]
fn clean_slate_removes_only_same_root_old_project_rows() {
    let fixture = TestRoot::new();
    let project_root = fixture.path().join("legacy-project");
    let unrelated_root = fixture.path().join("unrelated-project");
    let state_root = fixture.path().join("app-state");
    fs::create_dir(&project_root).unwrap();
    fs::create_dir(&unrelated_root).unwrap();
    fs::create_dir(project_root.join(".novelgainker")).unwrap();
    fs::write(project_root.join("project.md"), b"# Canon\n").unwrap();
    let old_project_id = "40000000-0000-4000-8000-000000000004";
    fs::write(
        project_root.join(".novelgainker/project.json"),
        format!("{{\"schemaVersion\":1,\"projectId\":\"{old_project_id}\"}}"),
    )
    .unwrap();

    let unrelated =
        create_project(&inspect_project_root(&unrelated_root, ProjectScanMode::Create).unwrap())
            .unwrap();
    let unrelated_id = unrelated.project_id().to_owned();
    let state = AppStateRepository::open(&state_root);
    let expected_ui = UiPreferences {
        left_collapsed: true,
        right_collapsed: false,
        api_card_expanded: false,
        right_panel_width: Some(640),
    };
    state.save_ui(expected_ui).unwrap();
    state
        .save_provider_credential_ref(ProviderId::Openai, "credential-ref-must-stay")
        .unwrap();
    state.record_project(&unrelated).unwrap();
    state
        .record_current_file(&unrelated_id, "project.md")
        .unwrap();

    let canonical_root = project_root.canonicalize().unwrap();
    let connection = Connection::open(state.database_path()).unwrap();
    connection
        .execute(
            "INSERT INTO recent_projects
             (project_id, root_path, display_name, last_opened_order)
             VALUES (?1, ?2, 'legacy', 99)",
            params![old_project_id, canonical_root.to_string_lossy()],
        )
        .unwrap();
    drop(connection);
    state
        .record_current_file(old_project_id, "chapters/old.md")
        .unwrap();

    let inspection = inspect_project_root(&project_root, ProjectScanMode::Open).unwrap();
    let reset_handle = open_project(&inspection, true).unwrap();
    let new_project_id = reset_handle.project_id().to_owned();
    state.record_project(&reset_handle).unwrap();

    let snapshot = state.snapshot();
    assert_eq!(snapshot.ui, expected_ui);
    assert!(snapshot
        .recent_projects
        .iter()
        .any(|project| project.project_id == new_project_id));
    assert!(snapshot
        .recent_projects
        .iter()
        .any(|project| project.project_id == unrelated_id));
    assert!(!snapshot
        .recent_projects
        .iter()
        .any(|project| project.project_id == old_project_id));
    assert!(!snapshot
        .project_files
        .iter()
        .any(|file| file.project_id == old_project_id));
    assert!(snapshot
        .project_files
        .iter()
        .any(|file| file.project_id == unrelated_id));
    assert!(state.recent_project_root(old_project_id).is_none());

    let openai = state
        .provider_settings()
        .unwrap()
        .into_iter()
        .find(|provider| provider.provider_id == ProviderId::Openai)
        .unwrap();
    assert_eq!(
        openai.credential_ref.as_deref(),
        Some("credential-ref-must-stay")
    );

    state
        .record_current_file(&new_project_id, "project.md")
        .unwrap();
    state.forget_project(&new_project_id);
    let after_forget = state.snapshot();
    assert!(!after_forget
        .recent_projects
        .iter()
        .any(|project| project.project_id == new_project_id));
    assert!(!after_forget
        .project_files
        .iter()
        .any(|file| file.project_id == new_project_id));
    assert!(after_forget
        .recent_projects
        .iter()
        .any(|project| project.project_id == unrelated_id));
}
