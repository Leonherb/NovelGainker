export type ProviderModeV2 =
  | "control"
  | "conversation"
  | "composition"
  | "reconciliation"
  | "memory_compaction";

export type CompletionContractV2 =
  | "skill_selection_v2"
  | "agent_action_v2"
  | "conversation_text_v1"
  | "composition_markdown_v1"
  | "canon_proposal_v1"
  | "memory_compaction_v1";

export type SkillIntentClassV2 =
  | "product_help"
  | "project_discussion"
  | "canon_update"
  | "composition"
  | "revision"
  | "review"
  | "visual"
  | "publish";

export type SkillSelectionV2 = {
  schemaVersion: 2;
  requestId: string;
  selectedSkillIds: string[];
  intentClass: SkillIntentClassV2;
  visibleReason: string;
};

export type ContextQueryKindV2 =
  | "entity_or_rule"
  | "relationship"
  | "plot_thread"
  | "chapter_or_scene"
  | "timeline"
  | "clue_or_material"
  | "full_text"
  | "marked_context"
  | "skill_resource";

export type ContextQueryV2 = {
  kind: ContextQueryKindV2;
  terms: string[];
  knownEntityIds: string[];
  knownSourceIds: string[];
  purpose: string;
};

export type AgentControlActionV2 =
  | {
      schemaVersion: 2;
      requestId: string;
      action: "request_context";
      selectedSkillIds: string[];
      visibleReason: string;
      queries: ContextQueryV2[];
    }
  | {
      schemaVersion: 2;
      requestId: string;
      action: "start_response";
      selectedSkillIds: string[];
      visibleReason: string;
      responseBrief: string;
      sourceIds: string[];
      markSuggestions: Array<{ sourceId: string; reason: string }>;
    }
  | {
      schemaVersion: 2;
      requestId: string;
      action: "submit_canon_proposal";
      selectedSkillIds: string[];
      visibleReason: string;
      targetKind: CanonTargetKindV1;
      displayName: string | null;
      proposalBrief: string;
      sourceIds: string[];
    };

export type CanonTargetKindV1 =
  | "character"
  | "setting"
  | "material"
  | "plot"
  | "chapter";

export type CanonFactSourceV1 = {
  kind: "visible_message" | "draft_artifact" | "canon_source";
  sourceId: string;
  sourceHash: string;
};

export type CanonFactV1 = {
  field: string;
  value: string;
  sources: CanonFactSourceV1[];
};

export type CanonProposalV1 = {
  schemaVersion: 1;
  requestId: string;
  proposalId: string;
  targetKind: CanonTargetKindV1;
  displayName: string;
  operation: "create" | "extend" | "append_chapter";
  facts: CanonFactV1[];
};

export type ConversationStatusV2 = "complete" | "cancelled" | "failed";

export type ConversationMessageV2 = {
  schemaVersion: 2;
  messageId: string;
  responseId: string | null;
  turnId: string;
  role: "user" | "assistant";
  body: string;
  bodyHash: string;
  sourceIds: string[];
  status: ConversationStatusV2;
  createdAtMs: number;
};
