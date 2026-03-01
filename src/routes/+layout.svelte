<script lang="ts">
	import "./layout.css";
	import * as Sidebar from "$lib/components/ui/sidebar/index.js";
	import AppSidebar from "$lib/components/app-sidebar.svelte";
	import ThemeToggle from "$lib/components/theme-toggle.svelte";
	import EditorActions from "$lib/editor/components/EditorActions.svelte";
	import { Toaster } from "$lib/components/ui/sonner/index.js";
	import Titlebar from "$lib/components/Titlebar.svelte";
	import * as Tooltip from "$lib/components/ui/tooltip/index.js";

	const { children } = $props();
</script>

<Tooltip.Provider delayDuration={400}>
	<div class="flex h-screen w-full flex-col overflow-hidden">
		<Titlebar />
		<div class="relative flex-1 overflow-hidden">
			<Sidebar.Provider open={false}>
				<AppSidebar />
				<Sidebar.Inset class="min-w-0">
					<header
						class="flex h-12 w-full shrink-0 items-center justify-between border-b bg-background px-4"
					>
						<Sidebar.Trigger class="-ml-1" />
						<div class="flex items-center gap-2">
							<EditorActions />
							<ThemeToggle />
						</div>
					</header>
					<div class="flex min-h-0 min-w-0 flex-1 flex-col">
						{@render children()}
					</div>
				</Sidebar.Inset>
			</Sidebar.Provider>
		</div>
	</div>
</Tooltip.Provider>
<Toaster position="top-right" />
