<script lang="ts">
    import { TooltipButton } from "$lib/components/ui/tooltip/index.js";
    import Sun from "~icons/lucide/sun";
    import Moon from "~icons/lucide/moon";
    import { applyTheme, getThemeFromLocalStorage } from "$lib/state/appSettings.svelte";

    let isDark = $state(getThemeFromLocalStorage() === "dark");

    function toggleTheme() {
        isDark = !isDark;
        applyTheme(isDark);
    }
</script>

<TooltipButton
    variant="ghost"
    size="icon"
    data-testid="theme-toggle"
    aria-label="Toggle theme"
    tooltip={isDark ? "Use light theme" : "Use dark theme"}
    onclick={toggleTheme}
>
    {#if isDark}
        <Sun class="size-4 transition-all" />
    {:else}
        <Moon class="size-4 transition-all" />
    {/if}
</TooltipButton>
