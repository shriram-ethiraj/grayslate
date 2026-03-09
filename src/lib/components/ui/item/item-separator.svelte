<script lang="ts">
	import Separator from "$lib/components/ui/separator/separator.svelte";
	import { cn } from "$lib/utils.js";
	import type { ComponentProps } from "svelte";

	let {
		class: className,
		...restProps
	}: ComponentProps<typeof Separator> = $props();
</script>

<Separator data-slot="item-separator" class={cn("bg-border/70", className)} {...restProps} />