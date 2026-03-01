<script lang="ts">
	import { onMount, tick } from "svelte";
	import AppSidebar from "$lib/components/app-sidebar.svelte";
	import ThemeToggle from "$lib/components/theme-toggle.svelte";
	import Titlebar from "$lib/components/Titlebar.svelte";
	import * as Sidebar from "$lib/components/ui/sidebar/index.js";
	import {
		ResizablePaneGroup,
		ResizablePane,
		ResizableHandle,
	} from "$lib/components/ui/resizable/index.js";
	import { Toaster } from "$lib/components/ui/sonner/index.js";
	import * as Tooltip from "$lib/components/ui/tooltip/index.js";
	import EditorActions from "$lib/editor/components/EditorActions.svelte";
	import "./layout.css";

	const { children } = $props();

	let sidebarPane: ReturnType<typeof ResizablePane> | undefined = $state();
	let sidebarOpen = $state(false);

	/** Transition class applied only during programmatic toggle, NOT during drag. */
	let animating = $state(false);

	/**
	 * Collapsible is toggled dynamically:
	 *  - true  → allows programmatic collapse()/expand() and drag-from-collapsed
	 *  - false → drag respects minSize without snap-to-collapse
	 */
	let paneCollapsible = $state(true);

	// Start collapsed instantly (no animation).
	onMount(() => {
		sidebarPane?.collapse();
	});

	/**
	 * Fired by Sidebar.Provider when the user clicks the trigger or
	 * presses the keyboard shortcut (Ctrl+B). Animates the pane.
	 */
	function handleOpenChange(newOpen: boolean) {
		animating = true;
		paneCollapsible = true;

		// Wait one tick so the updated `collapsible` prop reaches paneforge,
		// then perform the collapse/expand with the CSS transition active.
		tick().then(() => {
			if (newOpen) {
				sidebarPane?.expand();
			} else {
				sidebarPane?.collapse();
			}
			setTimeout(() => {
				animating = false;
			}, 210);
		});
	}

	/** Pane collapsed via drag → sync sidebar UI state. */
	function handlePaneCollapse() {
		sidebarOpen = false;
		paneCollapsible = true; // keep true so future expand() works
	}

	/** Pane expanded via drag → sync sidebar UI state, disable drag-collapse. */
	function handlePaneExpand() {
		sidebarOpen = true;
		paneCollapsible = false; // prevent accidental drag-to-collapse
	}
</script>

<Tooltip.Provider delayDuration={400}>
	<div class="flex h-screen w-full flex-col overflow-hidden">
		<Titlebar />
		<div class="relative flex-1 overflow-hidden">
			<Sidebar.Provider
				bind:open={sidebarOpen}
				onOpenChange={handleOpenChange}
				class="h-full min-h-0"
			>
				<ResizablePaneGroup direction="horizontal">
					<ResizablePane
						bind:this={sidebarPane}
						id="sidebar"
						defaultSize={20}
						minSize={15}
						maxSize={30}
						collapsible={paneCollapsible}
						collapsedSize={0}
						onCollapse={handlePaneCollapse}
						onExpand={handlePaneExpand}
						class={animating
							? "transition-[flex-grow] duration-200 ease-linear"
							: ""}
					>
						<div
							class="h-full w-full"
							style="--sidebar-width: 100%;"
						>
							<AppSidebar />
						</div>
					</ResizablePane>
					<ResizableHandle />
					<ResizablePane id="content" defaultSize={80} class="flex flex-col">
						<Sidebar.Inset class="h-full min-w-0">
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
					</ResizablePane>
				</ResizablePaneGroup>
			</Sidebar.Provider>
		</div>
	</div>
</Tooltip.Provider>
<Toaster position="top-right" />
