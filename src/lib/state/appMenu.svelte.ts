import { invoke } from "@tauri-apps/api/core";
import { toast } from "$lib/components/ui/sonner";
import { appDialogsState, openAboutAppDialog, closeAppDialog } from "$lib/state/appDialogs.svelte";

export type UpdateStatus =
    | "idle"
    | "checking"
    | "up-to-date"
    | "available"
    | "installing"
    | "installed"
    | "unconfigured"
    | "error";

type UpdateCheckResponse =
    | {
          status: "unconfigured";
          message: string;
          current_version: string;
      }
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
};

export const appMenuState = $state({
    appName: "Grayslate",
    appVersion: "",
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

export async function ensureAppInfoLoaded(): Promise<void> {
    if (appInfoLoaded) {
        return;
    }

    const appInfo = await invoke<AppInfo>("get_app_info");
    appMenuState.appName = appInfo.appName;
    appMenuState.appVersion = appInfo.appVersion;
    appMenuState.currentVersion = appInfo.appVersion;
    appInfoLoaded = true;
}

export async function openAboutDialog(): Promise<void> {
    await ensureAppInfoLoaded();
    openAboutAppDialog();
    void checkForAppUpdates({ openDialog: false, notify: false });
}

export async function checkForAppUpdates(options?: {
    openDialog?: boolean;
    notify?: boolean;
}): Promise<void> {
    await ensureAppInfoLoaded();

    const shouldNotify = options?.notify ?? true;

    if (
        appMenuState.updateStatus === "checking" ||
        appMenuState.updateStatus === "installing"
    ) {
        return;
    }

    if (options?.openDialog ?? true) {
        openAboutAppDialog();
    }

    appMenuState.updateStatus = "checking";
    appMenuState.updateMessage = "Checking for updates...";
    appMenuState.currentVersion = appMenuState.appVersion;
    resetUpdateDetails();

    try {
        const result = await invoke<UpdateCheckResponse>("check_for_updates");
        appMenuState.currentVersion = result.current_version;

        switch (result.status) {
            case "unconfigured":
                appMenuState.updateStatus = "unconfigured";
                appMenuState.updateMessage = result.message;
                if (shouldNotify) {
                    toast.message("Updates are not configured for this build.");
                }
                return;
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
        const message =
            error instanceof Error ? error.message : "Failed to check for updates.";
        appMenuState.updateStatus = "error";
        appMenuState.updateMessage = message;
        if (shouldNotify) {
            toast.error(message);
        }
    }
}

export async function installAvailableUpdate(): Promise<void> {
    if (appMenuState.updateStatus !== "available") {
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
        const message =
            error instanceof Error ? error.message : "Failed to install update.";
        appMenuState.updateStatus = "error";
        appMenuState.updateMessage = message;
        toast.error(message);
    }
}
