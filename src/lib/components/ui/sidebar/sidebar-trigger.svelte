<script lang="ts">
	import { Button } from "$lib/components/ui/button/index.js";
	import { cn } from "$lib/utils.js";
	import PanelLeftIcon from "@lucide/svelte/icons/panel-left";
	import type { ComponentProps } from "svelte";
	import { useSidebar } from "./context.svelte.js";

	import * as Tooltip from "$lib/components/ui/tooltip/index.js";

	let {
		ref = $bindable(null),
		class: className,
		onclick,
		...restProps
	}: ComponentProps<typeof Button> & {
		onclick?: (e: MouseEvent) => void;
	} = $props();

	const sidebar = useSidebar();
</script>

<Tooltip.Root>
	<Tooltip.Trigger>
		{#snippet child({ props }: { props: Record<string, unknown> })}
			<Button
				data-sidebar="trigger"
				data-slot="sidebar-trigger"
				variant="ghost"
				size="icon"
				class={cn("size-7", className)}
				type="button"
				{...restProps}
				{...props}
				onclick={(e) => {
					onclick?.(e);
					sidebar.toggle();
				}}
			>
				<PanelLeftIcon />
				<span class="sr-only">Toggle Sidebar</span>
			</Button>
		{/snippet}
	</Tooltip.Trigger>
	<Tooltip.Content side="right">
		{sidebar.state === "expanded" ? "Collapse Sidebar" : "Expand Sidebar"}
	</Tooltip.Content>
</Tooltip.Root>
