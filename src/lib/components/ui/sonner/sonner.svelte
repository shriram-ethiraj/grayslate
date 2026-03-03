<script lang="ts">
	import CircleCheckIcon from "~icons/lucide/circle-check";
	import InfoIcon from "~icons/lucide/info";
	import Loader2Icon from "~icons/lucide/loader-2";
	import OctagonXIcon from "~icons/lucide/octagon-x";
	import TriangleAlertIcon from "~icons/lucide/triangle-alert";

	import {
		Toaster as Sonner,
		type ToasterProps as SonnerProps,
	} from "svelte-sonner";
	import { mode } from "mode-watcher";

	let { ...restProps }: SonnerProps = $props();
</script>

<Sonner
	theme={mode.current}
	class="toaster group"
	style="--normal-bg: var(--color-popover); --normal-text: var(--color-popover-foreground); --normal-border: var(--color-border);"
	{...restProps}
	>{#snippet loadingIcon()}
		<Loader2Icon class="size-4 animate-spin" />
	{/snippet}
	{#snippet successIcon()}
		<CircleCheckIcon class="size-4" />
	{/snippet}
	{#snippet errorIcon()}
		<OctagonXIcon class="size-4" />
	{/snippet}
	{#snippet infoIcon()}
		<InfoIcon class="size-4" />
	{/snippet}
	{#snippet warningIcon()}
		<TriangleAlertIcon class="size-4" />
	{/snippet}
</Sonner>
