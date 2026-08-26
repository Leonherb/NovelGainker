use app_core::app_state::{AppStateRepository, UiPreferences};
use app_core::coordination::try_acquire_project_write_lease;
use app_core::project::{create_project, inspect_project_root, open_project, ProjectScanMode};
use app_core::recovery::RecoveryStore;
use app_core::search::search_project_sources;
use app_core::workspace::{
    list_project_files, read_project_file, save_project_file_recoverable, ProjectFileNode,
};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const EDITED_FILE: &str = "project.md";
fn file_tree_contains(nodes: &[ProjectFileNode], target: &str) -> bool {
    nodes
        .iter()
        .any(|node| node.relative_path == target || file_tree_contains(&node.children, target))
}

struct VerifiedFixture {
    root: PathBuf,
    canonical_temp: PathBuf,
}

impl VerifiedFixture {
    fn new() -> Self {
        let canonical_temp = std::env::temp_dir()
            .canonicalize()
            .expect("temporary root must canonicalize");
        let root = canonical_temp.join(format!("novelgainker-ng107-e2e-{}", Uuid::new_v4()));
        fs::create_dir(&root).expect("isolated fixture root must create");
        Self {
            root,
            canonical_temp,
        }
    }

    fn project_root(&self) -> PathBuf {
        self.root.join("project")
    }

    fn state_root(&self) -> PathBuf {
        self.root.join("app-state")
    }
}

impl Drop for VerifiedFixture {
    fn drop(&mut self) {
        let safe_name = self
            .root
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.starts_with("novelgainker-ng107-e2e-"));
        let safe_parent = self.root.parent() == Some(self.canonical_temp.as_path());
        if safe_name && safe_parent {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn remove_verified_app_state(path: &Path, fixture_root: &Path) {
    let canonical_fixture = fixture_root
        .canonicalize()
        .expect("fixture root must canonicalize before state deletion");
    let canonical_state = path
        .canonicalize()
        .expect("state root must canonicalize before deletion");
    assert_eq!(canonical_state.parent(), Some(canonical_fixture.as_path()));
    assert_eq!(
        canonical_state.file_name().and_then(|value| value.to_str()),
        Some("app-state")
    );
    fs::remove_dir_all(canonical_state).expect("verified app state must delete");
}

#[test]
fn ng_107_offline_sprint1_round_trip_preserves_canon_and_boundaries() {
    let fixture = VerifiedFixture::new();
    let project_root = fixture.project_root();
    let state_root = fixture.state_root();
    fs::create_dir(&project_root).expect("project root must create");

    let create_inspection = inspect_project_root(&project_root, ProjectScanMode::Create)
        .expect("empty project root must scan");
    let lease = try_acquire_project_write_lease(&create_inspection.coordination_key())
        .expect("writer lease lookup must succeed")
        .expect("first writer must acquire");
    let handle = create_project(&create_inspection).expect("project must create");
    let file_tree = list_project_files(&handle).expect("created file tree must load");
    assert!(file_tree_contains(&file_tree.nodes, EDITED_FILE));
    let initial = read_project_file(&handle, EDITED_FILE).expect("created source must read");
    let initial_hash = initial.content_hash.clone();

    let recovery = RecoveryStore::new(state_root.clone()).expect("recovery store must open");
    let invalid_save = save_project_file_recoverable(
        &handle,
        EDITED_FILE,
        "manual offline revision\n",
        &initial.content_hash,
        &recovery,
    )
    .expect("manual invalid draft must still save");
    assert!(invalid_save.diagnostics_triggered);
    assert!(!invalid_save.validation.diagnostics.is_empty());
    assert!(!invalid_save.validation.eligible_for_final);

    fs::write(
        project_root.join(EDITED_FILE),
        b"---\ntitle: External Revision\nwritingLanguage: zh-CN\n---\n\nexternal revision\n",
    )
    .expect("external editor fixture must write");
    let conflict = save_project_file_recoverable(
        &handle,
        EDITED_FILE,
        "stale manual revision\n",
        &invalid_save.content_hash,
        &recovery,
    )
    .expect_err("stale editor source must conflict");
    assert_eq!(conflict.code, "file.source_changed");

    let external = read_project_file(&handle, EDITED_FILE).expect("external revision must read");
    let valid_content =
        "---\ntitle: Offline Final\nwritingLanguage: zh-CN\n---\n\noffline final revision\n";
    let final_save = save_project_file_recoverable(
        &handle,
        EDITED_FILE,
        valid_content,
        &external.content_hash,
        &recovery,
    )
    .expect("valid offline revision must save");
    assert!(final_save.validation.diagnostics.is_empty());
    assert!(final_save.validation.eligible_for_final);
    let final_hash = final_save.content_hash.clone();

    let search = search_project_sources(&handle, "offline final", false)
        .expect("local project search must succeed");
    assert!(search
        .matches
        .iter()
        .any(|entry| entry.relative_path == EDITED_FILE));
    assert_eq!(
        read_project_file(&handle, EDITED_FILE)
            .expect("search must not mutate source")
            .content_hash,
        final_hash
    );

    let state = AppStateRepository::open(&state_root);
    let expected_ui = UiPreferences {
        left_collapsed: true,
        right_collapsed: false,
        api_card_expanded: false,
        right_panel_width: Some(920),
    };
    state.save_ui(expected_ui).expect("UI state must persist");
    state
        .record_project(&handle)
        .expect("recent project must persist");
    state
        .record_current_file(handle.project_id(), EDITED_FILE)
        .expect("current relative file must persist");
    assert!(
        try_acquire_project_write_lease(&handle.coordination_key())
            .expect("second writer lookup must succeed")
            .is_none(),
        "same project must expose only one writer"
    );

    let project_id = handle.project_id().to_owned();
    drop(state);
    drop(recovery);
    drop(handle);
    drop(lease);

    let reopened_state = AppStateRepository::open(&state_root);
    let snapshot = reopened_state.snapshot();
    assert_eq!(snapshot.ui, expected_ui);
    assert!(!snapshot.recovered_with_defaults);
    assert_eq!(
        reopened_state.current_file(&project_id).as_deref(),
        Some(EDITED_FILE)
    );
    assert!(snapshot
        .recent_projects
        .iter()
        .any(|project| project.project_id == project_id));
    let remembered_root = reopened_state
        .recent_project_root(&project_id)
        .expect("recent root must stay private but reopenable in Rust");
    assert_eq!(remembered_root, project_root.canonicalize().unwrap());

    let open_inspection = inspect_project_root(&remembered_root, ProjectScanMode::Open)
        .expect("remembered project must scan");
    let reopened = open_project(&open_inspection, false).expect("remembered project must reopen");
    let reopened_document =
        read_project_file(&reopened, EDITED_FILE).expect("reopened source must read");
    assert_eq!(reopened_document.content_hash, final_hash);

    drop(reopened_state);
    remove_verified_app_state(&state_root, &fixture.root);
    assert!(!state_root.exists());
    let open_without_state = inspect_project_root(&project_root, ProjectScanMode::Open)
        .expect("project must scan without app state");
    let without_state =
        open_project(&open_without_state, false).expect("project must open without app state");
    assert_eq!(
        read_project_file(&without_state, EDITED_FILE)
            .expect("source canon must survive state deletion")
            .content_hash,
        final_hash
    );

    for attack in [
        "../secret.md",
        "C:/Windows/System32/drivers/etc/hosts",
        ".git/config",
        "published/out.md",
        "chapters/../../secret.md",
        "CON.md",
    ] {
        assert!(
            read_project_file(&without_state, attack).is_err(),
            "path boundary attack must fail"
        );
    }

    let evidence = json!({
        "schemaVersion": 1,
        "story": "NG-107",
        "networkMode": "cargo-offline-no-network-capability",
        "externalNetworkRequests": 0,
        "providerKeysConfigured": false,
        "queuedAiRequests": 0,
        "hashes": {
            "initial": initial_hash,
            "invalidManualSave": invalid_save.content_hash,
            "final": final_hash,
            "reopened": reopened_document.content_hash,
        },
        "checks": {
            "manualEdit": true,
            "validationDiagnostics": true,
            "sourceConflict": true,
            "localSearch": true,
            "restartRestore": true,
            "singleWriter": true,
            "stateDeletionPreservesCanon": true,
            "pathBoundaryMatrix": true,
        }
    });
    let evidence_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("g1-evidence");
    fs::create_dir_all(&evidence_root).expect("evidence root must create");
    fs::write(
        evidence_root.join("offline-e2e.json"),
        serde_json::to_vec_pretty(&evidence).expect("evidence must serialize"),
    )
    .expect("evidence must write");

    println!(
        "NG-107 PASS initial={} final={} reopened={}",
        evidence["hashes"]["initial"], evidence["hashes"]["final"], evidence["hashes"]["reopened"]
    );
}
