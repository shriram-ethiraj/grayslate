<script lang="ts">
	import { onMount } from "svelte";
	import { tick } from "svelte";
	import AppSidebar from "$lib/components/app-sidebar.svelte";
	import ThemeToggle from "$lib/components/theme-toggle.svelte";
	import Titlebar from "$lib/components/Titlebar.svelte";
	import * as Sidebar from "$lib/components/ui/sidebar/index.js";
	import * as Tooltip from "$lib/components/ui/tooltip/index.js";
	import { TooltipButton } from "$lib/components/ui/tooltip/index.js";
	import {
		editorState,
		openFindReplacePanel,
		openGoToLinePanel,
	} from "$lib/state/editor.svelte";
	import { uiState, setSidebarWidth, setSidebarOpen } from "$lib/state/ui.svelte";
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
	import { formatShortcutTooltip } from "$lib/shortcuts";
	import { loadAllSettings, applyTheme, hydrateAppSettingsState } from "$lib/state/appSettings.svelte";
	import LucideFilePlusCorner from '~icons/lucide/file-plus-corner';
	import "./layout.css";

	const { children } = $props();

	// The WebdriverIO guest bridge is bundled and initialized only by the
	// dedicated `vite --mode e2e` build. Normal development and release bundles
	// do not import this test-only command surface.
	onMount(() => {
		if (import.meta.env.MODE === "e2e") {
			void import("@wdio/tauri-plugin");
		}
	});

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
		setSidebarOpen(newOpen);
		animating = true;
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
			setSidebarWidth(size);
		}
	}

	/** Pane collapsed via drag or programmatic collapse → sync sidebar UI state. */
	function handlePaneCollapse() {
		sidebarOpen = false;
		setSidebarOpen(false);
	}

	/** Pane expanded via drag or programmatic expand → sync sidebar UI state. */
	function handlePaneExpand() {
		sidebarOpen = true;
		setSidebarOpen(true);
	}

	async function handleNewFile() {
		await emit("menu://new-file");
	}

	const isNewFileDisabled = $derived(
		editorState.isUntitledDocument && editorState.currentDocumentLength === 0,
	);

	function editorOwnsActiveElement(): boolean {
		const activeView = editorState.activeView;
		const activeElement = document.activeElement;

		return !!activeView && !!activeElement && activeView.dom.contains(activeElement);
	}

	onMount(() => {
		void initPlatformState();
		void initAppSettings();
	});

	async function initAppSettings() {
		try {
			const settings = await loadAllSettings();

			editorState.fontSize = settings.fontSize;
			editorState.wordWrap = settings.wordWrap;
			uiState.sidebar.width = settings.sidebarWidth;
			lastExpandedSize = settings.sidebarWidth;
			uiState.sidebar.open = settings.sidebarOpen;

			// Populate the user-facing preferences consumed by the Settings dialog,
			// the editor's default-indent seed, and the delete-confirmation branch.
			hydrateAppSettingsState(settings);

			// Reconcile theme: SQLite is authoritative over localStorage.
			const isDark = settings.theme === "dark";
			if (document.documentElement.classList.contains("dark") !== isDark) {
				applyTheme(isDark);
			}

			// Expand sidebar if the saved state says it should be open.
			if (settings.sidebarOpen) {
				animating = true;
				await tick();
				sidebarPane?.resize(settings.sidebarWidth);
				setTimeout(() => {
					animating = false;
				}, 210);
			}
		} catch (error) {
			console.warn("[AppSettings] Failed to load settings:", error);
		}
	}

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

<Tooltip.Provider delayDuration={500} skipDelayDuration={300} disableHoverableContent>
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
								<Sidebar.Trigger data-testid="sidebar-toggle" class="-ml-1" />
								<TooltipButton
									variant="ghost"
									size="icon"
									aria-label="New slate"
									tooltip={formatShortcutTooltip("New slate", "new-slate", platformState.osType)}
									disabledTooltip="Already on a blank slate"
									disabled={isNewFileDisabled}
									onclick={() => {
										void handleNewFile();
									}}
								>
									<LucideFilePlusCorner class="size-4 transition-all" />
								</TooltipButton>
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
</Tooltip.Provider>
