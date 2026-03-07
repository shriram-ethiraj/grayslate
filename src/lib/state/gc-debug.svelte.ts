export type GcReclaimTriggerSource = "none" | "file-open" | "manual";

export type GcDebugDecisionStage =
    | "idle"
    | "skipped"
    | "running"
    | "completed"
    | "failed";

export type GcDebugSnapshot = {
    currentDocBytes: number;
    lastTriggerSource: GcReclaimTriggerSource;
    lastDecisionStage: GcDebugDecisionStage;
    lastDecisionReason: string;
    lastShrinkBytes: number;
    lastShrinkRatio: number;
    lastPostShrinkRatio: number;
    lastSystemUsed: number;
    lastAvailable: number;
    lastPressureBytes: number;
    minAvailableRamBytes: number;
    minPeakDocBytes: number;
    minShrinkBytes: number;
    minShrinkRatio: number;
    maxPostShrinkRatio: number;
    lastEvaluatedAt: number;
    lastReclaimAt: number;
    lastError: string;
};

export type GcDebugLogEntry = {
    id: number;
    timestamp: number;
    stage: GcDebugDecisionStage;
    source: GcReclaimTriggerSource;
    message: string;
};

export type GcDebugFinding = {
    timestamp: number;
    stage: GcDebugDecisionStage;
    source: GcReclaimTriggerSource;
    reason: string;
    currentDocBytes: number;
    lastReclaimAt: number;
    lastSystemUsed?: number;
    lastAvailable?: number;
    lastPressureBytes?: number;
    lastShrinkBytes?: number;
    lastShrinkRatio?: number;
    lastPostShrinkRatio?: number;
    lastError?: string;
};

const DEFAULT_SNAPSHOT: GcDebugSnapshot = {
    currentDocBytes: 0,
    lastTriggerSource: "none",
    lastDecisionStage: "idle",
    lastDecisionReason: "Waiting for document activity.",
    lastShrinkBytes: 0,
    lastShrinkRatio: 0,
    lastPostShrinkRatio: 0,
    lastSystemUsed: 0,
    lastAvailable: 0,
    lastPressureBytes: 0,
    minAvailableRamBytes: 500 * 1024 * 1024,
    minPeakDocBytes: 16 * 1024 * 1024,
    minShrinkBytes: 8 * 1024 * 1024,
    minShrinkRatio: 0.35,
    maxPostShrinkRatio: 0.7,
    lastEvaluatedAt: 0,
    lastReclaimAt: 0,
    lastError: "",
};

const MAX_GC_DEBUG_LOGS = 250;

let nextGcDebugLogId = 1;

export const gcDebugState = $state<{
    snapshot: GcDebugSnapshot;
    logs: GcDebugLogEntry[];
}>({
    snapshot: { ...DEFAULT_SNAPSHOT },
    logs: [],
});

export function resetGcDebugState(): void {
    gcDebugState.logs = [];
    gcDebugState.snapshot = { ...DEFAULT_SNAPSHOT };
}

export function applyGcDebugFinding(finding: GcDebugFinding): void {
    gcDebugState.snapshot = {
        ...gcDebugState.snapshot,
        currentDocBytes: finding.currentDocBytes,
        lastTriggerSource: finding.source,
        lastDecisionStage: finding.stage,
        lastDecisionReason: finding.reason,
        lastEvaluatedAt: finding.timestamp,
        lastReclaimAt: finding.lastReclaimAt,
        ...(finding.lastSystemUsed !== undefined
            ? { lastSystemUsed: finding.lastSystemUsed }
            : {}),
        ...(finding.lastAvailable !== undefined
            ? { lastAvailable: finding.lastAvailable }
            : {}),
        ...(finding.lastPressureBytes !== undefined
            ? { lastPressureBytes: finding.lastPressureBytes }
            : {}),
        ...(finding.lastShrinkBytes !== undefined
            ? { lastShrinkBytes: finding.lastShrinkBytes }
            : {}),
        ...(finding.lastShrinkRatio !== undefined
            ? { lastShrinkRatio: finding.lastShrinkRatio }
            : {}),
        ...(finding.lastPostShrinkRatio !== undefined
            ? { lastPostShrinkRatio: finding.lastPostShrinkRatio }
            : {}),
        ...(finding.lastError !== undefined
            ? { lastError: finding.lastError }
            : {}),
    };

    gcDebugState.logs = [
        {
            id: nextGcDebugLogId++,
            timestamp: finding.timestamp,
            stage: finding.stage,
            source: finding.source,
            message: finding.reason,
        },
        ...gcDebugState.logs,
    ].slice(0, MAX_GC_DEBUG_LOGS);
}