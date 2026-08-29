// Generated contract mirror for NG-A003. Build-validated and unreachable from production.

export type ProjectIdV1 = string & { readonly __brand: "ProjectIdV1" };
export type SessionIdV1 = string & { readonly __brand: "SessionIdV1" };
export type TurnIdV1 = string & { readonly __brand: "TurnIdV1" };
export type AgentActionIdV1 = string & { readonly __brand: "AgentActionIdV1" };
export type ToolCallIdV1 = string & { readonly __brand: "ToolCallIdV1" };
export type ContentHashV1 = string & { readonly __brand: "ContentHashV1" };
export type ProjectRelativePathV1 = string & {
  readonly __brand: "ProjectRelativePathV1";
};

export type RegisteredToolIdV1 =
  | "project.list_markdown"
  | "project.read_markdown"
  | "project.search_markdown"
  | "project.validate_markdown"
  | "project.create_markdown"
  | "project.patch_markdown";

export interface ToolArgumentsV1 {
  path?: ProjectRelativePathV1;
  directory?: ProjectRelativePathV1;
  query?: string;
  content?: string;
  patch?: string;
  expectedSourceHash?: ContentHashV1;
}

export interface ModelToolRequestV1 {
  callId: ToolCallIdV1;
  toolId: RegisteredToolIdV1;
  arguments: ToolArgumentsV1;
  purpose: string;
}

export type AgentActionPayloadV1 =
  | {
      schemaVersion: 1;
      action: "respond";
      skillIds: string[];
      visibleText: string;
    }
  | {
      schemaVersion: 1;
      action: "request_tools";
      skillIds: string[];
      toolRequests: ModelToolRequestV1[];
    }
  | {
      schemaVersion: 1;
      action: "complete";
      skillIds: string[];
      visibleSummary: string;
    };

export interface ProjectToolBindingV1 {
  projectId: ProjectIdV1;
  sessionId: SessionIdV1;
  turnId: TurnIdV1;
}

export interface ToolRequestV1 {
  schemaVersion: 1;
  binding: ProjectToolBindingV1;
  serialOrdinal: number;
  callId: ToolCallIdV1;
  toolId: RegisteredToolIdV1;
  arguments: ToolArgumentsV1;
  purpose: string;
}

export interface ToolResultDataV1 {
  itemCount: number;
  contentHash: ContentHashV1;
  affectedPaths: ProjectRelativePathV1[];
}

export interface ToolResultV1 {
  schemaVersion: 1;
  binding: ProjectToolBindingV1;
  callId: ToolCallIdV1;
  toolId: RegisteredToolIdV1;
  status: "succeeded" | "failed" | "cancelled";
  errorCode:
    | "not-found"
    | "hash-conflict"
    | "invalid-markdown"
    | "policy-blocked"
    | "cancelled"
    | "internal-redacted"
    | null;
  data: ToolResultDataV1 | null;
  resultHash: ContentHashV1;
  truncated: boolean;
  truncationReason: "item_limit" | "byte_limit" | null;
  trust: "untrusted_tool_data";
}

export interface AgentActionRecordContextV1 {
  actionId: AgentActionIdV1;
  projectId: ProjectIdV1;
  sessionId: SessionIdV1;
  turnId: TurnIdV1;
  sequence: number;
  provider: {
    providerId: string;
    presetVersion: string;
    modelId: string;
  };
  skillPack: {
    version: string;
    hash: ContentHashV1;
  };
  effectiveSkillIds: string[];
  policyDisposition: "allowed" | "blocked";
  createdAtMs: number;
}

export interface AgentActionRecordV1 {
  schemaVersion: 1;
  context: AgentActionRecordContextV1;
  payload: AgentActionPayloadV1;
  payloadHash: ContentHashV1;
  previousActionHash: ContentHashV1 | null;
  actionHash: ContentHashV1;
  redactionState: "redacted";
}

export interface RedactedAgentErrorLogV1 {
  schemaVersion: 1;
  code: string;
  sourceHash: ContentHashV1;
  attemptCount: number;
  redacted: true;
}
