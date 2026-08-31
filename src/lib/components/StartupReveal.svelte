<script lang="ts">
	import type { Snippet } from "svelte";

	let {
		ready = false,
		variant,
		children,
		onRevealComplete,
	}: {
		ready?: boolean;
		variant: "editor" | "sidebar";
		children: Snippet;
		onRevealComplete?: () => void;
	} = $props();

	let revealCompleted = $state(false);

	$effect(() => {
		if (!ready || revealCompleted) return;
		revealCompleted = true;
		onRevealComplete?.();
	});
</script>

<div
	class="startup-reveal"
	data-startup-reveal={variant}
	data-reveal-ready={ready}
	aria-busy={!revealCompleted}
>
	<div
		class="startup-reveal-actual"
		class:startup-reveal-interactive={ready}
	>
		{@render children()}
	</div>
</div>

<style>
	.startup-reveal {
		position: relative;
		display: flex;
		min-width: 0;
		min-height: 0;
		flex: 1;
		overflow: hidden;
	}

	.startup-reveal-actual {
		position: absolute;
		inset: 0;
		z-index: 0;
		display: flex;
		min-width: 0;
		min-height: 0;
		pointer-events: none;
	}

	.startup-reveal-interactive {
		pointer-events: auto;
	}
</style>
