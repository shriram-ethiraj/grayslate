<script lang="ts">
	import GripVerticalIcon from "~icons/lucide/grip-vertical";
	import * as ResizablePrimitive from "paneforge";
	import { cn, type WithoutChildrenOrChild } from "$lib/utils.js";
	import { platformState } from "$lib/state/platform.svelte";

	let {
		ref = $bindable(null),
		class: className,
		withHandle = false,
		...restProps
	}: WithoutChildrenOrChild<ResizablePrimitive.PaneResizerProps> & {
		withHandle?: boolean;
	} = $props();

	const useMacCursorFix = $derived(platformState.osType === "macos");
</script>

<ResizablePrimitive.PaneResizer
	bind:ref
	data-slot="resizable-handle"
	data-macos-cursor-fix={useMacCursorFix ? "true" : undefined}
	class={cn(
		"bg-border focus-visible:ring-ring relative z-10 flex w-px items-center justify-center after:absolute after:inset-y-0 after:left-[-6px] after:right-[-6px] focus-visible:ring-1 focus-visible:ring-offset-1 focus-visible:outline-hidden data-[direction=vertical]:h-px data-[direction=vertical]:w-full data-[direction=vertical]:after:inset-x-0 data-[direction=vertical]:after:top-[-6px] data-[direction=vertical]:after:bottom-[-6px] [&[data-direction=vertical]>div]:rotate-90",
		className,
	)}
	{...restProps}
>
	{#if withHandle}
		<div
			class="bg-border z-10 flex h-4 w-3 items-center justify-center rounded-xs border"
		>
			<GripVerticalIcon class="size-2.5" />
		</div>
	{/if}
</ResizablePrimitive.PaneResizer>

<style>
	/* paneforge injects cursor:ew-resize as an inline style; !important beats it.
	   Limit the override to macOS because Windows/WebView2 renders the
	   forced col-resize cursor incorrectly after sidebar resizing. */
	:global([data-slot="resizable-handle"][data-macos-cursor-fix="true"]) {
		cursor: col-resize !important;
	}
	:global([data-slot="resizable-handle"][data-direction="vertical"][data-macos-cursor-fix="true"]) {
		cursor: row-resize !important;
	}
</style>
