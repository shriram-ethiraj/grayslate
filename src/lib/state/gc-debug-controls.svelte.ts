import type { GcDebugFinding } from "$lib/state/gc-debug.svelte";

export const gcDebugControls = $state({
    enabled: false,
    panelOpen: false,
});

type GcDebugModule = typeof import("$lib/state/gc-debug.svelte");

let gcDebugModulePromise: Promise<GcDebugModule> | undefined;

function loadGcDebugModule(): Promise<GcDebugModule> {
    gcDebugModulePromise ??= import("$lib/state/gc-debug.svelte");
    return gcDebugModulePromise;
}

export function isGcDebugEnabled(): boolean {
    return gcDebugControls.enabled;
}

export function setGcDebugEnabled(enabled: boolean): void {
    const wasEnabled = gcDebugControls.enabled;
    gcDebugControls.enabled = enabled;
    if (!enabled) {
        gcDebugControls.panelOpen = false;
        if (wasEnabled) {
            void loadGcDebugModule().then(({ resetGcDebugState }) => {
                resetGcDebugState();
            });
        }
    }
}

export function setGcDebugPanelOpen(open: boolean): void {
    if (!gcDebugControls.enabled && open) {
        return;
    }
    gcDebugControls.panelOpen = open;
}

export function toggleGcDebugPanel(): void {
    setGcDebugPanelOpen(!gcDebugControls.panelOpen);
}

export function reportGcDebugFinding(finding: GcDebugFinding): void {
    if (!gcDebugControls.enabled) {
        return;
    }

    void loadGcDebugModule().then(({ applyGcDebugFinding }) => {
        applyGcDebugFinding(finding);
    });
}