#[path = "src/pack_validation.rs"]
mod pack_validation;

use pack_validation::validate_pack;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn collect_resources(
    root: &Path,
    directory: &Path,
    resources: &mut BTreeMap<String, Vec<u8>>,
) -> Result<(), String> {
    let entries = fs::read_dir(directory)
        .map_err(|_| "workflow-pack.resource-directory-unreadable".to_owned())?;
    for entry in entries {
        let entry = entry.map_err(|_| "workflow-pack.resource-entry-unreadable".to_owned())?;
        let file_type = entry
            .file_type()
            .map_err(|_| "workflow-pack.resource-type-unreadable".to_owned())?;
        let path = entry.path();
        if file_type.is_symlink() {
            return Err("workflow-pack.resource-symlink-forbidden".to_owned());
        }
        if file_type.is_dir() {
            collect_resources(root, &path, resources)?;
            continue;
        }
        if !file_type.is_file() {
            return Err("workflow-pack.resource-type-forbidden".to_owned());
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| "workflow-pack.resource-path-invalid".to_owned())?;
        let relative_path = relative
            .components()
            .map(|component| {
                component
                    .as_os_str()
                    .to_str()
                    .ok_or_else(|| "workflow-pack.resource-path-non-utf8".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?
            .join("/");
        let bytes = fs::read(&path)
            .map_err(|_| format!("workflow-pack.resource-unreadable:{relative_path}"))?;
        if resources.insert(relative_path, bytes).is_some() {
            return Err("workflow-pack.resource-path-duplicate".to_owned());
        }
        println!("cargo:rerun-if-changed={}", path.display());
    }
    Ok(())
}

fn main() {
    let manifest_dir = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be available"),
    );
    let resource_root = manifest_dir.join("../../resources/workflows/v1");
    println!("cargo:rerun-if-changed={}", resource_root.display());

    let mut resources = BTreeMap::new();
    collect_resources(&resource_root, &resource_root, &mut resources)
        .and_then(|_| validate_pack(&resources))
        .map(|validated| {
            let character = validated
                .workflows
                .iter()
                .find(|workflow| workflow.id == "novelgainker.character-create")
                .expect("validated pack must contain character-create");
            let project = validated
                .workflows
                .iter()
                .find(|workflow| workflow.id == "novelgainker.project-create")
                .expect("validated pack must contain project-create");
            let setting = validated
                .workflows
                .iter()
                .find(|workflow| workflow.id == "novelgainker.setting-create")
                .expect("validated pack must contain setting-create");
            let material = validated
                .workflows
                .iter()
                .find(|workflow| workflow.id == "novelgainker.material-create")
                .expect("validated pack must contain material-create");
            let plot = validated
                .workflows
                .iter()
                .find(|workflow| workflow.id == "novelgainker.plot-create")
                .expect("validated pack must contain plot-create");
            let storytelling = validated
                .workflows
                .iter()
                .find(|workflow| workflow.id == "novelgainker.storytelling")
                .expect("validated pack must contain storytelling");
            println!(
                "cargo:rustc-env=NOVELGAINKER_WORKFLOW_PACK_SHA256={}",
                validated.pack_hash
            );
            println!(
                "cargo:rustc-env=NOVELGAINKER_CHARACTER_WORKFLOW_SHA256={}",
                character.content_hash
            );
            println!(
                "cargo:rustc-env=NOVELGAINKER_PROJECT_WORKFLOW_SHA256={}",
                project.content_hash
            );
            println!(
                "cargo:rustc-env=NOVELGAINKER_SETTING_WORKFLOW_SHA256={}",
                setting.content_hash
            );
            println!(
                "cargo:rustc-env=NOVELGAINKER_MATERIAL_WORKFLOW_SHA256={}",
                material.content_hash
            );
            println!(
                "cargo:rustc-env=NOVELGAINKER_PLOT_WORKFLOW_SHA256={}",
                plot.content_hash
            );
            println!(
                "cargo:rustc-env=NOVELGAINKER_STORYTELLING_WORKFLOW_SHA256={}",
                storytelling.content_hash
            );
        })
        .unwrap_or_else(|error| panic!("NovelGainker Workflow Pack v1 validation failed: {error}"));
}
