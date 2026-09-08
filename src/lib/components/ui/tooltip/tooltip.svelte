<script lang="ts">
	import { Tooltip as TooltipPrimitive } from "bits-ui";
	import { tooltipLifecycle } from "./tooltip-lifecycle.svelte.js";

	let {
		open = $bindable(false),
		disabled = false,
		onOpenChange,
		...restProps
	}: TooltipPrimitive.RootProps = $props();

	const effectiveOpen = $derived(tooltipLifecycle.active && open);
	const effectiveDisabled = $derived(disabled || !tooltipLifecycle.active);

	function handleOpenChange(nextOpen: boolean): void {
		if (nextOpen && !tooltipLifecycle.active) {
			open = false;
			return;
		}

		open = nextOpen;
		onOpenChange?.(nextOpen);
	}

	$effect(() => {
		if (tooltipLifecycle.active || !open) return;
		open = false;
		onOpenChange?.(false);
	});
</script>

<TooltipPrimitive.Root
	open={effectiveOpen}
	disabled={effectiveDisabled}
	onOpenChange={handleOpenChange}
	{...restProps}
/>
