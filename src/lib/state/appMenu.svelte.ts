import { invoke } from "@tauri-apps/api/core";
import { toast } from "$lib/components/ui/sonner";
import { openAboutAppDialog } from "$lib/state/appDialogs.svelte";

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
});

let appInfoLoaded = false;

function resetUpdateDetails(): void {
    appMenuState.availableVersion = "";
    appMenuState.updatePublishedAt = "";
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
    if (appMenuState.updatePolicy === "self-update") {
        void checkForAppUpdates({ openDialog: false, notify: false });
    }
}

export async function checkForAppUpdates(options?: {
    openDialog?: boolean;
    notify?: boolean;
}): Promise<void> {
    await ensureAppInfoLoaded();

    const shouldNotify = options?.notify ?? true;

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

    appMenuState.updateStatus = "installing";
    appMenuState.updateMessage =
        "Installing the update. Grayslate will not restart automatically.";

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
