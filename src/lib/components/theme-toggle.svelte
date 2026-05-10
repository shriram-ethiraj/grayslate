<script lang="ts">
    import { Button } from "$lib/components/ui/button/index.js";
    import Sun from "~icons/lucide/sun";
    import Moon from "~icons/lucide/moon";
    import { applyTheme, getThemeFromLocalStorage } from "$lib/state/appSettings.svelte";

    let isDark = $state(getThemeFromLocalStorage() === "dark");

    function toggleTheme() {
        isDark = !isDark;
        applyTheme(isDark);
    }
</script>

<Button
    variant="ghost"
    size="icon"
    aria-label="Toggle theme"
    title={isDark ? "Light Mode" : "Dark Mode"}
    onclick={toggleTheme}
>
    {#if isDark}
        <Sun class="size-4 transition-all" />
    {:else}
        <Moon class="size-4 transition-all" />
    {/if}
</Button>
