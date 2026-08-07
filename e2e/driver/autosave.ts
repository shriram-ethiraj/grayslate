import { invokeInApp } from "./invoke.js";

export interface ForcedAutosaveCycle {
  source: "slates" | "local" | null;
  backendDirty: boolean;
  scheduledActions: number;
}

/**
 * Make the active document immediately eligible for one real autosave cycle.
 *
 * The E2E command does not write content itself. It drives the production
 * scheduler, event, authorization, save coordinator, and atomic write path,
 * then resolves only after that path has settled.
 */
export async function forceAutosaveCycle(): Promise<ForcedAutosaveCycle> {
  return invokeInApp<ForcedAutosaveCycle>("e2e_force_autosave_cycle");
}
