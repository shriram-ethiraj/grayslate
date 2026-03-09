<script lang="ts">
	import { Tabs as TabsPrimitive } from "bits-ui";

	let {
		value = $bindable(),
		children,
		...restProps
	}: TabsPrimitive.RootProps = $props();
</script>

<TabsPrimitive.Root bind:value={value as never} data-slot="tabs" {...restProps}>
	{@render children?.()}
</TabsPrimitive.Root>