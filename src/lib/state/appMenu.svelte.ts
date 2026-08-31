import { invoke } from "$lib/ipc";
import { toast } from "$lib/components/ui/sonner";
import { openAboutAppDialog } from "$lib/state/appDialogs.svelte";
import { platformState } from "$lib/state/platform.svelte";
import { confirmBeforeLeavingDocument } from "$lib/state/unsavedChangesGuard.svelte";
import { listen } from "@tauri-apps/api/event";

export type UpdatePolicy = "disabled" | "self-update" | "system-managed";

export type UpdateStatus =
    | "idle"
    | "checking"
    | "up-to-date"
    | "available"
    | "installing"
    | "installed"
    | "disabled"
    | "system-managed"
    | "error";

export type UpdateDiscoverySource = "automatic" | "manual";

type UpdateCheckResponse =
    | {
          status: "up-to-date";
          message: string;
          current_version: string;
      }
    | {
          status: "available";
          message: string;
          current_version: string;
          version: string;
          published_at: string | null;
      };

type UpdateInstallResponse = {
    version: string;
    message: string;
};

type BackendUpdateStatus =
    | { status: "idle" }
    | { status: "checking"; source: UpdateDiscoverySource }
    | ({ status: "up-to-date"; source: UpdateDiscoverySource } & Extract<UpdateCheckResponse, { status: "up-to-date" }>)
    | ({ status: "available"; source: UpdateDiscoverySource } & Extract<UpdateCheckResponse, { status: "available" }>)
    | { status: "installing"; message: string }
    | { status: "installed"; version: string; message: string }
    | { status: "error"; source: UpdateDiscoverySource; message: string };

type AppInfo = {
    appName: string;
    appVersion: string;
    updatePolicy: UpdatePolicy;
};

export const appMenuState = $state({
    appName: "Grayslate",
    appVersion: "",
    updatePolicy: "disabled" as UpdatePolicy,
    updateStatus: "idle" as UpdateStatus,
    updateMessage: "Check for updates to see whether a newer release is available.",
    currentVersion: "",
    availableVersion: "",
    updatePublishedAt: "",
    updateDiscoverySource: null as UpdateDiscoverySource | null,
});

let appInfoLoaded = false;

function resetUpdateDetails(): void {
    appMenuState.availableVersion = "";
    appMenuState.updatePublishedAt = "";
}

function resetAutomaticUpdateState(): void {
    if (
        appMenuState.updateDiscoverySource !== "automatic" ||
        appMenuState.updateStatus === "installing" ||
        appMenuState.updateStatus === "installed"
    ) {
        return;
    }

    appMenuState.updateStatus = "idle";
    appMenuState.updateMessage =
        "Check for updates to see whether a newer release is available.";
    appMenuState.currentVersion = appMenuState.appVersion;
    appMenuState.updateDiscoverySource = null;
    resetUpdateDetails();
}

function applyUpdatePolicy(policy: UpdatePolicy): void {
    appMenuState.updatePolicy = policy;
    if (policy === "system-managed") {
        appMenuState.updateStatus = "system-managed";
        appMenuState.updateMessage =
            "Updates for this build are managed by your package manager.";
    } else if (policy === "disabled") {
        appMenuState.updateStatus = "disabled";
        appMenuState.updateMessage = "Updates are unavailable for this build.";
    }
}

function commandErrorMessage(error: unknown, fallback: string): string {
    if (error instanceof Error && error.message) {
        return error.message;
    }
    if (typeof error === "string" && error) {
        return error;
    }
    if (typeof error === "object" && error !== null && "message" in error) {
        const message = error.message;
        if (typeof message === "string" && message) {
            return message;
        }
    }
    return fallback;
}

export async function ensureAppInfoLoaded(): Promise<void> {
    if (appInfoLoaded) {
        return;
    }

    const appInfo = await invoke<AppInfo>("get_app_info");
    appMenuState.appName = appInfo.appName;
    appMenuState.appVersion = appInfo.appVersion;
    appMenuState.currentVersion = appInfo.appVersion;
    applyUpdatePolicy(appInfo.updatePolicy);
    appInfoLoaded = true;
}

export async function openAboutDialog(): Promise<void> {
    await ensureAppInfoLoaded();
    openAboutAppDialog();
}

function applyBackendUpdateStatus(status: BackendUpdateStatus): void {
    if (status.status === "idle") {
        resetAutomaticUpdateState();
        return;
    }

    if (status.status === "checking") {
        appMenuState.updateStatus = "checking";
        appMenuState.updateDiscoverySource = status.source;
        appMenuState.updateMessage = "Checking for updates...";
        resetUpdateDetails();
        return;
    }

    if (status.status === "installing") {
        appMenuState.updateStatus = "installing";
        appMenuState.updateMessage = status.message;
        return;
    }

    if (status.status === "installed") {
        appMenuState.updateStatus = "installed";
        appMenuState.availableVersion = status.version;
        appMenuState.updateMessage = status.message;
        return;
    }

    appMenuState.updateDiscoverySource = status.source;
    if (status.status === "error") {
        appMenuState.updateStatus = "error";
        appMenuState.updateMessage = status.message;
        return;
    }

    appMenuState.currentVersion = status.current_version;
    appMenuState.updateMessage = status.message;
    if (status.status === "available") {
        appMenuState.updateStatus = "available";
        appMenuState.availableVersion = status.version;
        appMenuState.updatePublishedAt = status.published_at ?? "";
    } else {
        appMenuState.updateStatus = "up-to-date";
        resetUpdateDetails();
    }
}

/** Mirror the process-wide Rust updater state into this webview. */
export async function startUpdateStatusSync(): Promise<() => void> {
    let eventSeen = false;
    const unlisten = await listen<BackendUpdateStatus>("updates://status", (event) => {
        eventSeen = true;
        applyBackendUpdateStatus(event.payload);
    });
    try {
        const status = await invoke<BackendUpdateStatus>("get_update_status");
        if (!eventSeen) applyBackendUpdateStatus(status);
    } catch (error) {
        unlisten();
        throw error;
    }
    return unlisten;
}

export async function checkForAppUpdates(options?: {
    openDialog?: boolean;
    notify?: boolean;
    source?: UpdateDiscoverySource;
}): Promise<void> {
    await ensureAppInfoLoaded();

    const source = options?.source ?? "manual";
    const shouldNotify = options?.notify ?? source === "manual";

    if (options?.openDialog ?? true) {
        openAboutAppDialog();
    }

    if (appMenuState.updatePolicy !== "self-update") {
        applyUpdatePolicy(appMenuState.updatePolicy);
        if (shouldNotify) {
            toast.message(appMenuState.updateMessage);
        }
        return;
    }

    if (
        appMenuState.updateStatus === "checking" ||
        appMenuState.updateStatus === "installing"
    ) {
        return;
    }

    appMenuState.updateStatus = "checking";
    appMenuState.updateDiscoverySource = source;
    appMenuState.updateMessage = "Checking for updates...";
    appMenuState.currentVersion = appMenuState.appVersion;
    resetUpdateDetails();

    try {
        const result = await invoke<UpdateCheckResponse>("check_for_updates");
        appMenuState.currentVersion = result.current_version;

        switch (result.status) {
            case "up-to-date":
                appMenuState.updateStatus = "up-to-date";
                appMenuState.updateMessage = result.message;
                if (shouldNotify) {
                    toast.success(result.message);
                }
                return;
            case "available":
                appMenuState.updateStatus = "available";
                appMenuState.updateMessage = result.message;
                appMenuState.availableVersion = result.version;
                appMenuState.updatePublishedAt = result.published_at ?? "";
                if (shouldNotify) {
                    toast.message(result.message);
                }
                return;
        }
    } catch (error) {
        const message = commandErrorMessage(
            error,
            "Failed to check for updates.",
        );
        appMenuState.updateStatus = "error";
        appMenuState.updateMessage = message;
        if (shouldNotify) {
            toast.error(message);
        }
    }
}

export async function installAvailableUpdate(): Promise<void> {
    if (
        appMenuState.updatePolicy !== "self-update" ||
        appMenuState.updateStatus !== "available"
    ) {
        return;
    }

    const canInstall = await confirmBeforeLeavingDocument();
    // The unsaved-changes prompt temporarily replaces About in the app-level
    // dialog slot. Restore the update surface for either cancellation or the
    // download/install progress state.
    openAboutAppDialog();
    if (!canInstall) {
        return;
    }

    appMenuState.updateStatus = "installing";
    appMenuState.updateMessage = platformState.osType === "windows"
        ? "Downloading the update. Grayslate will close when the Windows installer starts."
        : "Downloading and installing the update. Grayslate will not restart automatically.";

    try {
        const result = await invoke<UpdateInstallResponse>(
            "install_available_update",
        );
        appMenuState.updateStatus = "installed";
        appMenuState.availableVersion = result.version;
        appMenuState.updateMessage = result.message;
        toast.success(result.message);
    } catch (error) {
        const message = commandErrorMessage(error, "Failed to install update.");
        appMenuState.updateStatus = "error";
        appMenuState.updateMessage = message;
        toast.error(message);
    }
}
