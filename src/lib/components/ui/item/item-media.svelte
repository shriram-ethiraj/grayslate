<script lang="ts" module>
	import { cn, type WithElementRef } from "$lib/utils.js";
	import type { HTMLAttributes } from "svelte/elements";
	import { type VariantProps, tv } from "tailwind-variants";

	export const itemMediaVariants = tv({
		base: "flex shrink-0 items-center justify-center",
		variants: {
			variant: {
				default: "",
				icon: "size-10 rounded-xl border border-border/70 bg-muted/45 [&_svg]:size-5",
				image: "overflow-hidden rounded-lg [&_img]:size-full [&_img]:object-cover",
			},
			size: {
				default: "",
				sm: "[&[data-variant='icon']]:size-9",
				lg: "[&[data-variant='icon']]:size-11",
			},
		},
		defaultVariants: {
			variant: "default",
			size: "default",
		},
	});

	export type ItemMediaVariant = VariantProps<typeof itemMediaVariants>["variant"];
	export type ItemMediaSize = VariantProps<typeof itemMediaVariants>["size"];
	export type ItemMediaProps = WithElementRef<HTMLAttributes<HTMLDivElement>> & {
		variant?: ItemMediaVariant;
		size?: ItemMediaSize;
	};
</script>

<script lang="ts">
	let {
		ref = $bindable(null),
		class: className,
		variant = "default",
		size = "default",
		children,
		...restProps
	}: ItemMediaProps = $props();
</script>

<div
	bind:this={ref}
	data-slot="item-media"
	data-variant={variant}
	data-size={size}
	class={cn(itemMediaVariants({ variant, size }), className)}
	{...restProps}
>
	{@render children?.()}
</div>