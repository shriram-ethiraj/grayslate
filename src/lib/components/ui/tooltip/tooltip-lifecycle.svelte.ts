import type { Window as TauriWindow } from "@tauri-apps/api/window";

type TooltipDeactivationListener = () => void;

const hasDocument = typeof document !== "undefined";

let browserWindowFocused = hasDocument ? document.hasFocus() : true;
let nativeWindowFocused: boolean | null = null;
let documentVisible = hasDocument ? document.visibilityState === "visible" : true;
let active = $state(browserWindowFocused && documentVisible);
let engagedSharedTrigger: HTMLElement | null = null;

const deactivationListeners = new Set<TooltipDeactivationListener>();

function resetEngagedSharedTrigger(): void {
	const trigger = engagedSharedTrigger;
	engagedSharedTrigger = null;
	if (!trigger?.isConnected) return;

	// Bits UI does not expose a public way to cancel its pending hover delay.
	// Delivering the trigger's normal leave event while the root is still active
	// clears that timer and its pointer-engagement flag without scanning the DOM.
	trigger.dispatchEvent(
		new PointerEvent("pointerleave", {
			bubbles: false,
			pointerType: "mouse",
			relatedTarget: null,
		}),
	);
}

function syncDocumentMarker(): void {
	if (!hasDocument) return;
	document.documentElement.dataset.tooltipWindowActive = String(active);
}

function reconcileActiveState(): void {
	const nextActive =
		documentVisible && browserWindowFocused && nativeWindowFocused !== false;
	if (nextActive === active) {
		syncDocumentMarker();
		return;
	}

	if (!nextActive) {
		resetEngagedSharedTrigger();

		// Let tooltip implementations run their normal leave/close paths before
		// roots become disabled. Bits UI intentionally ignores pointer events on
		// disabled roots, which would otherwise leave its private delay timer and
		// pointer-engagement flag armed across the focus transition.
		for (const listener of deactivationListeners) {
			try {
				listener();
			} catch (error) {
				// One tooltip implementation must not prevent the rest of the window
				// from reaching its inactive state.
				console.error("[Tooltip lifecycle] Failed to close a tooltip:", error);
			}
		}
	}

	active = nextActive;
	syncDocumentMarker();
}

export const tooltipLifecycle = {
	get active(): boolean {
		return active;
	},
};

function initializeTooltipLifecycle(focused: boolean, visible: boolean): void {
	browserWindowFocused = focused;
	nativeWindowFocused = null;
	documentVisible = visible;
	reconcileActiveState();
}

/** Native Tauri focus is authoritative and also repairs a missed browser event. */
function setNativeTooltipWindowFocused(focused: boolean): void {
	nativeWindowFocused = focused;
	browserWindowFocused = focused;
	reconcileActiveState();
}

/** Browser focus events provide immediate fallback while the native event is in flight. */
function setBrowserTooltipWindowFocused(focused: boolean): void {
	browserWindowFocused = focused;
	reconcileActiveState();
}

function setTooltipDocumentVisible(visible: boolean): void {
	documentVisible = visible;
	reconcileActiveState();
}

/**
 * Own the per-webview focus and visibility listeners that gate every tooltip
 * implementation. The returned cleanup is safe even if Tauri's asynchronous
 * native-listener registration has not completed yet.
 */
export function startTooltipWindowLifecycle(appWindow: TauriWindow): () => void {
	initializeTooltipLifecycle(
		document.hasFocus(),
		document.visibilityState === "visible",
	);

	let stopped = false;
	const handleWindowFocus = () => setBrowserTooltipWindowFocused(true);
	const handleWindowBlur = () => setBrowserTooltipWindowFocused(false);
	const handleVisibilityChange = () =>
		setTooltipDocumentVisible(document.visibilityState === "visible");
	const handlePageHide = () => setTooltipDocumentVisible(false);
	const handlePageShow = () => {
		setTooltipDocumentVisible(document.visibilityState === "visible");
		setBrowserTooltipWindowFocused(document.hasFocus());
	};

	window.addEventListener("focus", handleWindowFocus);
	window.addEventListener("blur", handleWindowBlur);
	document.addEventListener("visibilitychange", handleVisibilityChange);
	window.addEventListener("pagehide", handlePageHide);
	window.addEventListener("pageshow", handlePageShow);

	const nativeFocusUnlisten = appWindow
		.onFocusChanged(({ payload: focused }) => {
			if (!stopped) setNativeTooltipWindowFocused(focused);
		})
		.catch((error: unknown) => {
			console.warn("[Tooltip lifecycle] Native focus listener unavailable:", error);
			return undefined;
		});

	return () => {
		stopped = true;
		window.removeEventListener("focus", handleWindowFocus);
		window.removeEventListener("blur", handleWindowBlur);
		document.removeEventListener("visibilitychange", handleVisibilityChange);
		window.removeEventListener("pagehide", handlePageHide);
		window.removeEventListener("pageshow", handlePageShow);
		void nativeFocusUnlisten.then((unlisten) => unlisten?.());
		engagedSharedTrigger = null;
		delete document.documentElement.dataset.tooltipWindowActive;
	};
}

/** Track the one shared tooltip trigger currently engaged by a pointer. */
export function registerTooltipTriggerLifecycle(trigger: HTMLElement): () => void {
	const handlePointerEnter = (event: PointerEvent) => {
		if (event.pointerType !== "touch") engagedSharedTrigger = trigger;
	};
	const handlePointerLeave = () => {
		if (engagedSharedTrigger === trigger) engagedSharedTrigger = null;
	};

	trigger.addEventListener("pointerenter", handlePointerEnter);
	trigger.addEventListener("pointerleave", handlePointerLeave);

	return () => {
		trigger.removeEventListener("pointerenter", handlePointerEnter);
		trigger.removeEventListener("pointerleave", handlePointerLeave);
		if (engagedSharedTrigger === trigger) engagedSharedTrigger = null;
	};
}

export function onTooltipWindowDeactivated(
	listener: TooltipDeactivationListener,
): () => void {
	deactivationListeners.add(listener);
	return () => deactivationListeners.delete(listener);
}
