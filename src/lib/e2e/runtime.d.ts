declare module "virtual:grayslate-e2e-runtime" {
	export type E2ELifecyclePhase = "booting" | "ready" | "closing";

	export interface E2EPendingWork {
		phase: E2ELifecyclePhase;
		inFlight: number;
		commands: string[];
		tasks: string[];
		revision: number;
	}

	export function initializeE2ERuntime(): Promise<void>;
	export function markE2EReady(): void;
	export function markE2EClosing(): void;
	export function beginTrackedWork(label: string): () => void;
	export function beginTrackedInvoke(command: string): void;
	export function finishTrackedInvoke(command: string): void;
	export function readPendingWork(): E2EPendingWork;
}
