import { describe, expect, it, vi } from "vitest";
import workflowRegistryFixture from "../../../../../contracts/ipc/workflow-registry.v1.json";
import {
  parseWorkflowRegistrySnapshot,
  requestWorkflowRegistry,
  WORKFLOW_REGISTRY_LOAD_COMMAND,
} from "./workflow";

describe("Workflow Registry IPC contract", () => {
  it("accepts the five routable workflows and read-only Storytelling entry in fixed order", () => {
    expect(parseWorkflowRegistrySnapshot(workflowRegistryFixture)).toEqual(workflowRegistryFixture);
    expect(() =>
      parseWorkflowRegistrySnapshot({
        ...workflowRegistryFixture,
        workflows: [
          ...workflowRegistryFixture.workflows,
          { ...workflowRegistryFixture.workflows[0], id: "novelgainker.outline-create" },
        ],
      }),
    ).toThrow();
    expect(() =>
      parseWorkflowRegistrySnapshot({
        ...workflowRegistryFixture,
        workflows: workflowRegistryFixture.workflows.map((workflow, index) =>
          index === 0 ? { ...workflow, allowedTools: ["source.create_character"] } : workflow,
        ),
      }),
    ).toThrow();
  });

  it("invokes only the argument-free read-only registry command", async () => {
    const invoke = vi.fn().mockResolvedValue(workflowRegistryFixture);

    await expect(requestWorkflowRegistry(invoke)).resolves.toEqual(workflowRegistryFixture);
    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith(WORKFLOW_REGISTRY_LOAD_COMMAND);
    expect(JSON.stringify(invoke.mock.calls)).not.toMatch(/tool|path|prompt|key|provider|model/iu);
  });

  it("converts malformed or raw failures into redacted contract errors", async () => {
    await expect(
      requestWorkflowRegistry(vi.fn().mockResolvedValue({ workflows: [] })),
    ).rejects.toEqual({
      code: "contract.invalid_response",
      messageKey: "errors.invalidRuntimeResponse",
    });
    await expect(
      requestWorkflowRegistry(
        vi.fn().mockRejectedValue(new Error("C:\\Private\\SKILL.md should not leak")),
      ),
    ).rejects.toEqual({
      code: "runtime.unknown",
      messageKey: "errors.unknown",
    });
  });
});
