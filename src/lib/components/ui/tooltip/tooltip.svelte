<script lang="ts">
	import { Tooltip as TooltipPrimitive } from "bits-ui";
	import {
		isSharedTooltipOpenSuppressed,
		tooltipLifecycle,
	} from "./tooltip-lifecycle.svelte.js";

	let {
		open = $bindable(false),
		disabled = false,
		onOpenChange,
		...restProps
	}: TooltipPrimitive.RootProps = $props();

	const effectiveDisabled = $derived(disabled || !tooltipLifecycle.active);

	function handleOpenChange(nextOpen: boolean): void {
		if (
			nextOpen &&
			(!tooltipLifecycle.active || isSharedTooltipOpenSuppressed())
		) {
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
	bind:open
	disabled={effectiveDisabled}
	onOpenChange={handleOpenChange}
	{...restProps}
/>
