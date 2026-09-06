// These are deliberately scoped readers, not a general YAML parser. Duplicate blocks or
// direct fields fail closed; a nested/historical approval cannot authorize current work.
export function currentNr6Authorization(status) {
  const gate = stateSection(status, "nr_g6_technical_submission");
  return (
    currentNr5Authorization(status) &&
    [
      ["technical_checks_passed", "true"],
      ["initial_product_approved_claim", "false"],
      ["continuous_entry_authorized", "true"],
      ["report", '"docs/reviews/novelist-agent-runtime-nr-g6-technical-submission.zh-CN.md"'],
      ["report_sha256", '"bdf59cb8982cc4f783b5e569133265dce7a189153d9de49eda7d1f997364e1d8"'],
      ["manifest", '"target/qa/nr-506/nr-g6-manifest.json"'],
      ["manifest_sha256", '"166e2ef11c69230329ee3eba07d478098cfd40adb1db3cb3f15ddf24b83325d1"'],
      ["full_quality", '"target/qa/nr-506/quality.json"'],
      ["full_quality_sha256", '"098ca628049e23eb684692bb42968f7489f28fd4cfc3a90f89ae1a0044683541"'],
      ["final_nr_g7_approved", "false"],
    ].every(([key, value]) => stateField(gate, key, value))
  );
}
export function currentNr602Authorization(status) {
  const completed = stateSection(status, "nr601_completion");
  return (
    currentNr6Authorization(status) &&
    [
      ["full_quality_passed", "true"],
      ["quality", '"target/qa/nr-601/quality.json"'],
      ["quality_sha256", '"9f617a5518518de20ca66d87597ddde50f31aa1c1a302668c8cb73cc8be1d773"'],
      ["validation", '"target/qa/nr-601/validation.json"'],
      ["validation_sha256", '"fb22fd0dd3455a2ed56252ad626ceb55881ddad525e6bcc1da0376a75e831c3e"'],
    ].every(([key, value]) => stateField(completed, key, value))
  );
}
export function currentNr603Authorization(status) {
  const completed = stateSection(status, "nr602_completion");
  return (
    currentNr602Authorization(status) &&
    [
      ["full_quality_passed", "true"],
      ["quality", '"target/qa/nr-602/quality.json"'],
      ["quality_sha256", '"ea1421aa835117bc8ea9fb8aa8c1877f24fe24c03fe562d8b31c096b38b75001"'],
      ["validation", '"target/qa/nr-602/validation.json"'],
      ["validation_sha256", '"5b510cf6f0406db3a86435ac3562aa609df5a43f9fc498e3ef164c0b0e2b7f06"'],
    ].every(([key, value]) => stateField(completed, key, value))
  );
}
export function stateSection(status, name) {
  if (!/^[a-z][a-z0-9_]*$/u.test(name)) return "";
  const blocks = [
    ...status.matchAll(new RegExp(`^${name}:\\r?\\n((?:[ \\t].*(?:\\r?\\n|$)|\\r?\\n)*)`, "gmu")),
  ];
  return blocks.length === 1 ? blocks[0][1] : "";
}

export function stateField(block, name, value) {
  const fields = [...block.matchAll(new RegExp(`^  ${name}: ([^\\r\\n]*)\\r?$`, "gmu"))];
  return fields.length === 1 && fields[0][1] === value;
}

export function historicalNr4Approval(status) {
  const gate = stateSection(status, "gate");
  const packet = stateSection(status, "nr4_packet");
  return (
    stateField(gate, "nrG4Approved", "true") &&
    [
      ["state_scope", '"historical_nr4_implementation_and_nr_g5_submission_snapshot"'],
      ["id", '"NR-4-D1"'],
      ["formation_authorized", "true"],
      ["product_approved", "true"],
      ["implementation_authorized", "true"],
      ["implementation_completed", "true"],
      ["nrG5Approved", "false"],
      ["real_provider_live_smoke_executed", "false"],
      ["live_smoke_decision", '"explicitly_waived_risk_accepted"'],
      ["chapter_commit_source_staging", '"approved_deferred_to_nr5"'],
    ].every(([key, value]) => stateField(packet, key, value))
  );
}

export function currentNr5Authorization(status) {
  const gate = stateSection(status, "current_gate");
  const packet = stateSection(status, "successor_packet");
  return (
    [
      ["id", '"NR-G5"'],
      ["status", '"passed"'],
      ["review_status", '"product_approved"'],
      ["nrG5Approved", "true"],
      ["product_approved", "true"],
      ["approval_file", '"docs/reviews/novelist-agent-runtime-nr-g5-product-approval.zh-CN.md"'],
    ].every(([key, value]) => stateField(gate, key, value)) &&
    [
      ["id", '"NR-5-NR-6-D1"'],
      ["product_approved", "true"],
      ["implementation_started", "true"],
      ["implementation_release_status", '"released_continuous_with_quality_gates"'],
      [
        "implementation_approval_file",
        '"docs/reviews/novelist-agent-runtime-nr5-nr6-implementation-approval.zh-CN.md"',
      ],
      ["nr_g6_human_wait_required", "false"],
      ["nr_g6_technical_pass_required", "true"],
      ["nr_g7_human_approval_required", "true"],
      ["automated_live_execution_authorized", "false"],
      ["cloud_sync_in_scope", "false"],
    ].every(([key, value]) => stateField(packet, key, value))
  );
}
