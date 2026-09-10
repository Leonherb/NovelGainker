//! Required templates are complete Host resources, not manuscript evidence.
use super::*;

pub(super) fn required_templates(
    pack: &ValidatedCapabilityPack,
    request: &ValidatedRequest<'_>,
) -> Result<(Value, usize), BuildError> {
    let mut templates = Vec::new();
    let mut bytes = 0usize;
    if request.profile == ToolProtocolProfile::Nr5 {
        let capability = pack
            .capability(request.capability_id)
            .ok_or(BuildError::Io)?;
        for id in &capability.template_resources {
            let content = pack.resource(id).ok_or(BuildError::Io)?;
            bytes = bytes.checked_add(content.len()).ok_or(BuildError::Io)?;
            if bytes > MAX_CAPABILITY_RESOURCE_CONTENT_BYTES {
                // Never deliver a partial required template or mint a method receipt for it.
                return Err(BuildError::Io);
            }
            templates.push(json!({
                "sourceType": "capability_method",
                "resourceId": id,
                "sourceSha256": pack.resource_sha256(id),
                "content": content,
                "truncated": false,
                "status": "ok"
            }));
        }
    }
    Ok((json!(templates), bytes))
}

pub(super) fn validate_required_templates(
    bundle: &Value,
    pack: &ValidatedCapabilityPack,
    request: &ValidatedRequest<'_>,
) -> Result<(), BuildError> {
    if request.profile == ToolProtocolProfile::Nr5
        && bundle["requiredTemplates"] != required_templates(pack, request)?.0
    {
        return Err(BuildError::Io);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability_pack::embedded_pack;

    #[test]
    fn every_nr5_method_delivers_complete_templates_without_requesting_them() {
        let fixture = super::super::tests::Fixture::new();
        let pack = embedded_pack().unwrap();
        for capability in pack.capabilities() {
            let input = NovelistReadContextRequestV1 {
                capability_id: capability.id.clone(),
                capability_resources: vec![],
                canon_operations: vec![],
            };
            let request = validate_request(&pack, &input, ToolProtocolProfile::Nr5).unwrap();
            let bundle = build_bundle_v2(
                &fixture.handle,
                &pack,
                request,
                &CancellationToken::default(),
            )
            .unwrap();
            let delivered = bundle["requiredTemplates"].as_array().unwrap();
            assert_eq!(delivered.len(), capability.template_resources.len());
            for (item, id) in delivered.iter().zip(&capability.template_resources) {
                assert_eq!(item["content"].as_str(), pack.resource(id));
                assert_eq!(item["sourceSha256"].as_str(), pack.resource_sha256(id));
                assert_eq!(item["truncated"], false);
            }
            assert!(serialized_len(&bundle).unwrap() <= ToolProtocolProfile::Nr5.result_bytes());
            assert_eq!(bundle["canon"], json!([]));
            let request = validate_request(&pack, &input, ToolProtocolProfile::Nr4).unwrap();
            let historical = build_bundle_v2(
                &fixture.handle,
                &pack,
                request,
                &CancellationToken::default(),
            )
            .unwrap();
            assert!(historical.get("requiredTemplates").is_none());
        }
    }

    #[test]
    fn incomplete_or_substituted_template_is_rejected() {
        let pack = embedded_pack().unwrap();
        let input = NovelistReadContextRequestV1 {
            capability_id: "create-character".to_owned(),
            capability_resources: vec![],
            canon_operations: vec![],
        };
        let request = validate_request(&pack, &input, ToolProtocolProfile::Nr5).unwrap();
        let mut bundle =
            json!({"requiredTemplates": required_templates(&pack, &request).unwrap().0});
        assert!(validate_required_templates(&bundle, &pack, &request).is_ok());
        bundle["requiredTemplates"][0]["content"] = json!("partial");
        assert!(validate_required_templates(&bundle, &pack, &request).is_err());
    }
}
