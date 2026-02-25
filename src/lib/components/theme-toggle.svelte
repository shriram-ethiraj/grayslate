<script lang="ts">
    import { Button } from "$lib/components/ui/button/index.js";
    import * as Tooltip from "$lib/components/ui/tooltip/index.js";
    import Sun from "@lucide/svelte/icons/sun";
    import Moon from "@lucide/svelte/icons/moon";
    import { onMount } from "svelte";

    let isDark = $state(true);

    onMount(() => {
        // Initialize from localStorage or fallback to dark mode assumption from app.html
        const storedTheme = localStorage.getItem("theme");
        if (storedTheme) {
            isDark = storedTheme === "dark";
        } else {
            isDark = document.documentElement.classList.contains("dark");
        }
        applyTheme(isDark);
    });

    function applyTheme(dark: boolean) {
        if (dark) {
            document.documentElement.classList.add("dark");
        } else {
            document.documentElement.classList.remove("dark");
        }
    }

    function toggleTheme() {
        isDark = !isDark;
        applyTheme(isDark);
        localStorage.setItem("theme", isDark ? "dark" : "light");
    }
</script>

<Tooltip.Root>
    <Tooltip.Trigger>
        {#snippet child({ props }: { props: Record<string, unknown> })}
            <Button
                variant="ghost"
                size="icon"
                aria-label="Toggle theme"
                {...props}
                onclick={(e) => {
                    toggleTheme();
                    if (typeof props.onclick === "function") {
                        props.onclick(e);
                    }
                }}
            >
                {#if isDark}
                    <Sun class="h-[1.2rem] w-[1.2rem] transition-all" />
                {:else}
                    <Moon class="h-[1.2rem] w-[1.2rem] transition-all" />
                {/if}
            </Button>
        {/snippet}
    </Tooltip.Trigger>
    <Tooltip.Content side="bottom">
        {isDark ? "Light Mode" : "Dark Mode"}
    </Tooltip.Content>
</Tooltip.Root>
