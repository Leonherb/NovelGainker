//! Domain contracts for NovelGainker projects.

use serde::{Deserialize, Serialize};

pub const DOMAIN_CONTRACT_VERSION: u16 = 1;
pub const PROJECT_SCHEMA_VERSION: u16 = 1;
pub const WORKFLOW_PACK_VERSION: &str = "1.0.0";
pub const DEFAULT_PROVIDER_ID: &str = "openai";
pub const DEFAULT_WRITING_LANGUAGE: &str = "zh-CN";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectMetadata {
    pub schema_version: u16,
    pub project_id: String,
    pub workflow_pack_version: String,
    pub default_provider_id: String,
    pub writing_language: String,
    pub strict_chapter_markup: bool,
}

impl ProjectMetadata {
    pub fn v1(project_id: impl Into<String>) -> Self {
        Self {
            schema_version: PROJECT_SCHEMA_VERSION,
            project_id: project_id.into(),
            workflow_pack_version: WORKFLOW_PACK_VERSION.to_owned(),
            default_provider_id: DEFAULT_PROVIDER_ID.to_owned(),
            writing_language: DEFAULT_WRITING_LANGUAGE.to_owned(),
            strict_chapter_markup: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ProjectMetadata, DEFAULT_PROVIDER_ID, DEFAULT_WRITING_LANGUAGE};

    #[test]
    fn v1_metadata_uses_the_approved_portable_defaults() {
        let metadata = ProjectMetadata::v1("project-id");

        assert_eq!(metadata.schema_version, 1);
        assert_eq!(metadata.default_provider_id, DEFAULT_PROVIDER_ID);
        assert_eq!(metadata.writing_language, DEFAULT_WRITING_LANGUAGE);
        assert!(metadata.strict_chapter_markup);
    }
}
