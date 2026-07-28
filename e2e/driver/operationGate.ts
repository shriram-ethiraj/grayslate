import fs from "node:fs";
import { TIMEOUTS } from "../config/timeouts.js";
import { operationSignalPath } from "../helpers/sandbox.js";
import { invokeInApp } from "./invoke.js";
import { waitForProcessSignal } from "./processSignal.js";
import { waitFor } from "./wait.js";

export type OperationGate =
  | "file-read"
  | "editor-find"
  | "transformation"
  | "markdown-render"
  | "sidebar-search"
  | "csv-initialize"
  | "csv-dispose";

export async function armOperationGate(name: OperationGate): Promise<void> {
  await invokeInApp<void>("e2e_arm_operation_gate", { name });
}

export async function waitForOperationGate(name: OperationGate): Promise<void> {
  await waitFor(
    () => invokeInApp<boolean>("e2e_operation_gate_reached", { name }),
    {
      message: `The '${name}' worker never reached its deterministic E2E checkpoint.`,
      timeoutMs: TIMEOUTS.heavy,
    },
  );
}

export async function releaseOperationGate(name: OperationGate): Promise<void> {
  await invokeInApp<void>("e2e_release_operation_gate", { name });
}

/**
 * Release a held backend worker without asking the webview to execute IPC.
 *
 * This is required when the production action that triggers cancellation is
 * itself awaiting the held worker. The E2E-only checkpoint watches this marker
 * alongside its normal condition variable; release semantics are identical.
 */
export function releaseOperationGateOutOfProcess(name: OperationGate): void {
  fs.writeFileSync(operationSignalPath(name, "release"), "release\n", "utf8");
}

/**
 * Observe a backend signal without entering WebDriver's serialized command
 * queue. This can run while a click command is still awaiting the held worker.
 */
export function waitForOperationSignalOutOfProcess(
  name: OperationGate,
): Promise<void> {
  return waitForProcessSignal(
    operationSignalPath(name, "observed"),
    `The '${name}' backend observation never arrived.`,
  );
}
