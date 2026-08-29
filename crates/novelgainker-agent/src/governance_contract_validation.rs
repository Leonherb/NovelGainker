use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const CONTRACT_SET_PATH: &str = "contract-set.json";
const EXPECTED_RESOURCES: [&str; 10] = [
    "fixtures/invalid-cross-project.json",
    "fixtures/invalid-plaintext-full-chat.json",
    "fixtures/invalid-sensitive-canary.json",
    "fixtures/invalid-stale-confirmation.json",
    "fixtures/invalid-undo-over-limit.json",
    "fixtures/valid-persistence-snapshot.json",
    "fixtures/valid-transaction-plan.json",
    "governance-types.ts",
    "governance.schema.json",
    "risk-matrix.json",
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
pub struct ValidatedGovernanceContractSet {
    pub contract_set_hash: String,
    pub resource_count: usize,
}

pub fn governance_contract_set_hash(resources: &BTreeMap<String, Vec<u8>>) -> String {
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

pub fn validate_governance_contract_set(
    resources: &BTreeMap<String, Vec<u8>>,
) -> Result<ValidatedGovernanceContractSet, String> {
    if resources.len() != EXPECTED_RESOURCES.len() + 1 {
        return Err("agent-governance-contract.resource-count-drift".to_owned());
    }
    let manifest: ContractSet = serde_json::from_slice(
        resources
            .get(CONTRACT_SET_PATH)
            .ok_or_else(|| "agent-governance-contract.manifest-missing".to_owned())?,
    )
    .map_err(|_| "agent-governance-contract.manifest-invalid".to_owned())?;
    if manifest.schema_version != 1
        || manifest.contract_set_id != "novelgainker.agent-governance-persistence-transaction-undo"
        || manifest.version != "1.0.0"
        || manifest.enabled_at_runtime
    {
        return Err("agent-governance-contract.manifest-identity-drift".to_owned());
    }

    let expected = EXPECTED_RESOURCES.into_iter().collect::<BTreeSet<_>>();
    let mut locked = BTreeSet::new();
    for resource in &manifest.resources {
        if !locked.insert(resource.path.as_str()) || !expected.contains(resource.path.as_str()) {
            return Err("agent-governance-contract.resource-path-drift".to_owned());
        }
        let bytes = resources
            .get(&resource.path)
            .ok_or_else(|| "agent-governance-contract.resource-missing".to_owned())?;
        if bytes.starts_with(&[0xef, 0xbb, 0xbf])
            || bytes.contains(&b'\r')
            || std::str::from_utf8(bytes).is_err()
            || bytes.len() != resource.bytes
            || format!("{:x}", Sha256::digest(bytes)) != resource.sha256
        {
            return Err("agent-governance-contract.resource-hash-drift".to_owned());
        }
    }
    if locked != expected {
        return Err("agent-governance-contract.resource-set-drift".to_owned());
    }
    let computed_hash = governance_contract_set_hash(resources);
    if computed_hash != manifest.contract_set_hash {
        return Err("agent-governance-contract.contract-set-hash-drift".to_owned());
    }

    let schema: Value = serde_json::from_slice(&resources["governance.schema.json"])
        .map_err(|_| "agent-governance-contract.schema-invalid".to_owned())?;
    if schema["$id"] != "https://novelgainker.local/schemas/agent/governance-v1.json" {
        return Err("agent-governance-contract.schema-identity-drift".to_owned());
    }
    for definition in [
        "AuthorizationDecisionV1",
        "TurnAuthorizationV1",
        "ConfirmationBindingV1",
        "HighRiskConfirmationV1",
        "FullChatPersistenceV1",
        "SessionPersistenceSnapshotV1",
        "UndoRecordV1",
        "UndoCatalogV1",
        "MultiFileTransactionPlanV1",
        "TransactionReceiptV1",
    ] {
        if schema["$defs"][definition]["additionalProperties"] != false {
            return Err(format!(
                "agent-governance-contract.schema-definition-invalid:{definition}"
            ));
        }
    }

    let matrix: Value = serde_json::from_slice(&resources["risk-matrix.json"])
        .map_err(|_| "agent-governance-contract.risk-matrix-invalid".to_owned())?;
    if matrix["schemaVersion"] != 1
        || matrix["policies"].as_array().map(Vec::len) != Some(3)
        || matrix["riskLevels"].as_array().map(Vec::len) != Some(4)
        || matrix["registeredTools"].as_array().map(Vec::len) != Some(6)
        || matrix["actions"].as_array().map(Vec::len) != Some(21)
    {
        return Err("agent-governance-contract.risk-matrix-drift".to_owned());
    }

    let typescript = std::str::from_utf8(&resources["governance-types.ts"])
        .map_err(|_| "agent-governance-contract.typescript-invalid".to_owned())?;
    for declaration in [
        "type AuthorizationPolicyV1",
        "type RiskLevelV1",
        "interface TurnAuthorizationV1",
        "interface HighRiskConfirmationV1",
        "interface SessionPersistenceSnapshotV1",
        "interface MultiFileTransactionPlanV1",
        "interface UndoRecordV1",
        "windows_user_context",
    ] {
        if !typescript.contains(declaration) {
            return Err(format!(
                "agent-governance-contract.typescript-drift:{declaration}"
            ));
        }
    }
    if typescript.contains(": any") || typescript.contains("plaintext") {
        return Err("agent-governance-contract.typescript-unsafe".to_owned());
    }

    Ok(ValidatedGovernanceContractSet {
        contract_set_hash: computed_hash,
        resource_count: EXPECTED_RESOURCES.len(),
    })
}
