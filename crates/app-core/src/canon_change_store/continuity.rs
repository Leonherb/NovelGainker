use super::*;
use crate::continuity::ChapterDependency;
use serde_json::json;

impl CanonChangeCoordinator {
    /// Called inside read_snapshot's writer coordination. Sources are issued
    /// only for the complete bytes in this successful delivered result.
    pub(crate) fn annotate_chapter_sources(
        &self,
        evidence: &mut ChangeEvidence,
        output: &mut Value,
    ) -> Result<(), ChangeError> {
        if !self.nr5 {
            return Ok(());
        }
        self.verify_lease()?;
        let records = self.records()?;
        ensure_recovered(&records)?;
        let targets = output["changeEvidence"]["completeTargets"]
            .as_array()
            .ok_or(ChangeError::IncompleteRead)?;
        let mut next = evidence.clone();
        let mut sources = Vec::new();
        for target in targets {
            if target["entityKind"] != "chapter" {
                continue;
            }
            let read_id = target["readReceiptId"]
                .as_str()
                .ok_or(ChangeError::IncompleteRead)?;
            let path = target["relativePath"]
                .as_str()
                .ok_or(ChangeError::IncompleteRead)?;
            let read = evidence
                .chapter_read(read_id)
                .ok_or(ChangeError::IncompleteRead)?;
            next.remove_chapter_source(path);
            let latest = records
                .iter()
                .filter(|record| {
                    matches!(
                        record.receipt.status,
                        ChangeStatus::Committed | ChangeStatus::Undone
                    ) && record.receipt.files[0].relative_path == path
                        && record
                            .chapter
                            .as_ref()
                            .is_some_and(|chapter| chapter.identity == read.identity)
                })
                .max_by_key(|record| {
                    record
                        .chapter
                        .as_ref()
                        .map(|chapter| chapter.generation)
                        .unwrap_or(0)
                });
            let source = latest
                .and_then(|record| {
                    self.chapter_commit_receipt_from_records(&record.receipt.change_id, &records)
                        .ok()
                })
                .filter(|receipt| receipt.after_hash == read.content_hash)
                .map(|receipt| ChapterDependency {
                    project_id: self.project_id().to_owned(),
                    transaction_id: receipt.transaction_id,
                    revision: receipt.revision,
                    identity: receipt.identity,
                    relative_path: path.to_owned(),
                    after_hash: receipt.after_hash,
                    read_receipt_id: read_id.to_owned(),
                });
            if let Some(source) = source {
                next.bind_chapter_source(source.clone())?;
                sources.push(json!({"status":"current","source":source}));
            } else {
                sources.push(
                    json!({"status":"unavailable","relativePath":path,"readReceiptId":read_id,
                    "notice":"已实际读取，但没有此版本的有效章节提交凭证；不可冒充正式章节来源。"}),
                );
            }
        }
        let mut result = output.clone();
        result["chapterSources"] = json!(sources);
        if serde_json::to_vec(&result)
            .map_err(|_| ChangeError::InvalidRequest)?
            .len()
            > self.protocol_profile().result_bytes()
        {
            return Err(ChangeError::BudgetExceeded);
        }
        *evidence = next;
        *output = result;
        Ok(())
    }

    pub(super) fn validate_chapter_dependencies(
        &self,
        record: &ChangeRecord,
    ) -> Result<(), ChangeError> {
        if record.chapter_dependencies.is_empty() {
            return Ok(());
        }
        if !self.nr5 || record.source_kind != ChangeSourceKind::ChapterCommit {
            return Err(ChangeError::SourceInvalid);
        }
        let records = self.records()?;
        self.validate_dependencies_from_records(record, &records)
    }

    fn validate_dependencies_from_records(
        &self,
        record: &ChangeRecord,
        records: &[ChangeRecord],
    ) -> Result<(), ChangeError> {
        if records.iter().any(|other| {
            other.receipt.change_id != record.receipt.change_id
                && matches!(
                    other.receipt.status,
                    ChangeStatus::Prepared
                        | ChangeStatus::Applying
                        | ChangeStatus::RecoveryRequired
                )
        }) {
            return Err(ChangeError::RecoveryRequired);
        }
        for source in &record.chapter_dependencies {
            source.validate()?;
            let receipt =
                self.chapter_commit_receipt_from_records(&source.transaction_id, records)?;
            let original = records
                .iter()
                .find(|record| record.receipt.change_id == source.transaction_id)
                .ok_or(ChangeError::SourceInvalid)?;
            if source.project_id != self.project_id()
                || receipt.identity != source.identity
                || receipt.revision != source.revision
                || receipt.after_hash != source.after_hash
                || original.receipt.files[0].relative_path != source.relative_path
            {
                return Err(ChangeError::SourceChanged);
            }
        }
        Ok(())
    }

    pub(super) fn activity_with_source_state(
        &self,
        record: &ChangeRecord,
        records: &[ChangeRecord],
    ) -> ChangeActivity {
        use crate::continuity::ChapterSourceState;
        let chapter_source_state = if record.chapter_dependencies.is_empty() {
            None
        } else {
            Some(
                match self.validate_dependencies_from_records(record, records) {
                    Ok(()) => ChapterSourceState::Current,
                    Err(ChangeError::SourceChanged) => ChapterSourceState::Stale,
                    Err(_) => ChapterSourceState::Unavailable,
                },
            )
        };
        ChangeActivity {
            conversation_session_id: record.conversation_session_id.clone(),
            turn_id: record.turn_id.clone(),
            receipt: record.receipt.clone(),
            chapter_source_state,
        }
    }
}
