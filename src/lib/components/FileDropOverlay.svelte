<script lang="ts">
	import { onMount } from "svelte";
	import {
		getCurrentWebview,
		type DragDropEvent,
	} from "@tauri-apps/api/webview";
	import LucideFileDown from "~icons/lucide/file-down";
	import { toast } from "$lib/components/ui/sonner";
	import { appDialogsState } from "$lib/state/appDialogs.svelte";

	let visible = $state(false);
	let fileCount = $state(0);

	const title = $derived(
		fileCount === 1
			? "Open in Grayslate"
			: `Open ${fileCount} files in Grayslate`,
	);
	const description = $derived(
		appDialogsState.active.type !== "none"
			? fileCount === 1
				? "Release now — it’ll open when the dialog closes"
				: "Release now — they’ll be queued until the dialog closes"
			: fileCount === 1
				? "Release anywhere in this window"
				: "The last file opens here; the others open in separate windows",
	);

	function handleDragDrop(event: DragDropEvent): void {
		switch (event.type) {
			case "enter":
				fileCount = event.paths.length;
				visible = fileCount > 0;
				break;
			case "over":
				break;
			case "leave":
				visible = false;
				fileCount = 0;
				break;
			case "drop":
				visible = false;
				fileCount = 0;
				if (event.paths.length > 0 && appDialogsState.active.type !== "none") {
					toast.info("Files queued until the current dialog closes.", {
						id: "file-drop-queued",
					});
				}
				break;
		}
	}

	onMount(() => {
		let disposed = false;
		let unlisten: (() => void) | undefined;

		void getCurrentWebview()
			.onDragDropEvent((event) => handleDragDrop(event.payload))
			.then((stopListening) => {
				if (disposed) {
					stopListening();
					return;
				}
				unlisten = stopListening;
			})
			.catch((error: unknown) => {
				console.warn("[File Drop] Failed to register native drop feedback:", error);
			});

		return () => {
			disposed = true;
			visible = false;
			unlisten?.();
		};
	});
</script>

{#if visible}
	<div
		class="pointer-events-none absolute inset-0 z-40 flex items-center justify-center bg-background/45 p-6 backdrop-blur-sm"
		data-testid="file-drop-overlay"
		role="status"
		aria-live="polite"
		aria-atomic="true"
	>
		<div
			class="absolute inset-2 rounded-xl border border-dashed border-primary/40"
			aria-hidden="true"
		></div>
		<div
			class="relative w-full max-w-md rounded-lg bg-background/90 p-6 text-center shadow-lg ring-1 ring-inset ring-border backdrop-blur-md"
		>
			<div class="flex flex-col items-center gap-3">
				<div
					class="flex size-9 items-center justify-center rounded-lg bg-primary/10 text-primary ring-1 ring-inset ring-primary/20"
				>
					<LucideFileDown class="size-4" />
				</div>
				<div class="flex flex-col items-center gap-1">
					<p class="text-lg font-semibold text-foreground">{title}</p>
					<p class="text-sm text-muted-foreground">{description}</p>
				</div>
			</div>
		</div>
	</div>
{/if}
