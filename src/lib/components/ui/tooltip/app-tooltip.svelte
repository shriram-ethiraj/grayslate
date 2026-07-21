<script lang="ts">
	import type { Snippet } from "svelte";
	import { mergeProps, useId, type Tooltip as TooltipPrimitive } from "bits-ui";
	import * as Tooltip from "./index.js";

	interface TriggerSnippetProps {
		props: Record<string, unknown>;
	}

	interface Props {
		content: string;
		trigger: Snippet<[TriggerSnippetProps]>;
		side?: TooltipPrimitive.ContentProps["side"];
		align?: TooltipPrimitive.ContentProps["align"];
		contentClass?: string;
		triggerTabindex?: number;
	}

	let {
		content,
		trigger,
		side = "top",
		align = "center",
		contentClass,
		triggerTabindex,
	}: Props = $props();

	const descriptionId = `${useId()}-tooltip-description`;
</script>

<Tooltip.Root>
	<Tooltip.Trigger tabindex={triggerTabindex}>
		{#snippet child({ props })}
			{@render trigger({ props: mergeProps(props, { "aria-describedby": descriptionId }) })}
		{/snippet}
	</Tooltip.Trigger>
	<Tooltip.Content {side} {align} class={contentClass} {descriptionId}>{content}</Tooltip.Content>
</Tooltip.Root>
