// Generated contract mirror for NG-A002. It is build-validated and not imported by production code.

export type ProjectIdV1 = string & { readonly __brand: "ProjectIdV1" };
export type SessionIdV1 = string & { readonly __brand: "SessionIdV1" };
export type TurnIdV1 = string & { readonly __brand: "TurnIdV1" };
export type SummaryIdV1 = string & { readonly __brand: "SummaryIdV1" };
export type ContentHashV1 = string & { readonly __brand: "ContentHashV1" };

export type AuthorizationPolicyV1 = "plan_only" | "review_every_write" | "auto_apply_safe";
export type SessionStatusV1 = "active" | "recovery_required" | "terminated";
export type TurnStateV1 =
  | "idle"
  | "planning"
  | "reading"
  | "acting"
  | "awaiting_high_risk_confirmation"
  | "applying"
  | "validating"
  | "completed"
  | "cancelled"
  | "failed"
  | "recovery_required";
export type TurnIntentV1 = "discussion" | "clarification" | "product_help" | "project_work";
export type ProjectReferenceKindV1 = "summary" | "tool_result" | "transaction";
export type TurnFailureCodeV1 =
  | "provider_snapshot_drift"
  | "skill_pack_snapshot_drift"
  | "authorization_policy_snapshot_drift"
  | "contract_violation"
  | "write_recovery_required";

export interface ProviderSnapshotV1 {
  providerId: string;
  presetVersion: string;
  modelId: string;
}

export interface SkillPackSnapshotV1 {
  version: string;
  hash: ContentHashV1;
}

export interface TurnBudgetV1 {
  maxAgentSteps: number;
  maxToolCalls: number;
  maxToolCallsPerBatch: number;
  maxUniqueProjectFiles: number;
  maxModifiedProjectFiles: number;
  maxWriteTransactions: 1;
  maxProviderRequestBytes: number;
}

export interface FrozenTurnSnapshotV1 {
  provider: ProviderSnapshotV1;
  skillPack: SkillPackSnapshotV1;
  authorizationPolicy: AuthorizationPolicyV1;
  userRequestHash: ContentHashV1;
  budget: TurnBudgetV1;
}

export interface SessionSummaryRefV1 {
  projectId: ProjectIdV1;
  summaryId: SummaryIdV1;
  version: number;
  contentHash: ContentHashV1;
  utf8Bytes: number;
}

export interface TurnIndexEntryV1 {
  projectId: ProjectIdV1;
  turnId: TurnIdV1;
  state: "completed" | "cancelled" | "failed" | "recovery_required";
  updatedAtMs: number;
}

export type WriterLeaseV1 =
  | { status: "released" }
  | { status: "held"; turnId: TurnIdV1 }
  | { status: "recovery_required"; turnId: TurnIdV1 };

export interface ProjectAgentSessionV1 {
  schemaVersion: 1;
  sessionId: SessionIdV1;
  projectId: ProjectIdV1;
  status: SessionStatusV1;
  provider: ProviderSnapshotV1;
  skillPack: SkillPackSnapshotV1;
  authorizationPolicy: AuthorizationPolicyV1;
  summary: SessionSummaryRefV1 | null;
  recentTurns: TurnIndexEntryV1[];
  activeTurnId: TurnIdV1 | null;
  writerLease: WriterLeaseV1;
  createdAtMs: number;
  updatedAtMs: number;
  terminationReason: "project_closed" | "user_ended" | "fatal_contract_error" | null;
}

export interface ProjectScopedReferenceV1 {
  projectId: ProjectIdV1;
  kind: ProjectReferenceKindV1;
  referenceId: string;
  contentHash: ContentHashV1;
}

export interface TurnFailureV1 {
  code: TurnFailureCodeV1;
  detailHash: ContentHashV1;
}

export interface AgentTurnV1 {
  schemaVersion: 1;
  turnId: TurnIdV1;
  sessionId: SessionIdV1;
  projectId: ProjectIdV1;
  state: TurnStateV1;
  intent: TurnIntentV1;
  selectedSkillIds: string[];
  frozen: FrozenTurnSnapshotV1;
  references: ProjectScopedReferenceV1[];
  cancellationRequested: boolean;
  writeTransactionStarted: boolean;
  failure: TurnFailureV1 | null;
  createdAtMs: number;
  updatedAtMs: number;
}

export interface ProjectAgentBasePolicyV1 {
  schemaVersion: 1;
  policyId: "project-agent-base-policy-v1";
  isSkill: false;
  requiresProjectSession: true;
  allowsBypassChat: false;
  allowsZeroSkillDirectResponse: true;
  allowedWithoutOpenProject: ["create_project", "open_project", "product_help"];
}
