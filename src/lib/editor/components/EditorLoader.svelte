<script lang="ts">
    import Loader2 from "~icons/lucide/loader-2";

    let {
        message = "Loading…",
        subMessage = "",
        visible = true,
        progress = -1 as number | undefined,
    }: {
        message?: string;
        subMessage?: string;
        visible?: boolean;
        progress?: number;
    } = $props();

    let showBar = $derived(progress !== undefined);
    let isIndeterminate = $derived(progress === -1);
    let clampedProgress = $derived(
        progress !== undefined && progress >= 0
            ? Math.min(100, Math.max(0, progress))
            : 0,
    );
</script>

{#if visible}
    <div
        class="absolute inset-0 z-50 flex flex-col items-center justify-center bg-background/72 backdrop-blur-[2px]"
        aria-live="polite"
        aria-label={message}
    >
        <!-- Progress bar: anchored to the top of the overlay -->
        {#if showBar}
            <div
                class="absolute top-0 left-0 right-0 h-0.5 bg-muted overflow-hidden"
                role="progressbar"
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={isIndeterminate ? undefined : clampedProgress}
            >
                {#if isIndeterminate}
                    <!-- Indeterminate: a shuttle animation -->
                    <div
                        class="h-full w-1/3 bg-primary rounded-full animate-[indeterminate_1.4s_ease-in-out_infinite]"
                    ></div>
                {:else}
                    <!-- Determinate: filled width, smooth transition -->
                    <div
                        class="h-full bg-primary rounded-full transition-[width] duration-300 ease-out"
                        style="width: {clampedProgress}%"
                    ></div>
                {/if}
            </div>
        {/if}

        <div class="flex flex-col items-center">
            <!-- Spinner -->
            <div class="flex items-center justify-center w-10 h-10">
                <Loader2 class="h-8 w-8 animate-spin text-muted-foreground" />
            </div>

            <!-- Text Area -->
            <div class="h-0 w-px relative flex flex-col items-center">
                <div
                    class="absolute top-4 w-max flex flex-col items-center gap-1.5"
                >
                    {#if message}
                        <p
                            class="text-sm font-medium text-foreground tracking-tight"
                        >
                            {message}
                        </p>
                    {/if}
                    {#if subMessage}
                        <p
                            class="text-xs text-muted-foreground tabular-nums opacity-70"
                        >
                            {subMessage}
                        </p>
                    {/if}
                </div>
            </div>
        </div>
    </div>
{/if}
