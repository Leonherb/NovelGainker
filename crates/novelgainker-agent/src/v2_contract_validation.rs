use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const MANIFEST_PATH: &str = "contract-set.json";
const EXPECTED: [&str; 6] = [
    "agent-control-action.schema.json",
    "canon-proposal.schema.json",
    "contract-types.ts",
    "memory-compaction.schema.json",
    "persistence.schema.json",
    "skill-selection.schema.json",
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContractSet {
    schema_version: u16,
    contract_set_id: String,
    contract_set_version: String,
    runtime_enabled: bool,
    resources: Vec<LockedResource>,
    contract_set_sha256: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LockedResource {
    path: String,
    bytes: usize,
    sha256: String,
}

pub struct ValidatedV2ContractSet {
    pub contract_set_hash: String,
    pub resource_count: usize,
}

pub fn validate_v2_contract_set(
    resources: &BTreeMap<String, Vec<u8>>,
) -> Result<ValidatedV2ContractSet, String> {
    if resources.len() != EXPECTED.len() + 1 {
        return Err("agent-v2.resource-count-drift".to_owned());
    }
    let manifest: ContractSet = serde_json::from_slice(
        resources
            .get(MANIFEST_PATH)
            .ok_or_else(|| "agent-v2.manifest-missing".to_owned())?,
    )
    .map_err(|_| "agent-v2.manifest-invalid".to_owned())?;
    if manifest.schema_version != 2
        || manifest.contract_set_id != "novelgainker.agent-v2"
        || manifest.contract_set_version != "2.0.0"
        || !manifest.runtime_enabled
    {
        return Err("agent-v2.manifest-identity-drift".to_owned());
    }
    let expected = EXPECTED.into_iter().collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::new();
    for locked in &manifest.resources {
        if !actual.insert(locked.path.as_str()) || !expected.contains(locked.path.as_str()) {
            return Err("agent-v2.resource-path-drift".to_owned());
        }
        let bytes = resources
            .get(&locked.path)
            .ok_or_else(|| "agent-v2.resource-missing".to_owned())?;
        if bytes.starts_with(&[0xef, 0xbb, 0xbf])
            || bytes.contains(&b'\r')
            || std::str::from_utf8(bytes).is_err()
            || bytes.len() != locked.bytes
            || format!("{:x}", Sha256::digest(bytes)) != locked.sha256
        {
            return Err("agent-v2.resource-hash-drift".to_owned());
        }
    }
    if actual != expected {
        return Err("agent-v2.resource-set-drift".to_owned());
    }
    let mut aggregate = Sha256::new();
    for path in EXPECTED {
        let bytes = &resources[path];
        aggregate.update(path.as_bytes());
        aggregate.update([0]);
        aggregate.update(format!("{:x}", Sha256::digest(bytes)).as_bytes());
        aggregate.update(b"\n");
    }
    let contract_set_hash = format!("{:x}", aggregate.finalize());
    if contract_set_hash != manifest.contract_set_sha256 {
        return Err("agent-v2.aggregate-hash-drift".to_owned());
    }
    validate_schema(resources, "skill-selection.schema.json")?;
    validate_schema(resources, "agent-control-action.schema.json")?;
    validate_schema(resources, "canon-proposal.schema.json")?;
    validate_schema(resources, "persistence.schema.json")?;
    validate_schema(resources, "memory-compaction.schema.json")?;
    let types = std::str::from_utf8(&resources["contract-types.ts"])
        .map_err(|_| "agent-v2.typescript-invalid".to_owned())?;
    for declaration in [
        "SkillSelectionV2",
        "AgentControlActionV2",
        "CanonProposalV1",
        "ConversationMessageV2",
    ] {
        if !types.contains(&format!("type {declaration}")) {
            return Err(format!("agent-v2.typescript-drift:{declaration}"));
        }
    }
    Ok(ValidatedV2ContractSet {
        contract_set_hash,
        resource_count: EXPECTED.len(),
    })
}

fn validate_schema(resources: &BTreeMap<String, Vec<u8>>, path: &str) -> Result<(), String> {
    let schema: Value = serde_json::from_slice(&resources[path])
        .map_err(|_| format!("agent-v2.schema-invalid:{path}"))?;
    if schema["$schema"] != "https://json-schema.org/draft/2020-12/schema" {
        return Err(format!("agent-v2.schema-dialect-drift:{path}"));
    }
    Ok(())
}
