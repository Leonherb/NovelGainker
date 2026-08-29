#[path = "src/skill_pack_validation.rs"]
mod skill_pack_validation;

#[path = "src/contract_validation.rs"]
mod contract_validation;

#[path = "src/action_contract_validation.rs"]
mod action_contract_validation;

#[path = "src/governance_contract_validation.rs"]
mod governance_contract_validation;

use action_contract_validation::validate_action_contract_set;
use contract_validation::validate_contract_set;
use governance_contract_validation::validate_governance_contract_set;
use skill_pack_validation::validate_pack;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn collect_resources(
    root: &Path,
    directory: &Path,
    resources: &mut BTreeMap<String, Vec<u8>>,
) -> Result<(), String> {
    let entries = fs::read_dir(directory)
        .map_err(|_| "skill-pack.resource-directory-unreadable".to_owned())?;
    for entry in entries {
        let entry = entry.map_err(|_| "skill-pack.resource-entry-unreadable".to_owned())?;
        let file_type = entry
            .file_type()
            .map_err(|_| "skill-pack.resource-type-unreadable".to_owned())?;
        let path = entry.path();
        if file_type.is_symlink() {
            return Err("skill-pack.resource-symlink-forbidden".to_owned());
        }
        if file_type.is_dir() {
            collect_resources(root, &path, resources)?;
            continue;
        }
        if !file_type.is_file() {
            return Err("skill-pack.resource-type-forbidden".to_owned());
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| "skill-pack.resource-path-invalid".to_owned())?;
        let relative_path = relative
            .components()
            .map(|component| {
                component
                    .as_os_str()
                    .to_str()
                    .ok_or_else(|| "skill-pack.resource-path-non-utf8".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?
            .join("/");
        let bytes = fs::read(&path)
            .map_err(|_| format!("skill-pack.resource-unreadable:{relative_path}"))?;
        if resources.insert(relative_path, bytes).is_some() {
            return Err("skill-pack.resource-path-duplicate".to_owned());
        }
        println!("cargo:rerun-if-changed={}", path.display());
    }
    Ok(())
}

fn main() {
    let manifest_dir = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be available"),
    );
    let resource_root = manifest_dir.join("../../resources/skills/novelist/v1");
    println!("cargo:rerun-if-changed={}", resource_root.display());

    let mut resources = BTreeMap::new();
    collect_resources(&resource_root, &resource_root, &mut resources)
        .and_then(|_| validate_pack(&resources))
        .map(|validated| {
            println!(
                "cargo:rustc-env=NOVELGAINKER_SKILL_PACK_SHA256={}",
                validated.pack_hash
            );
            println!(
                "cargo:rustc-env=NOVELGAINKER_SKILL_PACK_SKILL_COUNT={}",
                validated.skill_ids.len()
            );
        })
        .unwrap_or_else(|error| {
            panic!("NovelGainker Novelist-compatible Skill Pack v1 validation failed: {error}")
        });

    let contract_root = manifest_dir.join("../../resources/contracts/agent/v1");
    println!("cargo:rerun-if-changed={}", contract_root.display());
    let mut contract_resources = BTreeMap::new();
    collect_resources(&contract_root, &contract_root, &mut contract_resources)
        .and_then(|_| validate_contract_set(&contract_resources))
        .map(|validated| {
            println!(
                "cargo:rustc-env=NOVELGAINKER_AGENT_CONTRACT_SET_SHA256={}",
                validated.contract_set_hash
            );
            println!(
                "cargo:rustc-env=NOVELGAINKER_AGENT_CONTRACT_RESOURCE_COUNT={}",
                validated.resource_count
            );
        })
        .unwrap_or_else(|error| {
            panic!("NovelGainker Agent Session/Turn v1 validation failed: {error}")
        });

    let action_contract_root = manifest_dir.join("../../resources/contracts/agent-action/v1");
    println!("cargo:rerun-if-changed={}", action_contract_root.display());
    let mut action_contract_resources = BTreeMap::new();
    collect_resources(
        &action_contract_root,
        &action_contract_root,
        &mut action_contract_resources,
    )
    .and_then(|_| validate_action_contract_set(&action_contract_resources))
    .map(|validated| {
        println!(
            "cargo:rustc-env=NOVELGAINKER_AGENT_ACTION_CONTRACT_SET_SHA256={}",
            validated.contract_set_hash
        );
        println!(
            "cargo:rustc-env=NOVELGAINKER_AGENT_ACTION_CONTRACT_RESOURCE_COUNT={}",
            validated.resource_count
        );
    })
    .unwrap_or_else(|error| {
        panic!("NovelGainker Agent Action/Tool/Provider v1 validation failed: {error}")
    });

    let governance_contract_root =
        manifest_dir.join("../../resources/contracts/agent-governance/v1");
    println!(
        "cargo:rerun-if-changed={}",
        governance_contract_root.display()
    );
    let mut governance_contract_resources = BTreeMap::new();
    collect_resources(
        &governance_contract_root,
        &governance_contract_root,
        &mut governance_contract_resources,
    )
    .and_then(|_| validate_governance_contract_set(&governance_contract_resources))
    .map(|validated| {
        println!(
            "cargo:rustc-env=NOVELGAINKER_AGENT_GOVERNANCE_CONTRACT_SET_SHA256={}",
            validated.contract_set_hash
        );
        println!(
            "cargo:rustc-env=NOVELGAINKER_AGENT_GOVERNANCE_CONTRACT_RESOURCE_COUNT={}",
            validated.resource_count
        );
    })
    .unwrap_or_else(|error| {
        panic!(
            "NovelGainker Agent Authorization/Persistence/Transaction/Undo v1 validation failed: {error}"
        )
    });
}
