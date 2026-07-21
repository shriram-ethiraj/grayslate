<script lang="ts">
	import { Tooltip as TooltipPrimitive } from "bits-ui";
	import { cn } from "$lib/utils.js";
	import TooltipPortal from "./tooltip-portal.svelte";

	let {
		ref = $bindable(null),
		class: className,
		sideOffset = 6,
		descriptionId,
		children,
		...restProps
	}: TooltipPrimitive.ContentProps & { descriptionId?: string } = $props();
</script>

<TooltipPortal>
	<TooltipPrimitive.Content
		bind:ref
		data-slot="tooltip-content"
		{sideOffset}
		class={cn(
			"z-50 max-w-xs break-words rounded-lg border border-border bg-popover px-2 py-1.5 font-sans text-xs text-popover-foreground shadow-md data-[state=delayed-open]:animate-in data-[state=instant-open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=delayed-open]:fade-in-0 data-[state=instant-open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=delayed-open]:zoom-in-95 data-[state=instant-open]:zoom-in-95",
			className,
		)}
		{...restProps}
	>
		<span id={descriptionId} role="tooltip">{@render children?.()}</span>
	</TooltipPrimitive.Content>
</TooltipPortal>
