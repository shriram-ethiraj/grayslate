<script lang="ts">
	import "./layout.css";
	import * as Sidebar from "$lib/components/ui/sidebar/index.js";
	import AppSidebar from "$lib/components/app-sidebar.svelte";
	import ThemeToggle from "$lib/components/theme-toggle.svelte";

	const { children } = $props();
</script>

<Sidebar.Provider>
	<AppSidebar />
	<Sidebar.Inset>
		<header
			class="flex h-12 w-full items-center justify-between border-b bg-background px-4"
		>
			<Sidebar.Trigger class="-ml-1" />
			<ThemeToggle />
		</header>
		<div class="flex flex-1 flex-col min-h-0">
			{@render children()}
		</div>
	</Sidebar.Inset>
</Sidebar.Provider>
