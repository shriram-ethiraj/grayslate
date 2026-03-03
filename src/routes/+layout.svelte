<script lang="ts">
	import { tick } from "svelte";
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
	import EditorActions from "$lib/editor/components/EditorActions.svelte";
	import "./layout.css";

	const { children } = $props();

	let sidebarPane: ReturnType<typeof ResizablePane> | undefined = $state();
	let sidebarOpen = $state(false);

	/** Transition class applied only during programmatic toggle, NOT during drag. */
	let animating = $state(false);

	/** The last non-zero size of the sidebar pane, used to restore after close. */
	let lastExpandedSize = $state(20);

	/**
	 * Fired by Sidebar.Provider when the user clicks the trigger or
	 * presses the keyboard shortcut (Ctrl+B). Animates the pane.
	 */
	function handleOpenChange(newOpen: boolean) {
		animating = true;

		// Wait one tick so the animating class reaches the DOM,
		// then perform the collapse/expand with the CSS transition active.
		tick().then(() => {
			if (newOpen) {
				// Restore to the last known width rather than the default.
				sidebarPane?.resize(lastExpandedSize);
			} else {
				sidebarPane?.collapse();
			}
			setTimeout(() => {
				animating = false;
			}, 210);
		});
	}

	/** Track the last non-zero size so we can restore it on expand.
	 *  Only update during user drag (animating === false) to prevent
	 *  intermediate collapse/expand transition sizes from overwriting it.
	 */
	function handlePaneResize(size: number) {
		if (size > 0 && !animating) {
			lastExpandedSize = size;
		}
	}

	/** Pane collapsed via drag or programmatic collapse → sync sidebar UI state. */
	function handlePaneCollapse() {
		sidebarOpen = false;
	}

	/** Pane expanded via drag or programmatic expand → sync sidebar UI state. */
	function handlePaneExpand() {
		sidebarOpen = true;
	}
</script>

<div class="flex h-screen w-full flex-col overflow-hidden">
	<Titlebar />
	<!-- Sidebar.Provider supplies open/close state & Ctrl+B shortcut.
				 Actual sizing is handled by paneforge (ResizablePane), NOT the
				 shadcn Sidebar component. The Sidebar.Sidebar component is omitted;
				 only Provider (state), Trigger (button), and Inset (wrapper) are used.
				 The "--sidebar-width: 100%" override lets AppSidebar fill whatever
				 width paneforge allocates to the sidebar pane. -->
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
					defaultSize={0}
					minSize={15}
					maxSize={30}
					collapsible={true}
					collapsedSize={0}
					onCollapse={handlePaneCollapse}
					onExpand={handlePaneExpand}
					onResize={handlePaneResize}
					class={animating
						? "transition-[flex-grow] duration-200 ease-linear"
						: ""}
				>
					<div class="h-full w-full" style="--sidebar-width: 100%;">
						<AppSidebar />
					</div>
				</ResizablePane>
				<ResizableHandle />
				<ResizablePane
					id="content"
					defaultSize={100}
					class="flex flex-col"
				>
					<Sidebar.Inset class="min-w-0 min-h-0 overflow-hidden">
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
<Toaster position="top-right" />
