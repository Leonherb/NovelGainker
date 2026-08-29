//! Build-validated Novelist-compatible Agent contracts and the AM1/AM2 runtime registry.
//!
//! NG-A001 freezes the Skill Pack, NG-A002 freezes Session/Turn/Base Policy types, NG-A003
//! freezes Action/Tool/Provider-normalization contracts, and NG-A004 freezes authorization,
//! persistence, transaction and undo contracts. AM1 adds the deterministic embedded read-only
//! registry; AM2 activates the two frozen governed write tools while execution remains app-owned.

pub mod action_contracts;
pub mod authorization;
pub mod contracts;
pub mod persistence_contracts;
pub mod provider_adapter;
pub mod runtime_registry;
pub mod session_machine;
pub mod transaction_contracts;
pub mod turn_budget;

pub const SKILL_PACK_SCHEMA_VERSION: u16 = 1;
pub const SKILL_PACK_VERSION: &str = "1.0.0";
pub const SKILL_PACK_ID: &str = "novelgainker.novelist-compatible";
pub const SKILL_PACK_SOURCE_COMMIT: &str = "64e4a2e1473ff0bad5893eff394ccbf1fa1cd580";
pub const SKILL_PACK_ENABLED_AT_RUNTIME: bool = false;
/// The immutable AM0 Pack manifest remains inactive by default. AM1 activates a separate
/// application-owned read-only view whose tools are limited by `runtime_registry`.
pub const AM1_READ_ONLY_RUNTIME_ENABLED: bool = true;
/// GA1-approved AM2 view: four read tools plus the two frozen governed Markdown write tools.
pub const AM2_GOVERNED_WRITE_RUNTIME_ENABLED: bool = true;
pub const SKILL_PACK_SHA256: &str = env!("NOVELGAINKER_SKILL_PACK_SHA256");
pub const AGENT_CONTRACT_SET_SHA256: &str = env!("NOVELGAINKER_AGENT_CONTRACT_SET_SHA256");
pub const AGENT_ACTION_CONTRACT_SET_SHA256: &str =
    env!("NOVELGAINKER_AGENT_ACTION_CONTRACT_SET_SHA256");
pub const AGENT_GOVERNANCE_CONTRACT_SET_SHA256: &str =
    env!("NOVELGAINKER_AGENT_GOVERNANCE_CONTRACT_SET_SHA256");

pub const SKILL_IDS: [&str; 9] = [
    "create-novel-project",
    "create-setting",
    "create-character",
    "create-material",
    "create-plot",
    "novel-story-telling",
    "analytical-review",
    "create-visual-asset",
    "publish-novel",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillPackMetadata {
    pub schema_version: u16,
    pub pack_id: &'static str,
    pub version: &'static str,
    pub source_commit: &'static str,
    pub pack_hash: &'static str,
    pub skill_count: usize,
    pub enabled_at_runtime: bool,
}

pub fn metadata() -> SkillPackMetadata {
    SkillPackMetadata {
        schema_version: SKILL_PACK_SCHEMA_VERSION,
        pack_id: SKILL_PACK_ID,
        version: SKILL_PACK_VERSION,
        source_commit: SKILL_PACK_SOURCE_COMMIT,
        pack_hash: SKILL_PACK_SHA256,
        skill_count: SKILL_IDS.len(),
        enabled_at_runtime: SKILL_PACK_ENABLED_AT_RUNTIME,
    }
}

pub fn pack_manifest_source() -> &'static str {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/skills/novelist/v1/pack.json"
    ))
}

pub fn provenance_source() -> &'static str {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/skills/novelist/v1/provenance.lock.json"
    ))
}

pub fn contract_set_source() -> &'static str {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/contracts/agent/v1/contract-set.json"
    ))
}

pub fn session_turn_schema_source() -> &'static str {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/contracts/agent/v1/session-turn.schema.json"
    ))
}

pub fn contract_typescript_source() -> &'static str {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/contracts/agent/v1/contract-types.ts"
    ))
}

pub fn action_contract_set_source() -> &'static str {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/contracts/agent-action/v1/contract-set.json"
    ))
}

pub fn action_tool_schema_source() -> &'static str {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/contracts/agent-action/v1/action-tool.schema.json"
    ))
}

pub fn action_typescript_source() -> &'static str {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/contracts/agent-action/v1/action-types.ts"
    ))
}

pub fn governance_contract_set_source() -> &'static str {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/contracts/agent-governance/v1/contract-set.json"
    ))
}

pub fn governance_schema_source() -> &'static str {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/contracts/agent-governance/v1/governance.schema.json"
    ))
}

pub fn governance_typescript_source() -> &'static str {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/contracts/agent-governance/v1/governance-types.ts"
    ))
}

pub fn risk_matrix_source() -> &'static str {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/contracts/agent-governance/v1/risk-matrix.json"
    ))
}

pub fn agent_read_action_schema_source() -> &'static str {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/contracts/agent-runtime/v1/agent-read-action.schema.json"
    ))
}

pub fn agent_write_action_schema_source() -> &'static str {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/contracts/agent-runtime/v2/agent-write-action.schema.json"
    ))
}

#[cfg(test)]
#[path = "skill_pack_validation.rs"]
mod skill_pack_validation;

#[cfg(test)]
#[path = "contract_validation.rs"]
mod contract_validation;

#[cfg(test)]
#[path = "contract_tests.rs"]
mod contract_tests;

#[cfg(test)]
#[path = "action_contract_validation.rs"]
mod action_contract_validation;

#[cfg(test)]
#[path = "action_contract_tests.rs"]
mod action_contract_tests;

#[cfg(test)]
#[path = "governance_contract_validation.rs"]
mod governance_contract_validation;

#[cfg(test)]
#[path = "governance_contract_tests.rs"]
mod governance_contract_tests;

#[cfg(test)]
mod tests {
    use super::{metadata, SKILL_IDS};
    use crate::skill_pack_validation::{pack_hash, validate_pack};
    use serde_json::{json, Value};
    use sha2::{Digest, Sha256};
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn collect(root: &Path, directory: &Path, resources: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let file_type = entry.file_type().unwrap();
            assert!(!file_type.is_symlink());
            let path = entry.path();
            if file_type.is_dir() {
                collect(root, &path, resources);
            } else {
                assert!(file_type.is_file());
                let relative = path
                    .strip_prefix(root)
                    .unwrap()
                    .components()
                    .map(|component| component.as_os_str().to_str().unwrap())
                    .collect::<Vec<_>>()
                    .join("/");
                resources.insert(relative, fs::read(path).unwrap());
            }
        }
    }

    fn resources() -> BTreeMap<String, Vec<u8>> {
        let root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources/skills/novelist/v1");
        let mut resources = BTreeMap::new();
        collect(&root, &root, &mut resources);
        resources
    }

    fn relock_inventory(resources: &mut BTreeMap<String, Vec<u8>>) {
        let mut inventory: Value = serde_json::from_slice(&resources["inventory.json"]).unwrap();
        for file in inventory["files"].as_array_mut().unwrap() {
            let resource_path = file["path"].as_str().unwrap();
            let bytes = resources.get(resource_path).unwrap();
            file["sha256"] = json!(format!("{:x}", Sha256::digest(bytes)));
            file["bytes"] = json!(bytes.len());
        }
        let mut serialized = serde_json::to_string_pretty(&inventory).unwrap();
        serialized.push('\n');
        resources.insert("inventory.json".to_owned(), serialized.into_bytes());
    }

    fn mutate_json_resource(
        resources: &mut BTreeMap<String, Vec<u8>>,
        resource_path: &str,
        mutator: impl FnOnce(&mut Value),
    ) {
        let mut value: Value = serde_json::from_slice(&resources[resource_path]).unwrap();
        mutator(&mut value);
        let mut serialized = serde_json::to_string_pretty(&value).unwrap();
        serialized.push('\n');
        resources.insert(resource_path.to_owned(), serialized.into_bytes());
        relock_inventory(resources);
    }

    #[test]
    fn build_validated_metadata_is_inactive_and_complete() {
        let resources = resources();
        let first = validate_pack(&resources).expect("checked-in Skill Pack must validate");
        let second = validate_pack(&resources).expect("validation must be deterministic");
        let metadata = metadata();

        assert_eq!(first, second);
        assert_eq!(first.skill_ids, SKILL_IDS);
        assert_eq!(first.pack_hash, metadata.pack_hash);
        assert_eq!(metadata.skill_count, 9);
        assert!(!metadata.enabled_at_runtime);
    }

    #[test]
    fn pack_hash_changes_with_resource_bytes() {
        let original = resources();
        let mut changed = original.clone();
        changed
            .get_mut("instructions/create-character.md")
            .unwrap()
            .extend_from_slice(b"\n");
        assert_ne!(pack_hash(&original), pack_hash(&changed));
        assert!(validate_pack(&changed).is_err());
    }

    #[test]
    fn rejects_unknown_files_missing_license_and_executable_payloads() {
        let mut unknown = resources();
        unknown.insert("unknown.md".to_owned(), b"unknown\n".to_vec());
        assert!(validate_pack(&unknown).is_err());

        let mut missing_license = resources();
        missing_license.remove("LICENSE.upstream");
        assert!(validate_pack(&missing_license).is_err());

        let mut executable = resources();
        executable.insert("payload.py".to_owned(), b"print('forbidden')\n".to_vec());
        assert!(validate_pack(&executable).is_err());
    }

    #[test]
    fn rejects_runtime_activation_tool_expansion_and_provenance_drift() {
        let mut enabled = resources();
        mutate_json_resource(
            &mut enabled,
            "manifests/create-character.json",
            |manifest| manifest["enabledAtRuntime"] = json!(true),
        );
        assert!(validate_pack(&enabled).is_err());

        let mut tool_expansion = resources();
        mutate_json_resource(
            &mut tool_expansion,
            "manifests/create-character.json",
            |manifest| manifest["allowedTools"] = json!(["filesystem.write"]),
        );
        assert!(validate_pack(&tool_expansion).is_err());

        let mut provenance_drift = resources();
        mutate_json_resource(
            &mut provenance_drift,
            "provenance.lock.json",
            |provenance| provenance["sourceCommit"] = json!("main"),
        );
        assert!(validate_pack(&provenance_drift).is_err());
    }
}
