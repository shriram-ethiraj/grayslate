<script lang="ts">
	import { toast } from "svelte-sonner";

	let {
		message,
		toastId,
	}: {
		message: string;
		toastId: string | number;
	} = $props();

	let el: HTMLSpanElement | null = $state(null);

	// Walk up to the [data-sonner-toast] <li> and attach a click listener
	// so clicking anywhere on the toast (icon, padding, text) dismisses it.
	$effect(() => {
		if (!el) return;

		let target: HTMLElement | null = el.parentElement;
		while (target && !target.hasAttribute("data-sonner-toast")) {
			target = target.parentElement;
		}
		if (!target) return;

		const toastEl = target;
		toastEl.style.cursor = "pointer";

		function handleClick() {
			toast.dismiss(toastId);
		}

		toastEl.addEventListener("click", handleClick);
		return () => {
			toastEl.removeEventListener("click", handleClick);
			toastEl.style.cursor = "";
		};
	});
</script>

<span bind:this={el}>{message}</span>
