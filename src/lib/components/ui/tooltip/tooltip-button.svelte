<script lang="ts">
	import { mergeProps } from "bits-ui";
	import type { HTMLButtonAttributes } from "svelte/elements";
	import {
		buttonVariants,
		type ButtonSize,
		type ButtonVariant,
	} from "$lib/components/ui/button/index.js";
	import { cn } from "$lib/utils.js";
	import type { WithElementRef } from "$lib/utils.js";
	import AppTooltip from "./app-tooltip.svelte";

	type TooltipButtonClickEvent = MouseEvent & {
		currentTarget: EventTarget & HTMLButtonElement;
	};

	type Props = Omit<WithElementRef<HTMLButtonAttributes, HTMLButtonElement>, "onclick"> & {
		tooltip: string;
		disabledTooltip?: string;
		tooltipSide?: "top" | "right" | "bottom" | "left";
		tooltipClass?: string;
		onclick?: (event: TooltipButtonClickEvent) => void;
		variant?: ButtonVariant;
		size?: ButtonSize;
	};

	let {
		tooltip,
		disabledTooltip,
		tooltipSide = "top",
		tooltipClass,
		disabled = false,
		onclick,
		class: className,
		variant = "default",
		size = "default",
		type = "button",
		ref = $bindable(null),
		children,
		...restProps
	}: Props = $props();

	const tooltipContent = $derived(disabled && disabledTooltip ? disabledTooltip : tooltip);
</script>

<AppTooltip content={tooltipContent} side={tooltipSide} contentClass={tooltipClass}>
	{#snippet trigger({ props })}
		{@const buttonProps = mergeProps(props, restProps, {
			"aria-disabled": disabled ? "true" : undefined,
			class: cn(
				buttonVariants({ variant, size }),
				"aria-disabled:pointer-events-auto aria-disabled:cursor-default",
				className,
			),
			type,
			onclick: (event: TooltipButtonClickEvent) => {
				if (disabled) {
					event.preventDefault();
					event.stopPropagation();
					return;
				}
				onclick?.(event);
			},
		})}
		<button bind:this={ref} data-slot="button" {...buttonProps}>
			{@render children?.()}
		</button>
	{/snippet}
</AppTooltip>
