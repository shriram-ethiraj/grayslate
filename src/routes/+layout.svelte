<script lang="ts">
	import { onDestroy, onMount } from "svelte";
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
	import {
		appSettingsState,
		loadAllSettings,
		applyTheme,
		hydrateAppSettingsState,
	} from "$lib/state/appSettings.svelte";
	import {
		appMenuState,
		ensureAppInfoLoaded,
		startAutomaticUpdateChecks,
	} from "$lib/state/appMenu.svelte";
	import {
		beginTrackedWork,
		initializeE2ERuntime,
		markE2EReady,
	} from "virtual:grayslate-e2e-runtime";
	import LucideFilePlusCorner from '~icons/lucide/file-plus-corner';
	import "./layout.css";

	const { children } = $props();

	const isE2EMode = import.meta.env.MODE === "e2e";
	const tooltipDelayDuration = isE2EMode ? 0 : 500;
	const tooltipSkipDelayDuration = isE2EMode ? 0 : 300;
	let e2eRuntimeReady = $state(!isE2EMode);
	let e2eApplicationStateReady = $state(!isE2EMode);
	let e2eRuntimeError = $state<string | null>(null);

	let sidebarPane: ReturnType<typeof ResizablePane> | undefined = $state();
	let sidebarPaneElement: HTMLDivElement | null = $state(null);
	let sidebarOpen = $state(false);
	let settingsHydrated = $state(false);
	let appInfoReady = $state(false);

	/** Transition class applied only during programmatic toggle, NOT during drag. */
	let animating = $state(false);
	let expandingProgrammatically = $state(false);
	const SIDEBAR_TRANSITION_RECOVERY_MS = 1_000;
	let sidebarTransitionRecovery: ReturnType<typeof setTimeout> | undefined;
	let finishTrackedSidebarTransition: (() => void) | undefined;

	/** The last non-zero size of the sidebar pane, used to restore after close. */
	let lastExpandedSize = $state(20);

	function finishProgrammaticSidebarTransition(): void {
		if (sidebarTransitionRecovery !== undefined) {
			clearTimeout(sidebarTransitionRecovery);
			sidebarTransitionRecovery = undefined;
		}
		finishTrackedSidebarTransition?.();
		finishTrackedSidebarTransition = undefined;
		animating = false;
		expandingProgrammatically = false;
	}

	function beginProgrammaticSidebarTransition(expanding: boolean): void {
		finishProgrammaticSidebarTransition();
		finishTrackedSidebarTransition = beginTrackedWork("sidebar-programmatic-transition");
		animating = true;
		expandingProgrammatically = expanding;
		// Recovery only: normal completion is driven by transitionend. This
		// handles an unchanged target size or a browser cancelling the event.
		sidebarTransitionRecovery = setTimeout(
			finishProgrammaticSidebarTransition,
			SIDEBAR_TRANSITION_RECOVERY_MS,
		);
	}

	function handleSidebarTransitionEnd(event: TransitionEvent): void {
		if (
			event.target !== sidebarPaneElement ||
			event.currentTarget !== sidebarPaneElement ||
			event.propertyName !== "flex-grow"
		) {
			return;
		}
		finishProgrammaticSidebarTransition();
	}

	/**
	 * Fired by Sidebar.Provider when the user clicks the trigger or
	 * presses the keyboard shortcut (Ctrl+B). Animates the pane.
	 */
	function handleOpenChange(newOpen: boolean) {
		if (settingsHydrated) {
			setSidebarOpen(newOpen);
		}
		beginProgrammaticSidebarTransition(newOpen);
		tick().then(() => {
			if (newOpen) {
				// Restore to the last known width rather than the default.
				sidebarPane?.resize(lastExpandedSize);
			} else {
				sidebarPane?.collapse();
			}
		});
	}

	/** Track the last non-zero size so we can restore it on expand.
	 *  Expansion frames may update it because the debounced final frame is the
	 *  desired width. Collapse frames stay suppressed so a near-zero frame
	 *  cannot overwrite the remembered width.
	 */
	function handlePaneResize(size: number) {
		if (size > 0 && (!animating || expandingProgrammatically)) {
			lastExpandedSize = size;
			if (settingsHydrated) {
				setSidebarWidth(size);
			}
		}
	}

	/** Pane collapsed via drag or programmatic collapse → sync sidebar UI state. */
	function handlePaneCollapse() {
		sidebarOpen = false;
		if (settingsHydrated) {
			setSidebarOpen(false);
		}
	}

	/** Pane expanded via drag or programmatic expand → sync sidebar UI state. */
	function handlePaneExpand() {
		sidebarOpen = true;
		if (settingsHydrated) {
			setSidebarOpen(true);
		}
	}

	/** A real pointer drag takes ownership from the programmatic transition. */
	function handlePaneDraggingChange(isDragging: boolean) {
		if (isDragging) {
			finishProgrammaticSidebarTransition();
		}
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

	// The test-only runtime must exist before any child component mounts and
	// starts IPC. Its lifecycle stays `booting` until platform/settings state is
	// hydrated and the initial layout has committed. Normal development and
	// release bundles resolve the virtual runtime import to a no-op module before
	// Vite constructs the graph, so test-only modules never become build entries.
	onMount(() => {
		let cancelled = false;
		void (async () => {
			try {
				if (isE2EMode) {
					await initializeE2ERuntime();
					// Mount the pane controller while the lifecycle is still
					// `booting`. Persisted layout can then be applied through the
					// same controller path as normal startup, while the driver stays
					// causally blocked until markE2EReady() below.
					e2eRuntimeReady = true;
					await tick();
				}

				await initPlatformState();
				await initAppSettings();
				try {
					await ensureAppInfoLoaded();
				} catch (error) {
					console.warn("[App bootstrap] Failed to load app/update information:", error);
				} finally {
					appInfoReady = true;
				}
				if (cancelled) return;
				if (isE2EMode) {
					// CodeMirror must be created from hydrated preferences. The pane
					// shell is already mounted above, but the page/editor subtree stays
					// absent until font, wrapping, and format defaults are authoritative.
					e2eApplicationStateReady = true;
					await tick();
				}

				await applyHydratedSidebarLayout();
				await tick();
				if (cancelled) return;
				markE2EReady();
			} catch (error) {
				if (cancelled) return;
				if (isE2EMode) {
					const message = error instanceof Error ? error.message : String(error);
					console.error("[E2E bootstrap] Failed to initialize the test runtime:", error);
					e2eRuntimeError = message;
					return;
				}
				console.warn("[App bootstrap] Failed to initialize application state:", error);
			}
		})();
		return () => {
			cancelled = true;
		};
	});

	onDestroy(finishProgrammaticSidebarTransition);

	async function initAppSettings() {
		try {
			const settings = await loadAllSettings();

			editorState.fontSize = settings.fontSize;
			editorState.wordWrap = settings.wordWrap;
			uiState.sidebar.width = settings.sidebarWidth;
			lastExpandedSize = settings.sidebarWidth;
			uiState.sidebar.open = settings.sidebarOpen;
			sidebarOpen = settings.sidebarOpen;
			settingsHydrated = true;

			// Populate the user-facing preferences consumed by the Settings dialog,
			// the editor's default-indent seed, and the delete-confirmation branch.
			hydrateAppSettingsState(settings);

			// Reconcile theme: SQLite is authoritative over localStorage.
			const isDark = settings.theme === "dark";
			if (document.documentElement.classList.contains("dark") !== isDark) {
				applyTheme(isDark);
			}

		} catch (error) {
			console.warn("[AppSettings] Failed to load settings:", error);
			// Keep sidebar controls functional when settings hydration fails.
			settingsHydrated = true;
		}
	}

	async function applyHydratedSidebarLayout(): Promise<void> {
		if (!sidebarOpen) return;
		// Expanding a collapsed pane first emits its minimum size. Keep the
		// hydrated target in a local so that intermediate resize notification
		// cannot replace the width we are about to restore.
		const hydratedWidth = lastExpandedSize;
		beginProgrammaticSidebarTransition(true);
		await tick();
		sidebarPane?.expand();
		await tick();
		sidebarPane?.resize(hydratedWidth);
	}

	$effect(() => {
		if (
			!settingsHydrated ||
			!appSettingsState.automaticUpdateChecks ||
			appMenuState.updatePolicy !== "self-update"
		) {
			return;
		}

		return startAutomaticUpdateChecks();
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

{#if e2eRuntimeError}
<main
	class="flex h-screen w-full items-center justify-center overflow-hidden p-6 text-sm text-destructive"
	data-testid="e2e-bootstrap-error"
	role="alert"
>
	E2E runtime failed to initialize: {e2eRuntimeError}
</main>
{:else if e2eRuntimeReady}
<Tooltip.Provider
	delayDuration={tooltipDelayDuration}
	skipDelayDuration={tooltipSkipDelayDuration}
	disableHoverableContent
>
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
					bind:ref={sidebarPaneElement}
					id="sidebar"
					defaultSize={0}
					minSize={15}
					maxSize={30}
					collapsible={true}
					collapsedSize={0}
					onCollapse={handlePaneCollapse}
					onExpand={handlePaneExpand}
					onResize={handlePaneResize}
					ontransitionend={handleSidebarTransitionEnd}
					class={animating
						? "transition-[flex-grow] duration-200 ease-linear"
						: ""}
				>
					<div class="h-full w-full" style="--sidebar-width: 100%;">
						<AppSidebar />
					</div>
				</ResizablePane>
				<ResizableHandle
					data-testid="sidebar-resize-handle"
					onDraggingChange={handlePaneDraggingChange}
				/>
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
									data-testid="header-new-slate"
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
							{#if platformState.ready && appInfoReady && e2eApplicationStateReady}
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
{/if}
