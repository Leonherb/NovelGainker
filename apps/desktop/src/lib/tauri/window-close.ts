import { getCurrentWindow } from "@tauri-apps/api/window";

export type CloseRequestEvent = {
  preventDefault: () => void;
};

export type WindowClosePort = {
  onCloseRequested: (
    handler: (event: CloseRequestEvent) => void | Promise<void>,
  ) => Promise<() => void>;
  close: () => Promise<void>;
};

export const desktopWindowClosePort: WindowClosePort = {
  onCloseRequested: (handler) => getCurrentWindow().onCloseRequested(handler),
  close: () => getCurrentWindow().close(),
};
