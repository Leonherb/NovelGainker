use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const PACK_SCHEMA_VERSION: u16 = 1;
pub const PACK_ID: &str = "novelgainker.novelist-compatible";
pub const PACK_VERSION: &str = "1.0.0";
pub const FEATURE_GATE: &str = "agent_migration_skill_runtime_v1";
pub const SOURCE_REPOSITORY: &str = "https://github.com/comfuture/novelist";
pub const SOURCE_COMMIT: &str = "64e4a2e1473ff0bad5893eff394ccbf1fa1cd580";
pub const SOURCE_SUBTREE: &str = "plugins/novelist";
pub const LICENSE_SHA256: &str = "88b9a14391d40016e79135d201f277885eabf9b5388279a3ea0a4270e708c109";
pub const UPSTREAM_INVENTORY_SHA256: &str =
    "5831708ba45b668dac36c0e6541d25a9abccacfd7e06cb549d503a325b630362";

const EXPECTED_SKILL_IDS: [&str; 9] = [
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

const EXPECTED_MANIFEST_PATHS: [&str; 9] = [
    "manifests/create-novel-project.json",
    "manifests/create-setting.json",
    "manifests/create-character.json",
    "manifests/create-material.json",
    "manifests/create-plot.json",
    "manifests/novel-story-telling.json",
    "manifests/analytical-review.json",
    "manifests/create-visual-asset.json",
    "manifests/publish-novel.json",
];

const ALLOWED_TOOLS: [&str; 7] = [
    "project.create_markdown",
    "project.get_diff",
    "project.list_files",
    "project.patch_markdown",
    "project.read_markdown",
    "project.search_markdown",
    "project.validate_markdown",
];

const ALLOWED_CONFIRM_ACTIONS: [&str; 10] = [
    "asset.import",
    "asset.replace",
    "chapter.replace_whole",
    "chapter.transition_accepted_final",
    "document.replace_whole",
    "metadata.update_critical",
    "project.create_scaffold",
    "publication.build",
    "publication.replace",
    "story_ledger.update",
];

const FORBIDDEN_RESOURCE_SEGMENTS: [&str; 7] = [
    ".claude-plugin",
    ".codex-plugin",
    "agents",
    "scripts",
    "submission",
    "tests",
    "website",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PackManifest {
    schema_version: u16,
    pack_id: String,
    skill_pack_version: String,
    feature_gate: String,
    enabled_at_runtime: bool,
    provenance_resource: String,
    license_resource: String,
    attribution_resource: String,
    inventory_resource: String,
    skill_manifests: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SkillManifest {
    id: String,
    version: u16,
    description: String,
    trigger_summary: String,
    instruction_resources: Vec<String>,
    template_resources: Vec<String>,
    reference_resources: Vec<String>,
    allowed_tools: Vec<String>,
    read_scopes: Vec<String>,
    write_scopes: Vec<String>,
    risk_class: String,
    always_confirm_actions: Vec<String>,
    context_budget: ContextBudget,
    output_contract: String,
    source_commit: String,
    resource_hashes: BTreeMap<String, String>,
    feature_gate: String,
    enabled_at_runtime: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContextBudget {
    max_request_bytes: u32,
    max_unique_files: u16,
    max_modified_files: u16,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Inventory {
    schema_version: u16,
    algorithm: String,
    excluded_resources: Vec<String>,
    files: Vec<InventoryFile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InventoryFile {
    path: String,
    sha256: String,
    bytes: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Provenance {
    schema_version: u16,
    source_repository: String,
    source_commit: String,
    source_subtree: String,
    retrieved_at: String,
    source_archive_sha256: String,
    license_spdx: String,
    license_text_sha256: String,
    upstream_file_count: u64,
    upstream_inventory_algorithm: String,
    upstream_inventory_sha256: String,
    adaptation_version: String,
    novel_gainker_pack_version: String,
    included_files: Vec<IncludedFile>,
    excluded_files: Vec<ExcludedFile>,
    local_changes: Vec<LocalChange>,
    generated_from_scripts: Vec<GeneratedFromScript>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct IncludedFile {
    source_path: String,
    source_sha256: String,
    integration_mode: String,
    local_resources: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExcludedFile {
    source_pattern: String,
    reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LocalChange {
    id: String,
    description: String,
    local_resources: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GeneratedFromScript {
    source_path: String,
    target_contract: String,
    status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPack {
    pub pack_hash: String,
    pub skill_ids: Vec<String>,
}

fn invalid(code: &str) -> String {
    code.to_owned()
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn validate_source_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains(':')
        || path.chars().any(char::is_control)
        || path
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(invalid("skill-pack.source-path-invalid"));
    }
    Ok(())
}

fn validate_scope(scope: &str) -> Result<(), String> {
    if scope.is_empty()
        || scope.starts_with('/')
        || scope.contains('\\')
        || scope.contains(':')
        || scope.chars().any(char::is_control)
        || scope
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(invalid("skill-pack.scope-invalid"));
    }
    Ok(())
}

fn validate_resource_path(path: &str) -> Result<(), String> {
    validate_source_path(path)?;
    if path.contains('*') || path.contains('?') {
        return Err(invalid("skill-pack.resource-path-invalid"));
    }
    if path.split('/').any(|segment| {
        FORBIDDEN_RESOURCE_SEGMENTS
            .iter()
            .any(|forbidden| segment.eq_ignore_ascii_case(forbidden))
    }) {
        return Err(invalid("skill-pack.resource-segment-forbidden"));
    }
    let extension = path.rsplit_once('.').map(|(_, extension)| extension);
    if !matches!(extension, Some("json" | "md" | "upstream")) {
        return Err(invalid("skill-pack.resource-extension-forbidden"));
    }
    Ok(())
}

fn resource_text<'a>(
    resources: &'a BTreeMap<String, Vec<u8>>,
    path: &str,
) -> Result<&'a str, String> {
    validate_resource_path(path)?;
    let bytes = resources
        .get(path)
        .ok_or_else(|| invalid("skill-pack.resource-missing"))?;
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err(invalid("skill-pack.resource-bom-forbidden"));
    }
    if bytes.contains(&b'\r') {
        return Err(invalid("skill-pack.resource-line-ending-invalid"));
    }
    std::str::from_utf8(bytes).map_err(|_| invalid("skill-pack.resource-encoding-invalid"))
}

fn unique_strings(values: &[String], code: &str) -> Result<(), String> {
    let unique = values.iter().map(String::as_str).collect::<BTreeSet<_>>();
    if unique.len() != values.len() {
        return Err(invalid(code));
    }
    Ok(())
}

fn validate_inventory(resources: &BTreeMap<String, Vec<u8>>) -> Result<(), String> {
    let inventory: Inventory = serde_json::from_str(resource_text(resources, "inventory.json")?)
        .map_err(|_| invalid("skill-pack.inventory-invalid"))?;
    if inventory.schema_version != 1
        || inventory.algorithm != "sha256"
        || inventory.excluded_resources.as_slice() != ["inventory.json"]
    {
        return Err(invalid("skill-pack.inventory-contract-invalid"));
    }

    let expected_paths = resources
        .keys()
        .filter(|path| path.as_str() != "inventory.json")
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let actual_paths = inventory
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<BTreeSet<_>>();
    if expected_paths != actual_paths || inventory.files.len() != expected_paths.len() {
        return Err(invalid("skill-pack.inventory-resource-set-invalid"));
    }
    if inventory
        .files
        .windows(2)
        .any(|window| window[0].path >= window[1].path)
    {
        return Err(invalid("skill-pack.inventory-order-invalid"));
    }
    for file in &inventory.files {
        validate_resource_path(&file.path)?;
        let bytes = resources
            .get(&file.path)
            .ok_or_else(|| invalid("skill-pack.inventory-resource-missing"))?;
        if file.bytes != bytes.len() as u64
            || !is_sha256(&file.sha256)
            || file.sha256 != sha256_hex(bytes)
        {
            return Err(invalid("skill-pack.inventory-hash-invalid"));
        }
    }
    Ok(())
}

fn validate_provenance(
    provenance: &Provenance,
    resources: &BTreeMap<String, Vec<u8>>,
) -> Result<(), String> {
    if provenance.schema_version != 1
        || provenance.source_repository != SOURCE_REPOSITORY
        || provenance.source_commit != SOURCE_COMMIT
        || provenance.source_subtree != SOURCE_SUBTREE
        || provenance.retrieved_at.trim().is_empty()
        || !is_sha256(&provenance.source_archive_sha256)
        || provenance.license_spdx != "MIT"
        || provenance.license_text_sha256 != LICENSE_SHA256
        || provenance.upstream_file_count != 78
        || provenance.upstream_inventory_algorithm
            != "sha256(utf8(sort(path + tab + byteLength + tab + fileSha256 + lf)))"
        || provenance.upstream_inventory_sha256 != UPSTREAM_INVENTORY_SHA256
        || provenance.adaptation_version != "1.0.0"
        || provenance.novel_gainker_pack_version != PACK_VERSION
    {
        return Err(invalid("skill-pack.provenance-contract-invalid"));
    }

    if provenance.included_files.is_empty()
        || provenance.excluded_files.is_empty()
        || provenance.local_changes.is_empty()
        || provenance.generated_from_scripts.len() != 13
    {
        return Err(invalid("skill-pack.provenance-inventory-invalid"));
    }

    let mut source_paths = BTreeSet::new();
    for included in &provenance.included_files {
        validate_source_path(&included.source_path)?;
        if !source_paths.insert(included.source_path.as_str())
            || !is_sha256(&included.source_sha256)
            || !matches!(included.integration_mode.as_str(), "adapted" | "verbatim")
            || included.local_resources.is_empty()
        {
            return Err(invalid("skill-pack.provenance-included-file-invalid"));
        }
        for local_resource in &included.local_resources {
            validate_resource_path(local_resource)?;
            if !resources.contains_key(local_resource) {
                return Err(invalid("skill-pack.provenance-local-resource-missing"));
            }
        }
        if included.integration_mode == "verbatim"
            && (included.local_resources.len() != 1
                || sha256_hex(
                    resources
                        .get(&included.local_resources[0])
                        .expect("validated local resource must exist"),
                ) != included.source_sha256)
        {
            return Err(invalid("skill-pack.provenance-verbatim-hash-invalid"));
        }
    }
    if !source_paths.contains("LICENSE") {
        return Err(invalid("skill-pack.provenance-license-missing"));
    }
    for skill_id in EXPECTED_SKILL_IDS {
        let expected = format!("plugins/novelist/skills/{skill_id}/SKILL.md");
        if !source_paths.contains(expected.as_str()) {
            return Err(invalid("skill-pack.provenance-skill-source-missing"));
        }
    }

    for excluded in &provenance.excluded_files {
        if excluded.source_pattern.trim().is_empty() || excluded.reason.trim().is_empty() {
            return Err(invalid("skill-pack.provenance-exclusion-invalid"));
        }
    }
    let mut change_ids = BTreeSet::new();
    for change in &provenance.local_changes {
        if change.id.trim().is_empty()
            || change.description.trim().is_empty()
            || change.local_resources.is_empty()
            || !change_ids.insert(change.id.as_str())
        {
            return Err(invalid("skill-pack.provenance-local-change-invalid"));
        }
        for local_resource in &change.local_resources {
            validate_scope(local_resource)?;
        }
    }
    let mut script_paths = BTreeSet::new();
    for mapping in &provenance.generated_from_scripts {
        validate_source_path(&mapping.source_path)?;
        if !mapping.source_path.ends_with(".py")
            || !mapping.target_contract.starts_with("future:")
            || mapping.status != "excluded-awaiting-authorized-port"
            || !script_paths.insert(mapping.source_path.as_str())
        {
            return Err(invalid("skill-pack.provenance-script-mapping-invalid"));
        }
    }
    Ok(())
}

fn expected_risk_class(skill_id: &str) -> Option<&'static str> {
    match skill_id {
        "create-novel-project" | "publish-novel" => Some("high"),
        "create-setting" | "create-character" | "create-material" | "create-plot" => Some("mixed"),
        "novel-story-telling" | "create-visual-asset" => Some("high/mixed"),
        "analytical-review" => Some("read-only"),
        _ => None,
    }
}

fn validate_manifest(
    manifest: &SkillManifest,
    manifest_path: &str,
    resources: &BTreeMap<String, Vec<u8>>,
) -> Result<(), String> {
    let expected_manifest_path = format!("manifests/{}.json", manifest.id);
    let expected_instruction_path = format!("instructions/{}.md", manifest.id);
    if manifest.version != 1
        || expected_manifest_path != manifest_path
        || manifest.description.trim().is_empty()
        || manifest.description.chars().count() > 160
        || manifest.trigger_summary.trim().is_empty()
        || manifest.trigger_summary.chars().count() > 160
        || manifest.instruction_resources.as_slice() != [expected_instruction_path.as_str()]
        || manifest.output_contract != "AgentActionPayloadV1"
        || manifest.source_commit != SOURCE_COMMIT
        || manifest.feature_gate != FEATURE_GATE
        || manifest.enabled_at_runtime
        || expected_risk_class(&manifest.id) != Some(manifest.risk_class.as_str())
    {
        return Err(invalid("skill-pack.manifest-contract-invalid"));
    }

    if manifest.allowed_tools.is_empty() {
        return Err(invalid("skill-pack.manifest-tools-empty"));
    }
    unique_strings(
        &manifest.allowed_tools,
        "skill-pack.manifest-tools-duplicate",
    )?;
    for tool in &manifest.allowed_tools {
        if !ALLOWED_TOOLS.contains(&tool.as_str()) {
            return Err(invalid("skill-pack.manifest-tool-forbidden"));
        }
    }
    unique_strings(
        &manifest.always_confirm_actions,
        "skill-pack.manifest-confirmation-duplicate",
    )?;
    for action in &manifest.always_confirm_actions {
        if !ALLOWED_CONFIRM_ACTIONS.contains(&action.as_str()) {
            return Err(invalid("skill-pack.manifest-confirmation-forbidden"));
        }
    }
    if manifest.risk_class == "read-only" {
        if !manifest.write_scopes.is_empty()
            || !manifest.always_confirm_actions.is_empty()
            || manifest.context_budget.max_modified_files != 0
        {
            return Err(invalid("skill-pack.manifest-read-only-contract-invalid"));
        }
    } else if manifest.always_confirm_actions.is_empty() {
        return Err(invalid("skill-pack.manifest-confirmation-required"));
    }

    if manifest.context_budget.max_request_bytes == 0
        || manifest.context_budget.max_request_bytes > 98_304
        || manifest.context_budget.max_unique_files == 0
        || manifest.context_budget.max_unique_files > 24
        || manifest.context_budget.max_modified_files > 8
    {
        return Err(invalid("skill-pack.manifest-context-budget-invalid"));
    }
    unique_strings(
        &manifest.read_scopes,
        "skill-pack.manifest-read-scope-duplicate",
    )?;
    unique_strings(
        &manifest.write_scopes,
        "skill-pack.manifest-write-scope-duplicate",
    )?;
    for scope in manifest
        .read_scopes
        .iter()
        .chain(manifest.write_scopes.iter())
    {
        validate_scope(scope)?;
    }

    let manifest_resources = manifest
        .instruction_resources
        .iter()
        .chain(manifest.template_resources.iter())
        .chain(manifest.reference_resources.iter())
        .map(String::as_str)
        .collect::<Vec<_>>();
    if manifest_resources.is_empty()
        || manifest_resources
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            != manifest_resources.len()
    {
        return Err(invalid("skill-pack.manifest-resource-set-invalid"));
    }
    let expected_hash_paths = manifest_resources.iter().copied().collect::<BTreeSet<_>>();
    let actual_hash_paths = manifest
        .resource_hashes
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if expected_hash_paths != actual_hash_paths {
        return Err(invalid("skill-pack.manifest-resource-hash-set-invalid"));
    }
    for resource_path in manifest_resources {
        let text = resource_text(resources, resource_path)?;
        let expected_hash = manifest
            .resource_hashes
            .get(resource_path)
            .ok_or_else(|| invalid("skill-pack.manifest-resource-hash-missing"))?;
        if !is_sha256(expected_hash)
            || expected_hash
                != &sha256_hex(
                    resources
                        .get(resource_path)
                        .expect("validated manifest resource must exist"),
                )
        {
            return Err(invalid("skill-pack.manifest-resource-hash-invalid"));
        }
        if resource_path == expected_instruction_path && !text.contains("AgentActionPayloadV1") {
            return Err(invalid("skill-pack.manifest-instruction-contract-invalid"));
        }
    }
    Ok(())
}

pub(crate) fn pack_hash(resources: &BTreeMap<String, Vec<u8>>) -> String {
    let mut hasher = Sha256::new();
    for (path, bytes) in resources {
        hasher.update(path.as_bytes());
        hasher.update([0]);
        hasher.update(sha256_hex(bytes).as_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())
}

pub fn validate_pack(resources: &BTreeMap<String, Vec<u8>>) -> Result<ValidatedPack, String> {
    if resources.is_empty() {
        return Err(invalid("skill-pack.empty"));
    }
    for path in resources.keys() {
        resource_text(resources, path)?;
    }
    validate_inventory(resources)?;

    let pack: PackManifest = serde_json::from_str(resource_text(resources, "pack.json")?)
        .map_err(|_| invalid("skill-pack.pack-manifest-invalid"))?;
    if pack.schema_version != PACK_SCHEMA_VERSION
        || pack.pack_id != PACK_ID
        || pack.skill_pack_version != PACK_VERSION
        || pack.feature_gate != FEATURE_GATE
        || pack.enabled_at_runtime
        || pack.provenance_resource != "provenance.lock.json"
        || pack.license_resource != "LICENSE.upstream"
        || pack.attribution_resource != "ATTRIBUTION.md"
        || pack.inventory_resource != "inventory.json"
        || pack.skill_manifests.as_slice() != EXPECTED_MANIFEST_PATHS
    {
        return Err(invalid("skill-pack.pack-contract-invalid"));
    }

    let license = resources
        .get(&pack.license_resource)
        .ok_or_else(|| invalid("skill-pack.license-missing"))?;
    if sha256_hex(license) != LICENSE_SHA256
        || !resource_text(resources, &pack.license_resource)?.starts_with("MIT License\n")
    {
        return Err(invalid("skill-pack.license-invalid"));
    }
    let attribution = resource_text(resources, &pack.attribution_resource)?;
    for required in [
        "comfuture/novelist",
        SOURCE_COMMIT,
        "MIT License",
        "Changkyun Kim",
    ] {
        if !attribution.contains(required) {
            return Err(invalid("skill-pack.attribution-invalid"));
        }
    }

    let provenance: Provenance =
        serde_json::from_str(resource_text(resources, &pack.provenance_resource)?)
            .map_err(|_| invalid("skill-pack.provenance-invalid"))?;
    validate_provenance(&provenance, resources)?;

    let mut skill_ids = Vec::with_capacity(pack.skill_manifests.len());
    let mut unique_ids = BTreeSet::new();
    for (index, manifest_path) in pack.skill_manifests.iter().enumerate() {
        let manifest: SkillManifest =
            serde_json::from_str(resource_text(resources, manifest_path)?)
                .map_err(|_| invalid("skill-pack.skill-manifest-invalid"))?;
        if manifest.id != EXPECTED_SKILL_IDS[index] || !unique_ids.insert(manifest.id.clone()) {
            return Err(invalid("skill-pack.skill-id-invalid"));
        }
        validate_manifest(&manifest, manifest_path, resources)?;
        skill_ids.push(manifest.id);
    }

    Ok(ValidatedPack {
        pack_hash: pack_hash(resources),
        skill_ids,
    })
}
