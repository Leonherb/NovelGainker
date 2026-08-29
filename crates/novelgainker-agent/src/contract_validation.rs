use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const CONTRACT_SET_PATH: &str = "contract-set.json";
const EXPECTED_RESOURCES: [&str; 5] = [
    "contract-types.ts",
    "fixtures/cross-project-turn.json",
    "fixtures/valid-session.json",
    "fixtures/valid-turn.json",
    "session-turn.schema.json",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContractSet {
    schema_version: u16,
    contract_set_id: String,
    version: String,
    enabled_at_runtime: bool,
    resources: Vec<LockedResource>,
    contract_set_hash: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LockedResource {
    path: String,
    bytes: usize,
    sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedContractSet {
    pub contract_set_hash: String,
    pub resource_count: usize,
}

pub fn contract_set_hash(resources: &BTreeMap<String, Vec<u8>>) -> String {
    let mut hash = Sha256::new();
    for (path, bytes) in resources {
        if path == CONTRACT_SET_PATH {
            continue;
        }
        hash.update(path.as_bytes());
        hash.update([0]);
        hash.update(format!("{:x}", Sha256::digest(bytes)).as_bytes());
        hash.update(b"\n");
    }
    format!("{:x}", hash.finalize())
}

pub fn validate_contract_set(
    resources: &BTreeMap<String, Vec<u8>>,
) -> Result<ValidatedContractSet, String> {
    if resources.len() != EXPECTED_RESOURCES.len() + 1 {
        return Err("agent-contract.resource-count-drift".to_owned());
    }
    let manifest_bytes = resources
        .get(CONTRACT_SET_PATH)
        .ok_or_else(|| "agent-contract.manifest-missing".to_owned())?;
    let manifest: ContractSet = serde_json::from_slice(manifest_bytes)
        .map_err(|_| "agent-contract.manifest-invalid".to_owned())?;
    if manifest.schema_version != 1
        || manifest.contract_set_id != "novelgainker.project-agent-session-turn"
        || manifest.version != "1.0.0"
        || manifest.enabled_at_runtime
    {
        return Err("agent-contract.manifest-identity-drift".to_owned());
    }

    let expected_paths = EXPECTED_RESOURCES.into_iter().collect::<BTreeSet<_>>();
    let mut locked_paths = BTreeSet::new();
    for locked in &manifest.resources {
        if !locked_paths.insert(locked.path.as_str())
            || !expected_paths.contains(locked.path.as_str())
        {
            return Err("agent-contract.resource-path-drift".to_owned());
        }
        let bytes = resources
            .get(&locked.path)
            .ok_or_else(|| "agent-contract.resource-missing".to_owned())?;
        validate_text(&locked.path, bytes)?;
        if bytes.len() != locked.bytes || format!("{:x}", Sha256::digest(bytes)) != locked.sha256 {
            return Err("agent-contract.resource-hash-drift".to_owned());
        }
    }
    if locked_paths != expected_paths {
        return Err("agent-contract.resource-set-drift".to_owned());
    }

    let computed_hash = contract_set_hash(resources);
    if computed_hash != manifest.contract_set_hash {
        return Err("agent-contract.contract-set-hash-drift".to_owned());
    }
    validate_schema(resources)?;
    validate_typescript(resources)?;
    validate_fixtures(resources)?;

    Ok(ValidatedContractSet {
        contract_set_hash: computed_hash,
        resource_count: EXPECTED_RESOURCES.len(),
    })
}

fn validate_text(path: &str, bytes: &[u8]) -> Result<(), String> {
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) || bytes.contains(&b'\r') {
        return Err(format!("agent-contract.text-format-invalid:{path}"));
    }
    std::str::from_utf8(bytes)
        .map(|_| ())
        .map_err(|_| format!("agent-contract.text-utf8-invalid:{path}"))
}

fn validate_schema(resources: &BTreeMap<String, Vec<u8>>) -> Result<(), String> {
    let schema: Value = serde_json::from_slice(&resources["session-turn.schema.json"])
        .map_err(|_| "agent-contract.schema-invalid".to_owned())?;
    if schema["$schema"] != "https://json-schema.org/draft/2020-12/schema"
        || schema["$id"] != "https://novelgainker.local/schemas/agent/session-turn-v1.json"
    {
        return Err("agent-contract.schema-identity-drift".to_owned());
    }
    for definition in [
        "ProjectAgentSessionV1",
        "AgentTurnV1",
        "ProjectAgentBasePolicyV1",
        "TurnBudgetV1",
    ] {
        let value = &schema["$defs"][definition];
        if value.is_null() || value["additionalProperties"] != false {
            return Err(format!(
                "agent-contract.schema-definition-invalid:{definition}"
            ));
        }
    }
    Ok(())
}

fn validate_typescript(resources: &BTreeMap<String, Vec<u8>>) -> Result<(), String> {
    let source = std::str::from_utf8(&resources["contract-types.ts"])
        .map_err(|_| "agent-contract.typescript-invalid".to_owned())?;
    for declaration in [
        "interface ProjectAgentSessionV1",
        "interface AgentTurnV1",
        "interface ProjectAgentBasePolicyV1",
        "allowsBypassChat: false",
        "maxWriteTransactions: 1",
    ] {
        if !source.contains(declaration) {
            return Err(format!("agent-contract.typescript-drift:{declaration}"));
        }
    }
    if source.contains(": any") || source.contains("enabledAtRuntime: true") {
        return Err("agent-contract.typescript-unsafe".to_owned());
    }
    Ok(())
}

fn validate_fixtures(resources: &BTreeMap<String, Vec<u8>>) -> Result<(), String> {
    for path in [
        "fixtures/valid-session.json",
        "fixtures/valid-turn.json",
        "fixtures/cross-project-turn.json",
    ] {
        let value: Value = serde_json::from_slice(&resources[path])
            .map_err(|_| format!("agent-contract.fixture-invalid:{path}"))?;
        if value["schemaVersion"] != 1 {
            return Err(format!("agent-contract.fixture-version-drift:{path}"));
        }
    }
    Ok(())
}
