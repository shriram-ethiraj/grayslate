<script lang="ts">
	import { onMount } from "svelte";
	import { tick } from "svelte";
	import AppSidebar from "$lib/components/app-sidebar.svelte";
	import ThemeToggle from "$lib/components/theme-toggle.svelte";
	import Titlebar from "$lib/components/Titlebar.svelte";
	import * as Sidebar from "$lib/components/ui/sidebar/index.js";
	import { Button } from "$lib/components/ui/button/index.js";
	import {
		editorState,
		openFindReplacePanel,
		openGoToLinePanel,
	} from "$lib/state/editor.svelte";
	import {
		ResizablePaneGroup,
		ResizablePane,
		ResizableHandle,
	} from "$lib/components/ui/resizable/index.js";
	import { emit } from "@tauri-apps/api/event";
	import { Toaster } from "$lib/components/ui/sonner/index.js";
	import EditorActions from "$lib/editor/components/EditorActions.svelte";
	import { registerHotkeys } from "$lib/hotkeys";
	import { initPlatformState, platformState } from "$lib/state/platform.svelte";
	import LucideFilePlusCorner from '~icons/lucide/file-plus-corner';
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

	async function handleNewFile() {
		await emit("menu://new-file");
	}

	const isNewFileDisabled = $derived(
		editorState.isUntitledDocument && editorState.currentDocumentLength === 0,
	);

	const currentFileName = $derived.by(() => {
		if (!editorState.currentFilePath) return "Untitled";
		const parts = editorState.currentFilePath.split(/[\\/]/);
		return parts[parts.length - 1] || "Untitled";
	});

	function editorOwnsActiveElement(): boolean {
		const activeView = editorState.activeView;
		const activeElement = document.activeElement;

		return !!activeView && !!activeElement && activeView.dom.contains(activeElement);
	}

	onMount(() => {
		void initPlatformState();
	});

	$effect(() => {
		return registerHotkeys([
			{
				key: "Mod+F",
				callback: () => {
					if (editorOwnsActiveElement()) return;
					openFindReplacePanel(false);
				},
				options: { ignoreInputs: false, stopPropagation: false },
			},
			{
				key: "Mod+Alt+F",
				callback: () => {
					if (editorOwnsActiveElement()) return;
					openFindReplacePanel(true);
				},
				options: { ignoreInputs: false, stopPropagation: false },
			},
			{
				key: "Mod+H",
				callback: () => {
					if (editorOwnsActiveElement()) return;
					openFindReplacePanel(true);
				},
				options: { ignoreInputs: false, stopPropagation: false },
			},
			{
				key: "Mod+G",
				callback: () => {
					if (editorOwnsActiveElement()) return;
					openGoToLinePanel();
				},
				options: { ignoreInputs: false, stopPropagation: false },
			},
		]);
	});
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
							class="relative flex h-12 w-full shrink-0 items-center justify-between border-b bg-background px-4"
						>
							<div class="relative z-10 flex items-center gap-1">
								<Sidebar.Trigger class="-ml-1" />
								<Button
									variant="ghost"
									size="icon"
									aria-label="New slate"
									title={isNewFileDisabled ? "Already on a new slate" : "New slate"}
									disabled={isNewFileDisabled}
									onclick={() => {
										void handleNewFile();
									}}
								>
									<LucideFilePlusCorner class="h-[1.2rem] w-[1.2rem] transition-all" />
								</Button>
							</div>
							<!-- Centered file name -->
							<div class="pointer-events-none absolute inset-0 flex items-center justify-center">
								<span
									class="pointer-events-auto max-w-[40%] truncate text-sm font-semibold text-foreground text-opacity-90"
									title={editorState.currentFilePath ?? currentFileName}
								>{currentFileName}</span>
							</div>
							<div class="relative z-10 flex items-center gap-2">
								<EditorActions />
								<ThemeToggle />
							</div>
						</header>
						<div class="flex min-h-0 min-w-0 flex-1 flex-col">
							{#if platformState.ready}
								{@render children()}
							{/if}
						</div>
					</Sidebar.Inset>
				</ResizablePane>
			</ResizablePaneGroup>
		</Sidebar.Provider>
	</div>
</div>
<Toaster position="top-right" offset={{ top: "96px", right: "24px" }} mobileOffset={{ top: "96px", right: "16px", left: "16px" }} />
