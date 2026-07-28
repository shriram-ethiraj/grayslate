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
	// The same walk stamps the toast kind onto the <li> as a test hook: the
	// library's own `data-type` lives there, but only the message span is
	// addressable from this component.
	$effect(() => {
		if (!el) return;

		let target: HTMLElement | null = el.parentElement;
		while (target && !target.hasAttribute("data-sonner-toast")) {
			target = target.parentElement;
		}
		if (!target) return;

		const toastEl = target;
		toastEl.setAttribute("data-testid", "toast");
		toastEl.style.cursor = "pointer";

		function handleClick() {
			toast.dismiss(toastId);
		}

		toastEl.addEventListener("click", handleClick);
		return () => {
			toastEl.removeEventListener("click", handleClick);
			toastEl.removeAttribute("data-testid");
			toastEl.style.cursor = "";
		};
	});
</script>

<span bind:this={el} data-testid="toast-message">{message}</span>
