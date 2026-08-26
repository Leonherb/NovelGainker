import { describe, expect, it, vi } from "vitest";
import {
  parseAppStateSnapshot,
  requestAppState,
  requestSaveUiPreferences,
  requestSetCurrentFile,
  type AppStateSnapshot,
} from "./app-state";

const snapshot: AppStateSnapshot = {
  schemaVersion: 3,
  ui: {
    leftCollapsed: true,
    rightCollapsed: false,
    apiCardExpanded: false,
    rightPanelWidth: 920,
  },
  recentProjects: [
    {
      projectId: "30000000-0000-4000-8000-000000000003",
      displayName: "Novel One",
    },
  ],
  projectFiles: [
    {
      projectId: "30000000-0000-4000-8000-000000000003",
      currentRelativeFile: "chapters/001.md",
    },
  ],
  recoveredWithDefaults: false,
};

describe("NG-106 App State IPC contract", () => {
  it("accepts only bounded versioned preferences and opaque recent projects", () => {
    expect(parseAppStateSnapshot(snapshot)).toEqual(snapshot);
    expect(() =>
      parseAppStateSnapshot({
        ...snapshot,
        recentProjects: [{ ...snapshot.recentProjects[0], rootPath: "D:\\Novel" }],
      }),
    ).toThrow();
    expect(() =>
      parseAppStateSnapshot({
        ...snapshot,
        ui: { ...snapshot.ui, rightPanelWidth: 1_601 },
      }),
    ).toThrow();
    expect(() =>
      parseAppStateSnapshot({
        ...snapshot,
        projectFiles: [
          {
            projectId: "40000000-0000-4000-8000-000000000004",
            currentRelativeFile: "../secret.md",
          },
        ],
      }),
    ).toThrow();
  });

  it("uses exact load, UI preference and relative-current-file commands", async () => {
    const invoke = vi
      .fn()
      .mockResolvedValueOnce(snapshot)
      .mockResolvedValueOnce({ saved: true })
      .mockResolvedValueOnce({ saved: true });

    await requestAppState(invoke);
    await requestSaveUiPreferences(snapshot.ui, invoke);
    await requestSetCurrentFile("20000000-0000-4000-8000-000000000002", "chapters/001.md", invoke);

    expect(invoke.mock.calls).toEqual([
      ["app_state_load", {}],
      ["app_state_save_ui", { preferences: snapshot.ui }],
      [
        "app_state_set_current_file",
        {
          sessionId: "20000000-0000-4000-8000-000000000002",
          relativePath: "chapters/001.md",
        },
      ],
    ]);
    expect(JSON.stringify(invoke.mock.calls)).not.toMatch(
      /api.?key|provider|content|query|[A-Z]:\\/iu,
    );
  });

  it("normalizes raw failures and malformed responses without exposing inputs", async () => {
    await expect(
      requestAppState(vi.fn().mockResolvedValue({ ...snapshot, schemaVersion: 99 })),
    ).rejects.toEqual({
      code: "contract.invalid_response",
      messageKey: "errors.invalidRuntimeResponse",
    });
    await expect(
      requestSaveUiPreferences(
        snapshot.ui,
        vi.fn().mockRejectedValue(new Error("api_key=secret D:\\Novel")),
      ),
    ).rejects.toEqual({
      code: "runtime.unknown",
      messageKey: "errors.unknown",
    });
  });
});
