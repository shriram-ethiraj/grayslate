<script lang="ts" module>
	import { tv, type VariantProps } from "tailwind-variants";

	export const sidebarMenuButtonVariants = tv({
		// Hover / press / active colours all come from `.ui-state` (layout.css).
		// `transition-[...]` gained the colour and box-shadow properties so the
		// active chip fades in rather than popping.
		base: "ui-state ui-state-selected peer/menu-button ring-sidebar-ring flex w-full items-center gap-2 overflow-hidden rounded-md p-2 text-start text-sm outline-hidden transition-[width,height,padding,color,background-color,box-shadow] group-has-data-[sidebar=menu-action]/menu-item:pe-8 group-data-[collapsible=icon]:size-8! group-data-[collapsible=icon]:p-2! focus-visible:ring-2 disabled:pointer-events-none disabled:opacity-50 aria-disabled:pointer-events-none aria-disabled:opacity-50 data-[active=true]:font-medium [&>span:last-child]:truncate [&>svg]:size-4 [&>svg]:shrink-0",
		variants: {
			variant: {
				default: "",
				// `ring-1` writes `--tw-ring-shadow`; the former `shadow-[0_0_0_1px_…]`
				// wrote `--tw-shadow`, which would collide with the chip elevation.
				outline: "bg-background ring-1 ring-sidebar-border hover:bg-state-hover data-[active=true]:bg-state-selected data-[active=true]:ring-0 data-[active=true]:shadow-[var(--state-selected-shadow)]",
			},
			size: {
				default: "h-8 text-sm",
				sm: "h-7 text-xs",
				lg: "h-12 text-sm group-data-[collapsible=icon]:p-0!",
			},
		},
		defaultVariants: {
			variant: "default",
			size: "default",
		},
	});

	export type SidebarMenuButtonVariant = VariantProps<
		typeof sidebarMenuButtonVariants
	>["variant"];
	export type SidebarMenuButtonSize = VariantProps<
		typeof sidebarMenuButtonVariants
	>["size"];
</script>

<script lang="ts">
	import {
		cn,
		type WithElementRef,
		type WithoutChildrenOrChild,
	} from "$lib/utils.js";
	import type { Snippet } from "svelte";
	import type { HTMLAttributes } from "svelte/elements";
	import { useSidebar } from "./context.svelte.js";

	let {
		ref = $bindable(null),
		class: className,
		children,
		child,
		variant = "default",
		size = "default",
		isActive = false,
		...restProps
	}: WithElementRef<HTMLAttributes<HTMLButtonElement>, HTMLButtonElement> & {
		isActive?: boolean;
		variant?: SidebarMenuButtonVariant;
		size?: SidebarMenuButtonSize;
		child?: Snippet<[{ props: Record<string, unknown> }]>;
	} = $props();

	const sidebar = useSidebar();

	const buttonProps = $derived({
		class: cn(sidebarMenuButtonVariants({ variant, size }), className),
		"data-slot": "sidebar-menu-button",
		"data-sidebar": "menu-button",
		"data-size": size,
		"data-active": isActive,
		...restProps,
	});
</script>

{#if child}
	{@render child({ props: buttonProps })}
{:else}
	<button bind:this={ref} {...buttonProps}>
		{@render children?.()}
	</button>
{/if}
