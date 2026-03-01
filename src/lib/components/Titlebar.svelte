<script lang="ts">
	import { Window } from "@tauri-apps/api/window";
	import { type } from "@tauri-apps/plugin-os";
	import { emit } from "@tauri-apps/api/event";
	import { onMount } from "svelte";
	import * as Menubar from "$lib/components/ui/menubar/index.js";
	import { Minus, Square, X } from "@lucide/svelte";

	let osType = $state("");
	const appWindow = new Window("main");

	const isMac = $derived(osType === "macos");
	/** Platform modifier key label */
	const mod = $derived(isMac ? "Cmd" : "Ctrl");
	/** Redo shortcut differs between platforms */
	const redoShortcut = $derived(isMac ? `${mod}+Shift+Z` : `${mod}+Y`);

	onMount(async () => {
		osType = await type();
	});

	async function handleOpen() {
		await emit("menu://open-file");
	}

	function handleEdit(action: string) {
		document.execCommand(action);
	}
</script>

{#snippet appMenubar()}
	<Menubar.Root class="border-none bg-transparent">
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
	data-tauri-drag-region
	class="flex h-10 w-full select-none items-center justify-between border-b bg-background shadow-sm"
>
	{#if isMac}
		<!-- Mac Traffic Lights -->
		<div
			class="z-10 flex h-full w-[72px] items-center justify-start gap-2 pl-4 cursor-default"
		>
			<button
				class="h-3 w-3 rounded-full border border-red-600/50 bg-red-500 hover:bg-red-600 focus:outline-none"
				onclick={() => appWindow.close()}
				aria-label="Close"
			></button>
			<button
				class="h-3 w-3 rounded-full border border-yellow-600/50 bg-yellow-500 hover:bg-yellow-600 focus:outline-none"
				onclick={() => appWindow.minimize()}
				aria-label="Minimize"
			></button>
			<button
				class="h-3 w-3 rounded-full border border-green-600/50 bg-green-500 hover:bg-green-600 focus:outline-none"
				onclick={() => appWindow.toggleMaximize()}
				aria-label="Maximize"
			></button>
		</div>

		<!-- Menubar (Mac) -->
		<div class="z-10 flex flex-1 justify-start pl-2 overflow-hidden">
			{@render appMenubar()}
		</div>
	{:else}
		<!-- App Name + Menubar (Windows / Linux) -->
		<div class="z-10 flex items-center pl-3">
			<span
				class="pointer-events-none mr-2 text-xs font-semibold tracking-wide"
				>Grayslate</span
			>
			{@render appMenubar()}
		</div>

		<!-- Drag region filler -->
		<div data-tauri-drag-region class="flex-1 h-full"></div>

		<!-- Window Controls (Windows / Linux) -->
		<div class="z-10 flex h-full items-center">
			<button
				class="inline-flex h-full w-12 items-center justify-center text-muted-foreground transition-colors hover:bg-foreground/10 hover:text-foreground focus:outline-none"
				onclick={() => appWindow.minimize()}
				aria-label="Minimize"
			>
				<Minus class="h-4 w-4" />
			</button>
			<button
				class="inline-flex h-full w-12 items-center justify-center text-muted-foreground transition-colors hover:bg-foreground/10 hover:text-foreground focus:outline-none"
				onclick={() => appWindow.toggleMaximize()}
				aria-label="Maximize"
			>
				<Square class="h-3.5 w-3.5" />
			</button>
			<button
				class="inline-flex h-full w-12 items-center justify-center text-muted-foreground transition-colors hover:bg-destructive hover:text-destructive-foreground focus:outline-none"
				onclick={() => appWindow.close()}
				aria-label="Close"
			>
				<X class="h-4 w-4" />
			</button>
		</div>
	{/if}
</div>
