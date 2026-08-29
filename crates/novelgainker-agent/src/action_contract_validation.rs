use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const CONTRACT_SET_PATH: &str = "contract-set.json";
const EXPECTED_RESOURCES: [&str; 10] = [
    "action-tool.schema.json",
    "action-types.ts",
    "fixtures/deepseek-native-tool-call.json",
    "fixtures/invalid-absolute-path.json",
    "fixtures/invalid-duplicate-call-id.json",
    "fixtures/invalid-provider-authority.json",
    "fixtures/invalid-unknown-field.json",
    "fixtures/invalid-unknown-tool.json",
    "fixtures/openai-native-tool-call.json",
    "fixtures/strict-json-action.json",
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
pub struct ValidatedActionContractSet {
    pub contract_set_hash: String,
    pub resource_count: usize,
}

pub fn action_contract_set_hash(resources: &BTreeMap<String, Vec<u8>>) -> String {
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

pub fn validate_action_contract_set(
    resources: &BTreeMap<String, Vec<u8>>,
) -> Result<ValidatedActionContractSet, String> {
    if resources.len() != EXPECTED_RESOURCES.len() + 1 {
        return Err("agent-action-contract.resource-count-drift".to_owned());
    }
    let manifest: ContractSet = serde_json::from_slice(
        resources
            .get(CONTRACT_SET_PATH)
            .ok_or_else(|| "agent-action-contract.manifest-missing".to_owned())?,
    )
    .map_err(|_| "agent-action-contract.manifest-invalid".to_owned())?;
    if manifest.schema_version != 1
        || manifest.contract_set_id != "novelgainker.agent-action-tool-provider"
        || manifest.version != "1.0.0"
        || manifest.enabled_at_runtime
    {
        return Err("agent-action-contract.manifest-identity-drift".to_owned());
    }

    let expected = EXPECTED_RESOURCES.into_iter().collect::<BTreeSet<_>>();
    let mut locked = BTreeSet::new();
    for resource in &manifest.resources {
        if !locked.insert(resource.path.as_str()) || !expected.contains(resource.path.as_str()) {
            return Err("agent-action-contract.resource-path-drift".to_owned());
        }
        let bytes = resources
            .get(&resource.path)
            .ok_or_else(|| "agent-action-contract.resource-missing".to_owned())?;
        if bytes.starts_with(&[0xef, 0xbb, 0xbf])
            || bytes.contains(&b'\r')
            || std::str::from_utf8(bytes).is_err()
            || bytes.len() != resource.bytes
            || format!("{:x}", Sha256::digest(bytes)) != resource.sha256
        {
            return Err("agent-action-contract.resource-hash-drift".to_owned());
        }
    }
    if locked != expected {
        return Err("agent-action-contract.resource-set-drift".to_owned());
    }
    let computed_hash = action_contract_set_hash(resources);
    if computed_hash != manifest.contract_set_hash {
        return Err("agent-action-contract.contract-set-hash-drift".to_owned());
    }

    let schema: Value = serde_json::from_slice(&resources["action-tool.schema.json"])
        .map_err(|_| "agent-action-contract.schema-invalid".to_owned())?;
    if schema["$id"] != "https://novelgainker.local/schemas/agent/action-tool-v1.json" {
        return Err("agent-action-contract.schema-identity-drift".to_owned());
    }
    for definition in [
        "ToolArgumentsV1",
        "ModelToolRequestV1",
        "ProjectToolBindingV1",
        "ToolRequestV1",
        "ToolResultDataV1",
        "ToolResultV1",
        "AgentActionRecordContextV1",
        "AgentActionRecordV1",
    ] {
        if schema["$defs"][definition]["additionalProperties"] != false {
            return Err(format!(
                "agent-action-contract.schema-definition-invalid:{definition}"
            ));
        }
    }
    let typescript = std::str::from_utf8(&resources["action-types.ts"])
        .map_err(|_| "agent-action-contract.typescript-invalid".to_owned())?;
    for declaration in [
        "type AgentActionPayloadV1",
        "interface ToolRequestV1",
        "interface ToolResultV1",
        "interface AgentActionRecordV1",
        "untrusted_tool_data",
    ] {
        if !typescript.contains(declaration) {
            return Err(format!(
                "agent-action-contract.typescript-drift:{declaration}"
            ));
        }
    }
    if typescript.contains(": any") {
        return Err("agent-action-contract.typescript-unsafe".to_owned());
    }

    Ok(ValidatedActionContractSet {
        contract_set_hash: computed_hash,
        resource_count: EXPECTED_RESOURCES.len(),
    })
}
