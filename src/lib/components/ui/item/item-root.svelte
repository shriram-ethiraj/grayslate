<script lang="ts" module>
	import { cn, type WithElementRef } from "$lib/utils.js";
	import type { HTMLAttributes } from "svelte/elements";
	import { type VariantProps, tv } from "tailwind-variants";

	export const itemVariants = tv({
		base: "relative flex min-w-0 items-start gap-3 rounded-xl border text-left shadow-xs transition-colors",
		variants: {
			variant: {
				default: "bg-card text-card-foreground border-border/80",
				outline: "bg-background/70 text-foreground border-border/70",
				muted: "bg-muted/40 text-foreground border-border/60",
			},
			size: {
				default: "p-4",
				sm: "p-3",
				lg: "p-5",
			},
		},
		defaultVariants: {
			variant: "default",
			size: "default",
		},
	});

	export type ItemVariant = VariantProps<typeof itemVariants>["variant"];
	export type ItemSize = VariantProps<typeof itemVariants>["size"];
	export type ItemRootProps = WithElementRef<HTMLAttributes<HTMLDivElement>> & {
		variant?: ItemVariant;
		size?: ItemSize;
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
	}: ItemRootProps = $props();
</script>

<div
	bind:this={ref}
	data-slot="item"
	data-variant={variant}
	data-size={size}
	class={cn(itemVariants({ variant, size }), className)}
	{...restProps}
>
	{@render children?.()}
</div>