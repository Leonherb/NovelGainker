// Generated contract mirror. Runtime use remains disabled until the GA0 gate is approved.
export type AuthorizationPolicyV1 = "plan_only" | "review_every_write" | "auto_apply_safe";

export type RiskLevelV1 = "read-safe" | "write-safe" | "always-confirm" | "forbidden-v1";

export type GovernedActionV1 =
  | "list_managed_markdown"
  | "read_managed_markdown"
  | "search_managed_markdown"
  | "validate_managed_markdown"
  | "create_entity_markdown"
  | "patch_entity_markdown"
  | "create_draft_chapter"
  | "replace_whole_file"
  | "replace_whole_chapter"
  | "promote_chapter_state"
  | "update_story_ledger"
  | "modify_project_rules"
  | "modify_critical_metadata"
  | "import_outside_project"
  | "publish_or_export"
  | "delete_path"
  | "move_path"
  | "rename_path"
  | "arbitrary_filesystem"
  | "system_capability"
  | "modify_authorization_or_skill_pack";

export type RegisteredToolIdV1 =
  | "project.list_markdown"
  | "project.read_markdown"
  | "project.search_markdown"
  | "project.validate_markdown"
  | "project.create_markdown"
  | "project.patch_markdown";

export type ValidatedMutationShapeV1 =
  | "create_entity_markdown"
  | "create_draft_chapter"
  | "patch_hunks"
  | "replace_whole_file"
  | "replace_whole_chapter"
  | "promote_chapter_state"
  | "update_story_ledger"
  | "modify_project_rules"
  | "modify_critical_metadata"
  | "publish_or_export";

export interface ProjectToolBindingV1 {
  projectId: string;
  sessionId: string;
  turnId: string;
}

export interface WriteToolClassificationV1 {
  schemaVersion: 1;
  binding: ProjectToolBindingV1;
  toolRequestHash: string;
  toolId: RegisteredToolIdV1;
  path: string;
  mutationShape: ValidatedMutationShapeV1;
  action: GovernedActionV1;
  risk: RiskLevelV1;
  classificationHash: string;
}

export interface AuthorizationDecisionV1 {
  action: GovernedActionV1;
  risk: RiskLevelV1;
  policy: AuthorizationPolicyV1;
  effect: "execute" | "await_confirmation" | "block";
  reason:
    | "read-safe"
    | "auto-apply-safe"
    | "review-every-write"
    | "high-risk-confirmation"
    | "plan-only-write-blocked"
    | "forbidden-v1";
}

export interface TurnAuthorizationV1 {
  schemaVersion: 1;
  policy: AuthorizationPolicyV1;
  decisions: readonly AuthorizationDecisionV1[];
  transactionEffect: "execute" | "await_confirmation" | "block";
  preconfirmationWriteCount: 0;
  atomicWriteTransaction: true;
}

export interface ConfirmationBindingV1 {
  projectId: string;
  sessionId: string;
  turnId: string;
  actionPlanHash: string;
  sourceSetHash: string;
  policySnapshotHash: string;
}

export interface HighRiskConfirmationV1 {
  schemaVersion: 1;
  confirmationId: string;
  authority: "user_via_application";
  binding: ConfirmationBindingV1;
  issuedAtMs: number;
  used: boolean;
  confirmationHash: string;
}

export interface FullChatPersistenceV1 {
  enabled: boolean;
  userOptIn: boolean;
  protection: "disabled" | "windows_user_context";
  maxUtf8Bytes: number;
  retentionDays: number;
  privacyNoticeVersion: number | null;
  clearGeneration: number;
}

export interface SessionPersistenceSnapshotV1 {
  schemaVersion: 1;
  projectId: string;
  sessionId: string;
  authorizationPolicy: AuthorizationPolicyV1;
  summary: Readonly<Record<string, unknown>> | null;
  messageIndex: readonly Readonly<Record<string, unknown>>[];
  terminalTurns: readonly Readonly<Record<string, unknown>>[];
  fullChat: FullChatPersistenceV1;
  clearGeneration: number;
}

export interface FrozenFileStateV1 {
  path: string;
  sourceHash: string | null;
  targetMustBeAbsent: boolean;
}

export interface PlannedFileMutationV1 {
  frozen: FrozenFileStateV1;
  classification: WriteToolClassificationV1;
  plannedResultHash: string;
  estimatedUndoBytes: number;
}

export interface MultiFileTransactionPlanV1 {
  schemaVersion: 1;
  projectId: string;
  sessionId: string;
  turnId: string;
  policySnapshotHash: string;
  mutations: readonly PlannedFileMutationV1[];
  sourceSetHash: string;
  actionPlanHash: string;
  transactionHash: string;
  totalEstimatedUndoBytes: number;
}

export interface UndoTargetV1 {
  path: string;
  expectedCurrentHash: string;
}

export interface UndoRecordV1 {
  schemaVersion: 1;
  projectId: string;
  sessionId: string;
  turnId: string;
  transactionHash: string;
  blobId: string;
  blobHash: string;
  encryptedBytes: number;
  protection: "windows_user_context";
  targets: readonly UndoTargetV1[];
  targetSetHash: string;
  createdAtMs: number;
  expiresAtMs: number;
  status: "ready" | "consumed" | "expired" | "hash_conflict" | "decrypt_failed" | "cleared";
  clearGeneration: number;
}
