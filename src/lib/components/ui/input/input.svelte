<script lang="ts">
	import type { HTMLInputAttributes, HTMLInputTypeAttribute } from "svelte/elements";
	import { cn, type WithElementRef } from "$lib/utils.js";
	import X from "~icons/lucide/x";

	type InputType = Exclude<HTMLInputTypeAttribute, "file">;

	type Props = WithElementRef<
		Omit<HTMLInputAttributes, "type"> &
			({ type: "file"; files?: FileList } | { type?: InputType; files?: undefined }) &
			{ clearable?: boolean }
	>;

	let {
		ref = $bindable(null),
		value = $bindable(),
		type,
		files = $bindable(),
		class: className,
		"data-slot": dataSlot = "input",
		clearable = false,
		...restProps
	}: Props = $props();
</script>

{#if type === "file"}
	<input
		bind:this={ref}
		data-slot={dataSlot}
		class={cn(
				"dark:bg-input/30 border-input ring-offset-background placeholder:text-muted-foreground flex h-9 w-full min-w-0 rounded-md border bg-transparent px-3 pt-1.5 text-sm font-medium shadow-xs transition-[color,box-shadow] outline-none disabled:cursor-not-allowed disabled:opacity-50",
			"focus-visible:border-ring focus-visible:ring-1 focus-visible:ring-ring",
			"aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive",
			className
		)}
		type="file"
		bind:files
		bind:value
		{...restProps}
	/>
{:else if clearable}
	<div class="relative w-full">
		<input
			bind:this={ref}
			data-slot={dataSlot}
			class={cn(
				"border-input bg-background dark:bg-input/30 ring-offset-background placeholder:text-muted-foreground flex h-9 w-full min-w-0 rounded-md border px-3 py-1 text-base shadow-xs transition-[color,box-shadow] outline-none disabled:cursor-not-allowed disabled:opacity-50 md:text-sm",
				"focus-visible:border-ring focus-visible:ring-1 focus-visible:ring-ring",
				"aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive",
				value ? "pr-8" : "",
				className
			)}
			{type}
			bind:value
			{...restProps}
		/>
		{#if value}
			<button
				type="button"
				tabindex="-1"
				aria-label="Clear"
				class="absolute right-2.5 top-1/2 -translate-y-1/2 rounded text-muted-foreground opacity-50 transition-opacity hover:opacity-100 focus-visible:opacity-100 focus-visible:outline-none"
				onclick={() => {
					value = "";
					ref?.focus();
				}}
			>
				<X class="size-3.5" />
			</button>
		{/if}
	</div>
{:else}
	<input
		bind:this={ref}
		data-slot={dataSlot}
		class={cn(
			"border-input bg-background dark:bg-input/30 ring-offset-background placeholder:text-muted-foreground flex h-9 w-full min-w-0 rounded-md border px-3 py-1 text-base shadow-xs transition-[color,box-shadow] outline-none disabled:cursor-not-allowed disabled:opacity-50 md:text-sm",
			"focus-visible:border-ring focus-visible:ring-1 focus-visible:ring-ring",
			"aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive",
			className
		)}
		{type}
		bind:value
		{...restProps}
	/>
{/if}
