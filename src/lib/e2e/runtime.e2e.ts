import "./determinism";

export * from "./workTracker";

/** Install WebdriverIO's guest bridge after the deterministic bridge exists. */
export async function initializeE2ERuntime(): Promise<void> {
	await import("@wdio/tauri-plugin");
}
