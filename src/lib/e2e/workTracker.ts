export interface E2EPendingWork {
	phase: E2ELifecyclePhase;
	inFlight: number;
	commands: string[];
	tasks: string[];
	revision: number;
}

export type E2ELifecyclePhase = "booting" | "ready" | "closing";

const commandCounts = new Map<string, number>();
const taskCounts = new Map<string, number>();
let inFlight = 0;
let revision = 0;
let phase: E2ELifecyclePhase = "booting";
const TRACK_E2E_WORK = import.meta.env.MODE === "e2e";
const finishNoop = (): void => {};

function describeCounts(counts: ReadonlyMap<string, number>): string[] {
	return [...counts.entries()]
		.sort(([left], [right]) => left.localeCompare(right))
		.flatMap(([label, count]) =>
			count === 1 ? [label] : [`${label} (${count})`],
		);
}

function setLifecyclePhase(next: E2ELifecyclePhase): void {
	if (phase === next) return;
	phase = next;
	revision += 1;
}

export function markE2EReady(): void {
	setLifecyclePhase("ready");
}

export function markE2EClosing(): void {
	setLifecyclePhase("closing");
}

/**
 * Track finite application work that is not represented by a Tauri invoke.
 *
 * The returned completion callback is idempotent so error and teardown paths
 * can safely share it without driving the global count below zero.
 */
export function beginTrackedWork(label: string): () => void {
	if (!TRACK_E2E_WORK) return finishNoop;

	let completed = false;
	inFlight += 1;
	revision += 1;
	taskCounts.set(label, (taskCounts.get(label) ?? 0) + 1);

	return () => {
		if (completed) return;
		completed = true;
		inFlight = Math.max(0, inFlight - 1);
		revision += 1;
		const remaining = (taskCounts.get(label) ?? 1) - 1;
		if (remaining <= 0) taskCounts.delete(label);
		else taskCounts.set(label, remaining);
	};
}

/** Record one application-owned IPC call in the E2E build. */
export function beginTrackedInvoke(command: string): void {
	inFlight += 1;
	revision += 1;
	commandCounts.set(command, (commandCounts.get(command) ?? 0) + 1);
}

/** Complete one application-owned IPC call without allowing counter drift. */
export function finishTrackedInvoke(command: string): void {
	inFlight = Math.max(0, inFlight - 1);
	revision += 1;
	const remaining = (commandCounts.get(command) ?? 1) - 1;
	if (remaining <= 0) commandCounts.delete(command);
	else commandCounts.set(command, remaining);
}

export function readPendingWork(): E2EPendingWork {
	return {
		phase,
		inFlight,
		commands: describeCounts(commandCounts),
		tasks: describeCounts(taskCounts),
		revision,
	};
}
