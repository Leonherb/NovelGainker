//! Evidence is issued only for method/file bytes actually delivered to the model.
use crate::canon_change::{CanonEntityKind, ChangeError, ChangeEvidence, MAX_CHANGE_FILE_BYTES};
use crate::capability_pack::ValidatedCapabilityPack;
use crate::protocol_profile::ToolProtocolProfile;
use crate::workspace::sha256_hex;
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone)]
struct DeliveredFile {
    hash: String,
    size: usize,
    content: String,
    next_line: usize,
    next_byte: usize,
}

#[derive(Clone, Default)]
pub struct DeliveredCanonReads {
    files: BTreeMap<String, DeliveredFile>,
    profile: ToolProtocolProfile,
}

impl DeliveredCanonReads {
    pub fn for_profile(profile: ToolProtocolProfile) -> Self {
        Self {
            profile,
            ..Self::default()
        }
    }
    /// Transactional in-memory update: an oversized/failed result issues no receipts.
    pub fn annotate(
        &mut self,
        evidence: &mut ChangeEvidence,
        pack: &ValidatedCapabilityPack,
        bundle: &mut Value,
    ) -> Result<(), ChangeError> {
        let mut next_reads = self.clone();
        let mut next_evidence = evidence.clone();
        let mut candidate = bundle.clone();
        let capability = &candidate["capability"];
        let capability_id = capability["id"]
            .as_str()
            .ok_or(ChangeError::MethodNotRead)?;
        let instruction = capability["instruction"]
            .as_str()
            .ok_or(ChangeError::MethodNotRead)?;
        let delivered_hash = sha256_hex(instruction.as_bytes());
        // Hash the delivered instruction, not its claimed resource hash.
        let method_receipt = next_evidence.record_method(pack, capability_id, &delivered_hash)?;
        let mut targets = Vec::new();
        let entries = candidate["canon"]
            .as_array()
            .ok_or(ChangeError::IncompleteRead)?;
        for item in entries {
            if item["kind"] != "read" || item["sourceType"] != "canon_snapshot" {
                continue;
            }
            if let Some(target) = next_reads.accept_fragment(&mut next_evidence, item)? {
                targets.push(target);
            }
        }
        let chapter_method = capability_id == "novel-story-telling";
        candidate["changeEvidence"] = json!({
            "methodReceiptId": method_receipt,
            "capabilityId": capability_id,
            "completeTargets": targets,
            "notice": "只有 completeTargets 中的完整原文回执可用于更新；搜索、列表、截断片段不构成更新权限。"
        });
        if self.profile == ToolProtocolProfile::Nr5 && chapter_method {
            candidate["chapterCandidateIdentity"] = json!({
                "chapterId": next_evidence.chapter_draft_id(),
                "candidateId": next_evidence.chapter_draft_id(),
                "notice": "新章试写若要支持下一轮接受这版，在唯一完整 markdown 代码块中使用此 Host chapterId；更新候选保持完整读取的原章节 ID。此标识不表示已保存。"
            });
        }
        if serde_json::to_vec(&candidate)
            .map_err(|_| ChangeError::InvalidRequest)?
            .len()
            > self.profile.result_bytes()
        {
            return Err(ChangeError::BudgetExceeded);
        }
        *bundle = candidate;
        *self = next_reads;
        *evidence = next_evidence;
        Ok(())
    }

    fn accept_fragment(
        &mut self,
        evidence: &mut ChangeEvidence,
        item: &Value,
    ) -> Result<Option<Value>, ChangeError> {
        let Some(path) = item["relativePath"].as_str() else {
            return Ok(None);
        };
        let Some(kind) = [
            CanonEntityKind::Character,
            CanonEntityKind::World,
            CanonEntityKind::Material,
            CanonEntityKind::Plot,
            CanonEntityKind::Outline,
            CanonEntityKind::Chapter,
        ]
        .into_iter()
        .find(|kind| kind.validate_path(path).is_ok()) else {
            return Ok(None);
        };
        let Some(size) = item["sizeBytes"]
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
        else {
            return Ok(None);
        };
        let maximum = if kind == CanonEntityKind::Chapter {
            if self.profile != ToolProtocolProfile::Nr5 {
                return Ok(None);
            }
            crate::chapter_contract::MAX_CHAPTER_BYTES
        } else {
            MAX_CHANGE_FILE_BYTES
        };
        if size > maximum {
            return Ok(None);
        }
        let Some(hash) = item["contentHash"].as_str() else {
            return Ok(None);
        };
        let Some(content) = item["content"].as_str() else {
            return Ok(None);
        };
        let start_line = item["range"]["startLine"].as_u64().unwrap_or(0) as usize;
        let start_byte = item["range"]["startByte"].as_u64().unwrap_or(u64::MAX) as usize;
        if start_line == 1 && start_byte == 0 {
            if self.files.len() >= 8 && !self.files.contains_key(path) {
                return Err(ChangeError::BudgetExceeded);
            }
            self.files.insert(
                path.to_owned(),
                DeliveredFile {
                    hash: hash.to_owned(),
                    size,
                    content: String::new(),
                    next_line: 1,
                    next_byte: 0,
                },
            );
        }
        let Some(delivered) = self.files.get_mut(path) else {
            return Ok(None);
        };
        if delivered.hash != hash
            || delivered.size != size
            || delivered.next_line != start_line
            || delivered.next_byte != start_byte
            || delivered.content.len().saturating_add(content.len()) > size
        {
            self.files.remove(path);
            return Ok(None);
        }
        delivered.content.push_str(content);
        let newlines = content.bytes().filter(|byte| *byte == b'\n').count();
        delivered.next_line += newlines;
        delivered.next_byte = if let Some(last) = content.rfind('\n') {
            content.len() - last - 1
        } else {
            delivered.next_byte + content.len()
        };
        if item["status"] != "ok"
            || item["truncated"] != false
            || delivered.content.len() != size
            || sha256_hex(delivered.content.as_bytes()) != hash
        {
            return Ok(None);
        }
        // Nonconforming/legacy files remain readable but are not mutable candidates.
        let Ok((target_id, receipt_id)) =
            evidence.record_complete_read(kind, path, &delivered.content, hash, None)
        else {
            return Ok(None);
        };
        Ok(Some(json!({
            "entityKind": kind, "relativePath": path, "targetId": target_id,
            "readReceiptId": receipt_id, "expectedSourceHash": hash
        })))
    }
}
