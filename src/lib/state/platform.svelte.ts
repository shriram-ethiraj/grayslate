import { type as getOsType } from "@tauri-apps/plugin-os";

export type AppOs = ReturnType<typeof getOsType>;

export const platformState = $state<{
    ready: boolean;
    osType: AppOs | undefined;
}>({
    ready: false,
    osType: undefined,
});

let initPromise: Promise<AppOs> | undefined;

async function resolvePlatformOsType(): Promise<AppOs> {
    try {
        return getOsType();
    } catch (error: unknown) {
        console.warn("[Platform] Failed to detect OS type", error);
        return "windows";
    }
}

export function initPlatformState(): Promise<AppOs> {
    initPromise ??= resolvePlatformOsType().then((osType) => {
        platformState.osType = osType;
        platformState.ready = true;
        return osType;
    });

    return initPromise;
}

export function getPlatformOsType(): AppOs {
    return platformState.osType ?? "windows";
}