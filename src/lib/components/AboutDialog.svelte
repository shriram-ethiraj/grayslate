<script lang="ts">
    import { openUrl } from "@tauri-apps/plugin-opener";
    import aboutImage from "$lib/assets/grayslate-about.png";
    import { Badge } from "$lib/components/ui/badge/index.js";
    import * as Dialog from "$lib/components/ui/dialog/index.js";
    import { Button } from "$lib/components/ui/button/index.js";
    import { Separator } from "$lib/components/ui/separator/index.js";
    import ArrowUpRight from "~icons/lucide/arrow-up-right";
    import BadgeCheck from "~icons/lucide/badge-check";
    import LoaderCircle from "~icons/lucide/loader-circle";
    import RefreshCw from "~icons/lucide/refresh-cw";
    import {
        appMenuState,
        checkForAppUpdates,
        installAvailableUpdate,
    } from "$lib/state/appMenu.svelte";
    import { appDialogsState, closeAppDialog } from "$lib/state/appDialogs.svelte";

    const REPOSITORY_URL = "https://github.com/shriram-ethiraj/grayslate";
    const RELEASES_URL = "https://github.com/shriram-ethiraj/grayslate/releases";
    const LICENSE_URL = "https://github.com/shriram-ethiraj/grayslate/blob/main/LICENSE";

    const isChecking = $derived(appMenuState.updateStatus === "checking");
    const isInstalling = $derived(appMenuState.updateStatus === "installing");
    const canInstall = $derived(appMenuState.updateStatus === "available");
    const showsInstallAction = $derived(
        appMenuState.updateStatus === "available" ||
            appMenuState.updateStatus === "installing",
    );
    const isUpToDate = $derived(appMenuState.updateStatus === "up-to-date");
    const isInstalled = $derived(appMenuState.updateStatus === "installed");
    const isBusy = $derived(isChecking || isInstalling);
    const currentVersionLabel = $derived(
        appMenuState.currentVersion || appMenuState.appVersion || "Unknown",
    );
    const whatsNewUrl = $derived.by(() => {
        if (!appMenuState.availableVersion) {
            return RELEASES_URL;
        }

        const version = appMenuState.availableVersion.startsWith("v")
            ? appMenuState.availableVersion
            : `v${appMenuState.availableVersion}`;

        return `${RELEASES_URL}/tag/${version}`;
    });

    async function openWhatsNew(): Promise<void> {
        await openUrl(whatsNewUrl);
    }

    async function openExternal(url: string): Promise<void> {
        await openUrl(url);
    }
</script>

<Dialog.Root
    open={appDialogsState.active.type === "about"}
    onOpenChange={(isOpen) => {
        if (!isOpen) closeAppDialog();
    }}
>
    <Dialog.Content
        class="p-0 sm:max-w-[44rem]"
        onOpenAutoFocus={(event) => {
            event.preventDefault();
        }}
    >
        <Dialog.Header class="sr-only">
            <Dialog.Title>About {appMenuState.appName}</Dialog.Title>
            <Dialog.Description>
                Product information and update status for {appMenuState.appName}.
            </Dialog.Description>
        </Dialog.Header>

           <!-- Inset the clipped content by 1px so the shared dialog ring stays visible on all
               sides, while still clipping the split layout to the rounded corners. -->
           <div class="m-px overflow-hidden rounded-[calc(var(--radius-lg)-1px)]">
           <div class="grid min-h-[18rem] gap-0 md:grid-cols-[17rem_minmax(0,1fr)]">
            <div class="border-b bg-muted/30 p-8 text-center md:border-r md:border-b-0">
                <div class="flex h-full flex-col items-center justify-center gap-4">
                    <img
                        src={aboutImage}
                        alt="Grayslate icon"
                        class="h-32 w-32 object-contain md:h-36 md:w-36"
                    />

                    <p class="text-2xl font-semibold tracking-tight text-foreground">
                        {appMenuState.appName}
                    </p>
                </div>
            </div>

            <div class="flex min-h-0 flex-col justify-between bg-background p-8">
                <div class="space-y-5">
                    <div class="space-y-3">
                        <Dialog.Title class="text-2xl font-semibold tracking-tight">
                            About
                        </Dialog.Title>

                        <div class="flex flex-wrap items-center gap-3 text-sm">
                            <Badge variant="secondary">v{currentVersionLabel}</Badge>
                            <Button
                                variant="link"
                                size="sm"
                                class="h-auto p-0"
                                onclick={() => {
                                    void openWhatsNew();
                                }}
                            >
                                What's new
                                <ArrowUpRight class="size-4" />
                            </Button>
                        </div>

                        <p class="leading-6 text-muted-foreground">
                            A fast scratchpad for code, data, and quick thinking.
                        </p>
                    </div>

                    <Separator />

                    <div class="space-y-3 text-sm">
                        {#if isChecking}
                            <div class="flex items-center gap-2 text-foreground">
                                <LoaderCircle class="size-4 animate-spin text-muted-foreground" />
                                <span>Checking for updates...</span>
                            </div>
                        {:else if showsInstallAction}
                            <div class="flex flex-wrap items-center gap-3">
                                <Button
                                    size="sm"
                                    disabled={isBusy}
                                    onclick={() => {
                                        void installAvailableUpdate();
                                    }}
                                >
                                    {#if isInstalling}
                                        Installing...
                                    {:else}
                                        Update to v{appMenuState.availableVersion}
                                    {/if}
                                </Button>
                            </div>
                        {:else if isUpToDate || isInstalled}
                            <div class="flex items-center gap-2 text-emerald-600 dark:text-emerald-400">
                                <BadgeCheck class="size-4" />
                                <span>
                                    {#if isInstalled}
                                        Update installed. Restart Grayslate when convenient.
                                    {:else}
                                        Grayslate is up to date.
                                    {/if}
                                </span>
                            </div>
                        {/if}

                        {#if appMenuState.updatePublishedAt && canInstall}
                            <p class="text-muted-foreground">
                                Published {appMenuState.updatePublishedAt}
                            </p>
                        {/if}

                    </div>

                    <div>
                        <Button
                            variant="outline"
                            size="sm"
                            class="w-fit self-start"
                            onclick={() => {
                                void checkForAppUpdates({ openDialog: false });
                            }}
                            disabled={isBusy}
                        >
                            <RefreshCw class={isChecking ? "mr-2 size-4 animate-spin" : "mr-2 size-4"} />
                            Check for updates
                        </Button>
                    </div>
                </div>

                <div class="pt-4 text-xs text-muted-foreground">
                    <div class="flex flex-wrap items-center gap-x-3 gap-y-2">
                        <Button
                            variant="link"
                            size="sm"
                            class="h-auto p-0 text-xs text-muted-foreground"
                            onclick={() => {
                                void openExternal(LICENSE_URL);
                            }}
                        >
                            MIT License
                        </Button>

                        <span aria-hidden="true">&#183;</span>

                        <Button
                            variant="link"
                            size="sm"
                            class="h-auto p-0 text-xs text-muted-foreground"
                            onclick={() => {
                                void openExternal(REPOSITORY_URL);
                            }}
                        >
                            GitHub
                        </Button>
                    </div>
                </div>
            </div>
        </div>
        </div>
    </Dialog.Content>
</Dialog.Root>