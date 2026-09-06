use super::*;
use crate::chapter_contract::{ChapterDocument, ChapterStatus};
use crate::publication::{
    self, assets, epub, markdown, store, BookChapter, BookMetadata, BookSnapshot,
    PublicationAction, PublicationDetail, PublicationIssue as Issue, PublicationPage,
    PublicationReceipt, PublicationRequest, PublicationStatus,
};
use store::{InputReference, Record, Stage};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::canon_change_store) enum PublicationCheckpoint {
    Snapshot,
    Built,
    Promoted,
}

impl CanonChangeCoordinator {
    pub fn publish(
        &self,
        evidence: &ChangeEvidence,
        request: &PublicationRequest,
        context: &crate::tool_runtime::ToolExecutionContext,
        cancellation: &CancellationToken,
    ) -> Result<PublicationReceipt, Issue> {
        self.publish_with_hook(evidence, request, context, cancellation, |_| Ok(()))
    }

    pub(in crate::canon_change_store) fn publish_with_hook(
        &self,
        evidence: &ChangeEvidence,
        request: &PublicationRequest,
        context: &crate::tool_runtime::ToolExecutionContext,
        cancellation: &CancellationToken,
        mut hook: impl FnMut(PublicationCheckpoint) -> Result<(), Issue>,
    ) -> Result<PublicationReceipt, Issue> {
        let state = self.state.try_lock().map_err(|_| Issue::StoreUnavailable)?;
        self.verify_lease().map_err(|_| Issue::StoreUnavailable)?;
        if !self.nr5
            || !evidence.matches_context(
                self.project_id(),
                &context.conversation_session_id,
                &context.turn_id,
            )
            || context.project_id != self.project_id()
            || !evidence.has_publication_method(&request.method_receipt_id)
            || !evidence.current_user_source(&request.source)
        {
            return Err(Issue::SourceRequired);
        }
        let conversation = read_conversation(&self.handle).map_err(|_| Issue::SourceRequired)?;
        if conversation.session_id != context.conversation_session_id
            || conversation
                .turns
                .last()
                .is_none_or(|turn| turn.turn_id != context.turn_id)
            || !ChangeEvidence::from_conversation(&conversation, &context.turn_id)
                .map_err(|_| Issue::SourceRequired)?
                .current_user_source(&request.source)
        {
            return Err(Issue::SourceRequired);
        }
        let request_hash =
            sha256_hex(&serde_json::to_vec(request).map_err(|_| Issue::SourceRequired)?);
        let existing = store::load_all(&self.handle)?;
        for mut record in existing.iter().cloned() {
            if record.receipt.session_id == context.conversation_session_id
                && record.receipt.turn_id == context.turn_id
            {
                if record.call_id != context.call_id || record.request_hash != request_hash {
                    return Err(Issue::LimitExceeded);
                }
                self.reconcile_publication(&mut record)?;
                return Ok(record.receipt);
            }
        }
        if existing.len() >= publication::MAX_PUBLICATION_RECORDS {
            return Err(Issue::LimitExceeded);
        }
        store::capacity(
            &self.handle,
            publication::MAX_PUBLICATION_OUTPUT + 3 * publication::MAX_PUBLICATION_RECORD_BYTES,
        )?;
        let mut record = Record {
            created_at_ms: u64::try_from(chrono::Utc::now().timestamp_millis())
                .map_err(|_| Issue::StoreUnavailable)?,
            project_id: self.project_id().to_owned(),
            call_id: context.call_id.clone(),
            request_hash,
            method_receipt_id: request.method_receipt_id.clone(),
            source: request.source.clone(),
            action: request.action,
            stage: Stage::Building,
            inputs: Vec::new(),
            receipt: PublicationReceipt {
                schema_version: 1,
                artifact_id: Uuid::new_v4().to_string(),
                session_id: context.conversation_session_id.clone(),
                turn_id: context.turn_id.clone(),
                status: PublicationStatus::ValidationFailed,
                chapter_count: 0,
                skipped_drafts: 0,
                without_cover: true,
                relative_path: None,
                output_hash: None,
                size_bytes: None,
                issue: Some(Issue::StoreUnavailable),
            },
        };
        // Intent exists before any artifact write; replay never executes a second build.
        store::save(&self.handle, &record)?;
        let result = (|| {
            publication_cancel(cancellation)?;
            let snapshot = self.publication_snapshot(&mut record, cancellation)?;
            if record.inputs.iter().any(|input| {
                state
                    .buffers
                    .get(&input.relative_path)
                    .is_some_and(|(_, dirty)| *dirty)
            }) {
                return Err(Issue::SourceChanged);
            }
            store::save(&self.handle, &record)?;
            hook(PublicationCheckpoint::Snapshot)?;
            let modified = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
            if request.action == PublicationAction::Preflight {
                epub::build(
                    &snapshot,
                    &record.receipt.artifact_id,
                    &modified,
                    std::io::sink(),
                    || publication_cancel(cancellation),
                )?;
                self.recheck_publication_inputs(&record)?;
                record.receipt.status = PublicationStatus::PreflightPassed;
                record.receipt.issue = None;
                return Ok(());
            }
            let temporary_relative = format!("published/.build-{}.tmp", record.receipt.artifact_id);
            let temporary = store::target(&self.handle, &temporary_relative)?;
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)
                .map_err(|_| Issue::StoreUnavailable)?;
            let built = epub::build(
                &snapshot,
                &record.receipt.artifact_id,
                &modified,
                &mut file,
                || publication_cancel(cancellation),
            )?;
            file.sync_all().map_err(|_| Issue::StoreUnavailable)?;
            drop(file);
            hook(PublicationCheckpoint::Built)?;
            if built.entries != 4 + snapshot.chapters.len() + snapshot.assets.len() {
                return Err(Issue::LocalValidationFailed);
            }
            let (actual_hash, actual_bytes) = store::file_hash(
                &self.handle,
                &temporary_relative,
                publication::MAX_PUBLICATION_OUTPUT,
            )?;
            if actual_hash != built.hash || actual_bytes != built.bytes {
                return Err(Issue::LocalValidationFailed);
            }
            self.recheck_publication_inputs(&record)?;
            publication_cancel(cancellation)?;
            let relative = format!("published/{}.epub", record.receipt.artifact_id);
            record.receipt.relative_path = Some(relative.clone());
            record.receipt.output_hash = Some(built.hash);
            record.receipt.size_bytes = Some(built.bytes);
            record.stage = Stage::ReadyToPromote;
            store::save(&self.handle, &record)?;
            let target = store::target(&self.handle, &relative)?;
            if store::target(&self.handle, &temporary_relative)? != temporary {
                return Err(Issue::UnsafePath);
            }
            self.verify_lease().map_err(|_| Issue::StoreUnavailable)?;
            publication_cancel(cancellation)?;
            store::promote(&temporary, &target)?;
            hook(PublicationCheckpoint::Promoted)?;
            // Last current-source check occurs after promotion as well. A changed
            // snapshot is preserved as stale, never presented as the current book.
            self.recheck_publication_inputs(&record)?;
            record.receipt.status = PublicationStatus::Generated;
            record.receipt.issue = None;
            Ok(())
        })();
        if let Err(issue) = result {
            record.receipt.status = if issue == Issue::SourceChanged {
                PublicationStatus::Stale
            } else {
                PublicationStatus::ValidationFailed
            };
            record.receipt.issue = Some(issue);
            // Only an actual matching final artifact can remain in a public receipt.
            if !self.publication_output_matches(&record) {
                record.receipt.relative_path = None;
                record.receipt.output_hash = None;
                record.receipt.size_bytes = None;
            }
        }
        record.stage = Stage::Terminal;
        store::save(&self.handle, &record)?;
        Ok(record.receipt)
    }

    fn publication_snapshot(
        &self,
        record: &mut Record,
        cancellation: &CancellationToken,
    ) -> Result<BookSnapshot, Issue> {
        let changes = self.records().map_err(|_| Issue::StoreUnavailable)?;
        ensure_recovered(&changes).map_err(|_| Issue::StoreUnavailable)?;
        // Observing an external edit must invalidate old final authority even
        // when its new status is draft or malformed and is not selected below.
        for change in &changes {
            if change.receipt.status == ChangeStatus::Committed
                && change
                    .chapter
                    .as_ref()
                    .is_some_and(|c| c.final_confirmed && !c.final_invalidated)
            {
                let _ =
                    self.chapter_commit_receipt_from_records(&change.receipt.change_id, &changes);
            }
        }
        let metadata_bytes = read_bounded(&store::target(&self.handle, "project.md")?, 32 * 1024)
            .map_err(|_| Issue::MetadataRequired)?;
        let metadata = BookMetadata::parse(&metadata_bytes)?;
        record.inputs.push(reference("project.md", &metadata_bytes));
        record.receipt.without_cover = metadata.cover.is_none();
        let mut chapters = Vec::new();
        let mut identities = Vec::new();
        let mut asset_paths = std::collections::BTreeSet::new();
        if let Some(cover) = &metadata.cover {
            asset_paths.insert(cover.clone());
        }
        let mut text_size = 0usize;
        for path in store::chapter_paths(&self.handle)? {
            publication_cancel(cancellation)?;
            let bytes = read_bounded(&store::target(&self.handle, &path)?, 32 * 1024)
                .map_err(|_| Issue::InvalidChapter)?;
            text_size += bytes.len();
            if text_size > publication::MAX_PUBLICATION_TEXT {
                return Err(Issue::LimitExceeded);
            }
            let document =
                ChapterDocument::parse(&path, &bytes).map_err(|_| Issue::InvalidChapter)?;
            identities.push(document.identity.clone());
            let mut input = reference(&path, &bytes);
            if document.status == ChapterStatus::Final {
                let change = changes
                    .iter()
                    .filter(|change| {
                        change
                            .chapter
                            .as_ref()
                            .is_some_and(|c| c.identity == document.identity)
                    })
                    .filter(|change| {
                        matches!(
                            change.receipt.status,
                            ChangeStatus::Committed | ChangeStatus::Undone
                        )
                    })
                    .max_by_key(|change| change.chapter.as_ref().map(|c| c.generation).unwrap_or(0))
                    .ok_or(Issue::FinalConfirmationRequired)?;
                if !change
                    .chapter
                    .as_ref()
                    .is_some_and(|c| c.final_confirmed && !c.final_invalidated)
                    || change
                        .receipt
                        .files
                        .first()
                        .is_none_or(|file| file.relative_path != path)
                {
                    return Err(Issue::FinalConfirmationRequired);
                }
                let proof = self
                    .chapter_commit_receipt_from_records(&change.receipt.change_id, &changes)
                    .map_err(|_| Issue::FinalConfirmationRequired)?;
                if proof.after_hash != input.content_hash || proof.identity != document.identity {
                    return Err(Issue::FinalConfirmationRequired);
                }
                input.confirmation_id = Some(proof.transaction_id);
                input.confirmation_revision = Some(proof.revision);
                let body = markdown::draft(&bytes)?;
                asset_paths.extend(markdown::asset_references(&body)?);
                chapters.push(BookChapter {
                    number: document.identity.chapter_number,
                    title: document.title,
                    body,
                });
            } else {
                record.receipt.skipped_drafts += 1;
            }
            record.inputs.push(input);
        }
        crate::chapter_contract::validate_chapter_inventory(&identities)
            .map_err(|_| Issue::InvalidChapter)?;
        if chapters.is_empty() {
            return Err(Issue::NoFinalChapters);
        }
        chapters.sort_by_key(|chapter| chapter.number);
        record.receipt.chapter_count = chapters.len() as u16;
        if asset_paths.len() > publication::MAX_PUBLICATION_ASSETS {
            return Err(Issue::LimitExceeded);
        }
        let mut assets = BTreeMap::new();
        let mut asset_size = 0usize;
        for path in asset_paths {
            publication_cancel(cancellation)?;
            let bytes = read_bounded(&store::target(&self.handle, &path)?, 16 * 1024 * 1024)
                .map_err(|_| Issue::InvalidAsset)?;
            asset_size += bytes.len();
            if asset_size > publication::MAX_PUBLICATION_ASSET_BYTES {
                return Err(Issue::LimitExceeded);
            }
            let input = reference(&path, &bytes);
            assets.insert(path.clone(), assets::validate(&path, bytes)?);
            record.inputs.push(input);
        }
        Ok(BookSnapshot {
            metadata,
            chapters,
            assets,
        })
    }

    fn recheck_publication_inputs(&self, record: &Record) -> Result<(), Issue> {
        self.verify_lease().map_err(|_| Issue::StoreUnavailable)?;
        let changes = self.records().map_err(|_| Issue::StoreUnavailable)?;
        ensure_recovered(&changes).map_err(|_| Issue::StoreUnavailable)?;
        let mut expected = record
            .inputs
            .iter()
            .filter(|input| input.relative_path.starts_with("chapters/"))
            .map(|input| input.relative_path.clone())
            .collect::<Vec<_>>();
        expected.sort();
        if store::chapter_paths(&self.handle).map_err(|_| Issue::SourceChanged)? != expected {
            return Err(Issue::SourceChanged);
        }
        for input in &record.inputs {
            if let Some(id) = &input.confirmation_id {
                let change = changes
                    .iter()
                    .find(|change| &change.receipt.change_id == id)
                    .ok_or(Issue::SourceChanged)?;
                if !change
                    .chapter
                    .as_ref()
                    .is_some_and(|c| c.final_confirmed && !c.final_invalidated)
                    || change
                        .receipt
                        .files
                        .first()
                        .is_none_or(|file| file.relative_path != input.relative_path)
                {
                    return Err(Issue::SourceChanged);
                }
                let proof = self
                    .chapter_commit_receipt_from_records(id, &changes)
                    .map_err(|_| Issue::SourceChanged)?;
                if proof.after_hash != input.content_hash
                    || Some(proof.revision) != input.confirmation_revision
                {
                    return Err(Issue::SourceChanged);
                }
            }
            let (hash, size) =
                store::file_hash(&self.handle, &input.relative_path, input.size_bytes)
                    .map_err(|_| Issue::SourceChanged)?;
            if hash != input.content_hash || size != input.size_bytes {
                return Err(Issue::SourceChanged);
            }
        }
        Ok(())
    }

    fn publication_output_matches(&self, record: &Record) -> bool {
        record.receipt.relative_path.as_ref().is_some_and(|path| {
            store::file_hash(&self.handle, path, publication::MAX_PUBLICATION_OUTPUT).is_ok_and(
                |(hash, size)| {
                    Some(&hash) == record.receipt.output_hash.as_ref()
                        && Some(size) == record.receipt.size_bytes
                },
            )
        })
    }

    fn reconcile_publication(&self, record: &mut Record) -> Result<(), Issue> {
        let before = serde_json::to_vec(record).map_err(|_| Issue::StoreUnavailable)?;
        if record.stage != Stage::Terminal {
            if record.stage == Stage::ReadyToPromote && self.publication_output_matches(record) {
                record.receipt.status = PublicationStatus::Generated;
                record.receipt.issue = None;
            } else {
                record.receipt.status = PublicationStatus::ValidationFailed;
                record.receipt.issue = Some(Issue::StoreUnavailable);
                record.receipt.relative_path = None;
                record.receipt.output_hash = None;
                record.receipt.size_bytes = None;
            }
            record.stage = Stage::Terminal;
        }
        if matches!(
            record.receipt.status,
            PublicationStatus::Generated | PublicationStatus::PreflightPassed
        ) {
            if self.recheck_publication_inputs(record).is_err() {
                record.receipt.status = PublicationStatus::Stale;
                record.receipt.issue = Some(Issue::SourceChanged);
            } else if record.receipt.status == PublicationStatus::Generated
                && !self.publication_output_matches(record)
            {
                record.receipt.status = PublicationStatus::ValidationFailed;
                record.receipt.issue = Some(Issue::LocalValidationFailed);
            }
        }
        if before != serde_json::to_vec(record).map_err(|_| Issue::StoreUnavailable)? {
            store::save(&self.handle, record)?;
        }
        Ok(())
    }

    pub fn publication_list(&self, cursor: Option<&str>) -> Result<PublicationPage, Issue> {
        let _state = self.state.try_lock().map_err(|_| Issue::StoreUnavailable)?;
        self.verify_lease().map_err(|_| Issue::StoreUnavailable)?;
        let mut records = store::load_all(&self.handle)?;
        for record in &mut records {
            self.reconcile_publication(record)?;
        }
        store::page(&records, cursor)
    }

    pub fn publication_detail(&self, artifact_id: &str) -> Result<PublicationDetail, Issue> {
        let _state = self.state.try_lock().map_err(|_| Issue::StoreUnavailable)?;
        self.verify_lease().map_err(|_| Issue::StoreUnavailable)?;
        let mut record = store::load_all(&self.handle)?
            .into_iter()
            .find(|record| record.receipt.artifact_id == artifact_id)
            .ok_or(Issue::StoreUnavailable)?;
        self.reconcile_publication(&mut record)?;
        Ok(PublicationDetail {
            receipt: record.receipt,
            inputs: record.inputs,
            validation_scope:
                "本地 Stored ZIP/XML/目录与资源结构检查；不是第三方 EPUB 认证或上架凭证。"
                    .to_owned(),
        })
    }
}

fn reference(path: &str, bytes: &[u8]) -> InputReference {
    InputReference {
        relative_path: path.to_owned(),
        content_hash: sha256_hex(bytes),
        size_bytes: bytes.len() as u64,
        confirmation_id: None,
        confirmation_revision: None,
    }
}
fn publication_cancel(token: &CancellationToken) -> Result<(), Issue> {
    if token.is_cancelled() {
        Err(Issue::Cancelled)
    } else {
        Ok(())
    }
}
