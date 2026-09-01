import {
  discoverReleaseSurface,
  inspectIpcClosure,
  proveReleaseScannerRejectsNewProductionFile,
  scanRetiredProductReferences,
} from "./nr-g1-gate-lib.mjs";

const root = process.cwd();
const releaseFiles = discoverReleaseSurface(root);
const retired = scanRetiredProductReferences(root, releaseFiles);
if (retired.length !== 0) {
  throw new Error(`retired production reference remains: ${JSON.stringify(retired[0])}`);
}
const ipc = inspectIpcClosure(root);
if (!proveReleaseScannerRejectsNewProductionFile(root)) {
  throw new Error("NR-G1 release scanner negative fixture did not fail as required");
}

console.log(
  `[nr0-boundaries] PASS ${releaseFiles.length} release files / ${ipc.manifest.length} commands / ${ipc.conversation.length} Conversation IPC / negative fixture rejected`,
);
