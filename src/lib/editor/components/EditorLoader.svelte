<script lang="ts">
    import { Loader2 } from "@lucide/svelte";
    import { cn } from "$lib/utils";

    let {
        message = "Loading…",
        subMessage = "",
        visible = true,
    }: {
        message?: string;
        subMessage?: string;
        visible?: boolean;
    } = $props();
</script>

{#if visible}
    <div
        class="absolute inset-0 z-50 flex items-center justify-center bg-background"
        aria-live="polite"
        aria-label={message}
    >
        <div class="flex flex-col items-center">
            <!-- Spinner: Centered parent -->
            <div class="flex items-center justify-center w-10 h-10">
                <Loader2 class="h-8 w-8 animate-spin text-muted-foreground" />
            </div>

            <!-- Text Area: Decoupled from the spinner's center using a zero-height container -->
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
