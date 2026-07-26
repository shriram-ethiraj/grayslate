<script lang="ts">
	import { Tabs as TabsPrimitive } from "bits-ui";
	import { cn } from "$lib/utils.js";

	let {
		ref = $bindable(null),
		class: className,
		children,
		...restProps
	}: TabsPrimitive.ListProps = $props();
</script>

<TabsPrimitive.List
	bind:ref
	data-slot="tabs-list"
	class={cn(
		// `bg-state-track` is the recessed rail the raised active chip sits above.
		"bg-state-track text-muted-foreground inline-flex h-9 w-fit items-center justify-center rounded-lg p-[3px]",
		className,
	)}
	{...restProps}
>
	{@render children?.()}
</TabsPrimitive.List>