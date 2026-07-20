<script lang="ts">
	import { TooltipButton } from "$lib/components/ui/tooltip/index.js";
	import { cn } from "$lib/utils.js";
	import PanelLeftIcon from "~icons/lucide/panel-left";
	import type { ComponentProps } from "svelte";
	import { formatShortcutTooltip } from "$lib/shortcuts";
	import { platformState } from "$lib/state/platform.svelte";
	import { useSidebar } from "./context.svelte.js";

	type Props = Omit<ComponentProps<typeof TooltipButton>, "tooltip">;

	let {
		ref = $bindable(null),
		class: className,
		onclick,
		...restProps
	}: Props & {
		onclick?: (e: MouseEvent) => void;
	} = $props();

	const sidebar = useSidebar();
</script>

<TooltipButton
	data-sidebar="trigger"
	data-slot="sidebar-trigger"
	variant="ghost"
	size="icon"
	class={cn("size-7", className)}
	type="button"
	tooltip={formatShortcutTooltip(
		sidebar.state === "expanded" ? "Collapse sidebar" : "Expand sidebar",
		"toggle-sidebar",
		platformState.osType,
	)}
	{...restProps}
	onclick={(e) => {
		onclick?.(e);
		sidebar.toggle();
	}}
>
	<PanelLeftIcon />
	<span class="sr-only">Toggle Sidebar</span>
</TooltipButton>
