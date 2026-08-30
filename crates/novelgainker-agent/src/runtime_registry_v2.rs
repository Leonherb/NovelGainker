use crate::runtime_registry::{runtime_skill, RuntimeSkillDescriptorV1};
use crate::v2::{AgentV2ContractError, SkillSelectionV2};
use crate::{SKILL_IDS, SKILL_PACK_SHA256, SKILL_PACK_SOURCE_COMMIT, SKILL_PACK_VERSION};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt::Write as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundledSkillResourceKindV2 {
    Instruction,
    Template,
    Reference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BundledSkillResourceV2 {
    pub path: &'static str,
    pub kind: BundledSkillResourceKindV2,
    pub content: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSkillDescriptorV2 {
    pub id: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub trigger_summary: &'static str,
    pub manifest_source: &'static str,
    pub resources: Vec<BundledSkillResourceV2>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSkillRegistryV2 {
    pub schema_version: u16,
    pub pack_id: &'static str,
    pub pack_version: &'static str,
    pub pack_hash: &'static str,
    pub source_commit: &'static str,
    pub runtime_source: &'static str,
    pub eligible_skill_ids: Vec<&'static str>,
    pub skills: Vec<RuntimeSkillDescriptorV2>,
}

impl RuntimeSkillRegistryV2 {
    pub fn load_gar1_read_only() -> Self {
        let skills = SKILL_IDS
            .iter()
            .map(|id| descriptor(id).expect("locked Skill ID must have a V2 descriptor"))
            .collect::<Vec<_>>();
        Self {
            schema_version: 2,
            pack_id: crate::SKILL_PACK_ID,
            pack_version: SKILL_PACK_VERSION,
            pack_hash: SKILL_PACK_SHA256,
            source_commit: SKILL_PACK_SOURCE_COMMIT,
            runtime_source: "bundled_release_only",
            eligible_skill_ids: SKILL_IDS.to_vec(),
            skills,
        }
    }

    pub fn validate_selection(
        &self,
        selection: &SkillSelectionV2,
        expected_request_id: &str,
        project_context_requested: bool,
    ) -> Result<(), AgentV2ContractError> {
        selection.validate(expected_request_id)?;
        let eligible = self
            .eligible_skill_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if selection
            .selected_skill_ids
            .iter()
            .any(|id| !eligible.contains(id.as_str()))
        {
            return Err(AgentV2ContractError::InvalidSkillSelection);
        }
        if project_context_requested && selection.selected_skill_ids.is_empty() {
            return Err(AgentV2ContractError::InvalidSkillSelection);
        }
        Ok(())
    }

    pub fn selected_provider_bundle(
        &self,
        selected_ids: &[String],
    ) -> Result<String, AgentV2ContractError> {
        if selected_ids.len() > crate::v2::MAX_SELECTED_SKILLS_V2 {
            return Err(AgentV2ContractError::InvalidSkillSelection);
        }
        let mut seen = BTreeSet::new();
        let mut output = String::new();
        for selected_id in selected_ids {
            if !seen.insert(selected_id.as_str()) {
                return Err(AgentV2ContractError::InvalidSkillSelection);
            }
            let skill = self
                .skills
                .iter()
                .find(|skill| skill.id == selected_id)
                .ok_or(AgentV2ContractError::InvalidSkillSelection)?;
            writeln!(&mut output, "## Skill: {}", skill.id).expect("writing to String cannot fail");
            for resource in &skill.resources {
                let label = match resource.kind {
                    BundledSkillResourceKindV2::Instruction => "instruction",
                    BundledSkillResourceKindV2::Template => "template",
                    BundledSkillResourceKindV2::Reference => "reference",
                };
                writeln!(&mut output, "\n### {label}: {}", resource.path)
                    .expect("writing to String cannot fail");
                output.push_str(resource.content);
                if !resource.content.ends_with('\n') {
                    output.push('\n');
                }
            }
        }
        Ok(output)
    }

    pub fn resource_hashes(&self) -> Vec<(String, String)> {
        let mut resources = self
            .skills
            .iter()
            .flat_map(|skill| {
                let manifest_path = format!("manifests/{}.json", skill.id);
                std::iter::once((manifest_path, sha256_hex(skill.manifest_source.as_bytes())))
                    .chain(skill.resources.iter().map(|resource| {
                        (
                            resource.path.to_owned(),
                            sha256_hex(resource.content.as_bytes()),
                        )
                    }))
            })
            .collect::<Vec<_>>();
        resources.sort();
        resources.dedup();
        resources
    }
}

fn descriptor(id: &str) -> Option<RuntimeSkillDescriptorV2> {
    let base = runtime_skill(id)?;
    Some(RuntimeSkillDescriptorV2 {
        id: base.id,
        display_name: base.display_name,
        description: base.description,
        trigger_summary: base.trigger_summary,
        manifest_source: manifest_source(id)?,
        resources: resources(id)?,
    })
}

fn manifest_source(id: &str) -> Option<&'static str> {
    Some(match id {
        "create-novel-project" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../resources/skills/novelist/v1/manifests/create-novel-project.json"
        )),
        "create-setting" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../resources/skills/novelist/v1/manifests/create-setting.json"
        )),
        "create-character" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../resources/skills/novelist/v1/manifests/create-character.json"
        )),
        "create-material" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../resources/skills/novelist/v1/manifests/create-material.json"
        )),
        "create-plot" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../resources/skills/novelist/v1/manifests/create-plot.json"
        )),
        "novel-story-telling" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../resources/skills/novelist/v1/manifests/novel-story-telling.json"
        )),
        "analytical-review" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../resources/skills/novelist/v1/manifests/analytical-review.json"
        )),
        "create-visual-asset" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../resources/skills/novelist/v1/manifests/create-visual-asset.json"
        )),
        "publish-novel" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../resources/skills/novelist/v1/manifests/publish-novel.json"
        )),
        _ => return None,
    })
}

macro_rules! resource {
    ($path:literal, $kind:ident) => {
        BundledSkillResourceV2 {
            path: $path,
            kind: BundledSkillResourceKindV2::$kind,
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../resources/skills/novelist/v1/",
                $path
            )),
        }
    };
}

fn instruction(
    base: &'static RuntimeSkillDescriptorV1,
    path: &'static str,
) -> BundledSkillResourceV2 {
    BundledSkillResourceV2 {
        path,
        kind: BundledSkillResourceKindV2::Instruction,
        content: base.instruction,
    }
}

fn resources(id: &str) -> Option<Vec<BundledSkillResourceV2>> {
    let base = runtime_skill(id)?;
    let resources = match id {
        "create-novel-project" => vec![
            instruction(base, "instructions/create-novel-project.md"),
            resource!("fixtures/chapter.template.md", Template),
            resource!("fixtures/character.template.md", Template),
            resource!("fixtures/material.template.md", Template),
            resource!("fixtures/plot.template.md", Template),
            resource!("fixtures/project.template.md", Template),
            resource!("fixtures/setting.template.md", Template),
        ],
        "create-setting" => vec![
            instruction(base, "instructions/create-setting.md"),
            resource!("fixtures/setting.template.md", Template),
        ],
        "create-character" => vec![
            instruction(base, "instructions/create-character.md"),
            resource!("fixtures/character.template.md", Template),
        ],
        "create-material" => vec![
            instruction(base, "instructions/create-material.md"),
            resource!("fixtures/material.template.md", Template),
        ],
        "create-plot" => vec![
            instruction(base, "instructions/create-plot.md"),
            resource!("fixtures/plot.template.md", Template),
        ],
        "novel-story-telling" => vec![
            instruction(base, "instructions/novel-story-telling.md"),
            resource!("fixtures/chapter.template.md", Template),
            resource!("references/storytelling/drama.md", Reference),
            resource!("references/storytelling/mystery.md", Reference),
            resource!(
                "references/storytelling/romantic-entanglement.md",
                Reference
            ),
            resource!("references/storytelling/science-fiction.md", Reference),
            resource!("references/storytelling/story-flow.md", Reference),
            resource!("references/storytelling/time-travel-and-loop.md", Reference),
            resource!("references/storytelling/wuxia.md", Reference),
        ],
        "analytical-review" => vec![
            instruction(base, "instructions/analytical-review.md"),
            resource!("references/analytical-review-method.zh-CN.md", Reference),
        ],
        "create-visual-asset" => vec![instruction(base, "instructions/create-visual-asset.md")],
        "publish-novel" => vec![instruction(base, "instructions/publish-novel.md")],
        _ => return None,
    };
    Some(resources)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v2::SkillSelectionV2;

    #[test]
    fn loads_all_nine_bundled_skills_with_complete_declared_resources() {
        let registry = RuntimeSkillRegistryV2::load_gar1_read_only();
        assert_eq!(registry.eligible_skill_ids, SKILL_IDS);
        assert_eq!(registry.skills.len(), 9);
        assert_eq!(registry.runtime_source, "bundled_release_only");
        assert!(registry.skills.iter().all(|skill| {
            !skill.manifest_source.is_empty()
                && skill
                    .resources
                    .iter()
                    .any(|resource| resource.kind == BundledSkillResourceKindV2::Instruction)
        }));
        let storytelling = registry
            .skills
            .iter()
            .find(|skill| skill.id == "novel-story-telling")
            .expect("storytelling skill");
        assert_eq!(
            storytelling
                .resources
                .iter()
                .filter(|resource| resource.kind == BundledSkillResourceKindV2::Reference)
                .count(),
            7
        );
    }

    #[test]
    fn eligibility_is_complete_and_never_keyword_filtered() {
        let first = RuntimeSkillRegistryV2::load_gar1_read_only();
        let second = RuntimeSkillRegistryV2::load_gar1_read_only();
        assert_eq!(first.eligible_skill_ids, second.eligible_skill_ids);
        assert_eq!(first.eligible_skill_ids.len(), 9);
    }

    #[test]
    fn selection_is_request_and_eligibility_bound_and_zero_skill_cannot_read_project() {
        let registry = RuntimeSkillRegistryV2::load_gar1_read_only();
        let selection = SkillSelectionV2 {
            schema_version: 2,
            request_id: "req_12345678".to_owned(),
            selected_skill_ids: vec!["create-character".to_owned()],
            intent_class: crate::v2::SkillIntentClassV2::ProjectDiscussion,
            visible_reason: "需要读取人物资料。".to_owned(),
        };
        assert_eq!(
            registry.validate_selection(&selection, "req_12345678", true),
            Ok(())
        );
        let empty = SkillSelectionV2 {
            selected_skill_ids: Vec::new(),
            ..selection
        };
        assert_eq!(
            registry.validate_selection(&empty, "req_12345678", true),
            Err(AgentV2ContractError::InvalidSkillSelection)
        );
        assert_eq!(
            registry.validate_selection(&empty, "req_12345678", false),
            Ok(())
        );
    }

    #[test]
    fn provider_bundle_contains_instruction_templates_and_references_without_paths_or_tools() {
        let registry = RuntimeSkillRegistryV2::load_gar1_read_only();
        let bundle = registry
            .selected_provider_bundle(&[
                "create-character".to_owned(),
                "novel-story-telling".to_owned(),
            ])
            .expect("bundle must render");
        assert!(bundle.contains("instructions/create-character.md"));
        assert!(bundle.contains("fixtures/character.template.md"));
        assert!(bundle.contains("references/storytelling/story-flow.md"));
        assert!(!bundle.contains("C:\\Users"));
        assert!(!bundle.contains("plugins/cache"));
    }
}
