<script lang="ts">
	import { Window } from "@tauri-apps/api/window";
	import { type } from "@tauri-apps/plugin-os";
	import { emit } from "@tauri-apps/api/event";
	import { onMount } from "svelte";
	import * as Menubar from "$lib/components/ui/menubar/index.js";
	import { Maximize2, Minus, Square, X } from "@lucide/svelte";
	import * as Tooltip from "$lib/components/ui/tooltip/index.js";
	import { editorState } from "$lib/state/editor.svelte";
	import {
		editorUndo,
		editorRedo,
		editorCut,
		editorCopy,
		editorPaste,
		editorSelectAll,
	} from "$lib/editor/core/actions";

	let osType = $state("");
	const appWindow = new Window("main");

	const isMac = $derived(osType === "macos");
	/** Platform modifier key label */
	const mod = $derived(isMac ? "⌘" : "Ctrl");
	/** Redo shortcut differs between platforms */
	const redoShortcut = $derived(isMac ? `${mod}+Shift+Z` : `${mod}+Y`);

	onMount(async () => {
		osType = await type();
	});

	async function handleOpen() {
		await emit("menu://open-file");
	}

	async function handleEdit(action: string) {
		const view = editorState.activeView;
		if (!view) return;

		switch (action) {
			case "undo":
				editorUndo(view);
				break;
			case "redo":
				editorRedo(view);
				break;
			case "cut":
				await editorCut(view);
				break;
			case "copy":
				await editorCopy(view);
				break;
			case "paste":
				await editorPaste(view);
				break;
			case "selectAll":
				editorSelectAll(view);
				break;
		}
	}
</script>

{#snippet appMenubar()}
	<Menubar.Root class="pointer-events-auto border-none bg-transparent">
		<Menubar.Menu>
			<Menubar.Trigger class="cursor-pointer">File</Menubar.Trigger>
			<Menubar.Content>
				<Menubar.Item onclick={handleOpen}>
					Open File...
					<Menubar.Shortcut>{mod}+O</Menubar.Shortcut>
				</Menubar.Item>
			</Menubar.Content>
		</Menubar.Menu>
		<Menubar.Menu>
			<Menubar.Trigger class="cursor-pointer">Edit</Menubar.Trigger>
			<Menubar.Content>
				<Menubar.Item onclick={() => handleEdit("undo")}
					>Undo<Menubar.Shortcut>{mod}+Z</Menubar.Shortcut
					></Menubar.Item
				>
				<Menubar.Item onclick={() => handleEdit("redo")}
					>Redo<Menubar.Shortcut>{redoShortcut}</Menubar.Shortcut
					></Menubar.Item
				>
				<Menubar.Separator />
				<Menubar.Item onclick={() => handleEdit("cut")}
					>Cut<Menubar.Shortcut>{mod}+X</Menubar.Shortcut
					></Menubar.Item
				>
				<Menubar.Item onclick={() => handleEdit("copy")}
					>Copy<Menubar.Shortcut>{mod}+C</Menubar.Shortcut
					></Menubar.Item
				>
				<Menubar.Item onclick={() => handleEdit("paste")}
					>Paste<Menubar.Shortcut>{mod}+V</Menubar.Shortcut
					></Menubar.Item
				>
				<Menubar.Separator />
				<Menubar.Item onclick={() => handleEdit("selectAll")}
					>Select All<Menubar.Shortcut>{mod}+A</Menubar.Shortcut
					></Menubar.Item
				>
			</Menubar.Content>
		</Menubar.Menu>
	</Menubar.Root>
{/snippet}

<div
	class="relative flex h-10 w-full select-none items-center justify-between border-b bg-background shadow-sm"
>
	<div data-tauri-drag-region class="absolute inset-0 z-0"></div>

	{#if isMac}
		<!-- Mac Traffic Lights -->
		<div
			class="group pointer-events-none z-10 flex h-full w-[72px] items-center justify-start gap-2 pl-4"
		>
			<Tooltip.Root>
				<Tooltip.Trigger>
					<button
						class="pointer-events-auto flex h-3.5 w-3.5 items-center justify-center rounded-full border border-red-600/50 bg-red-500 hover:bg-red-600 focus:outline-none"
						onclick={() => appWindow.close()}
						aria-label="Close"
					>
						<X
							class="pointer-events-none h-2 w-2 opacity-0 transition-opacity group-hover:opacity-100 text-[#4c0000]"
							strokeWidth={2.5}
						/>
					</button>
				</Tooltip.Trigger>
				<Tooltip.Content side="bottom" sideOffset={13}
					>Close</Tooltip.Content
				>
			</Tooltip.Root>

			<Tooltip.Root>
				<Tooltip.Trigger>
					<button
						class="pointer-events-auto flex h-3.5 w-3.5 items-center justify-center rounded-full border border-yellow-600/50 bg-yellow-500 hover:bg-yellow-600 focus:outline-none"
						onclick={() => appWindow.minimize()}
						aria-label="Minimize"
					>
						<Minus
							class="pointer-events-none h-2 w-2 opacity-0 transition-opacity group-hover:opacity-100 text-[#5a4300]"
							strokeWidth={2.5}
						/>
					</button>
				</Tooltip.Trigger>
				<Tooltip.Content side="bottom" sideOffset={13}
					>Minimize</Tooltip.Content
				>
			</Tooltip.Root>

			<Tooltip.Root>
				<Tooltip.Trigger>
					<button
						class="pointer-events-auto flex h-3.5 w-3.5 items-center justify-center rounded-full border border-green-600/50 bg-green-500 hover:bg-green-600 focus:outline-none"
						onclick={() => appWindow.toggleMaximize()}
						aria-label="Maximize"
					>
						<Maximize2
							class="pointer-events-none h-2 w-2 opacity-0 transition-opacity group-hover:opacity-100 text-[#004200]"
							strokeWidth={2.5}
						/>
					</button>
				</Tooltip.Trigger>
				<Tooltip.Content side="bottom" sideOffset={13}
					>Maximize</Tooltip.Content
				>
			</Tooltip.Root>
		</div>

		<!-- Menubar (Mac) -->
		<div
			class="pointer-events-none z-10 flex flex-1 justify-start pl-2 overflow-hidden"
		>
			{@render appMenubar()}
		</div>
	{:else}
		<!-- App Name + Menubar (Windows / Linux) -->
		<div class="pointer-events-none z-10 flex items-center pl-3">
			<span class="mr-2 text-xs font-semibold tracking-wide"
				>Grayslate</span
			>
			{@render appMenubar()}
		</div>

		<!-- Window Controls (Windows / Linux) -->
		<div class="pointer-events-none z-10 flex h-full items-center">
			<Tooltip.Root>
				<Tooltip.Trigger class="h-full">
					<button
						class="pointer-events-auto inline-flex h-full w-12 items-center justify-center text-muted-foreground transition-colors hover:bg-foreground/10 hover:text-foreground focus:outline-none"
						onclick={() => appWindow.minimize()}
						aria-label="Minimize"
					>
						<Minus class="h-4 w-4" />
					</button>
				</Tooltip.Trigger>
				<Tooltip.Content side="bottom" sideOffset={6}
					>Minimize</Tooltip.Content
				>
			</Tooltip.Root>

			<Tooltip.Root>
				<Tooltip.Trigger class="h-full">
					<button
						class="pointer-events-auto inline-flex h-full w-12 items-center justify-center text-muted-foreground transition-colors hover:bg-foreground/10 hover:text-foreground focus:outline-none"
						onclick={() => appWindow.toggleMaximize()}
						aria-label="Maximize"
					>
						<Square class="h-3.5 w-3.5" />
					</button>
				</Tooltip.Trigger>
				<Tooltip.Content side="bottom" sideOffset={6}
					>Maximize</Tooltip.Content
				>
			</Tooltip.Root>

			<Tooltip.Root>
				<Tooltip.Trigger class="h-full">
					<button
						class="pointer-events-auto inline-flex h-full w-12 items-center justify-center text-muted-foreground transition-colors hover:bg-destructive hover:text-destructive-foreground focus:outline-none"
						onclick={() => appWindow.close()}
						aria-label="Close"
					>
						<X class="h-4 w-4" />
					</button>
				</Tooltip.Trigger>
				<Tooltip.Content side="bottom" sideOffset={6}
					>Close</Tooltip.Content
				>
			</Tooltip.Root>
		</div>
	{/if}
</div>
