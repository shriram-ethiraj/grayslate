<script lang="ts">
	import { Tooltip as TooltipPrimitive } from "bits-ui";
	import { registerTooltipTriggerLifecycle } from "./tooltip-lifecycle.svelte.js";

	let { ref = $bindable(null), ...restProps }: TooltipPrimitive.TriggerProps = $props();

	$effect(() => {
		const trigger = ref;
		if (!trigger) return;
		return registerTooltipTriggerLifecycle(trigger);
	});
</script>

<TooltipPrimitive.Trigger bind:ref data-slot="tooltip-trigger" {...restProps} />
