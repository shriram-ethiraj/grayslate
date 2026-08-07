/**
 * Production replacement for the E2E runtime modules.
 *
 * Vite resolves the virtual runtime boundary to this module outside
 * `--mode e2e`. Selecting the implementation before graph construction keeps
 * test-only modules and WebdriverIO out of normal builds entirely.
 */

export interface E2EPendingWork {
	phase: "ready";
	inFlight: 0;
	commands: [];
	tasks: [];
	revision: 0;
}

const finishNoop = (): void => {};

export async function initializeE2ERuntime(): Promise<void> {}

export function markE2EReady(): void {}

export function markE2EClosing(): void {}

export function beginTrackedWork(_label: string): () => void {
	return finishNoop;
}

export function beginTrackedInvoke(_command: string): void {}

export function finishTrackedInvoke(_command: string): void {}

export function readPendingWork(): E2EPendingWork {
	return {
		phase: "ready",
		inFlight: 0,
		commands: [],
		tasks: [],
		revision: 0,
	};
}
