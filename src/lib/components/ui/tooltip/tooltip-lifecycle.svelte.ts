import type { Window as TauriWindow } from "@tauri-apps/api/window";

type TooltipDeactivationListener = () => void;

const hasDocument = typeof document !== "undefined";

let browserWindowFocused = hasDocument ? document.hasFocus() : true;
let nativeWindowFocused: boolean | null = null;
let documentVisible = hasDocument ? document.visibilityState === "visible" : true;
let active = $state(browserWindowFocused && documentVisible);
let engagedSharedTrigger: HTMLElement | null = null;
let suppressedSharedFocusTrigger: HTMLElement | null = null;
const registeredSharedTriggers = new Set<HTMLElement>();

const deactivationListeners = new Set<TooltipDeactivationListener>();

function resetEngagedSharedTrigger(): void {
	const focusedElement = document.activeElement;
	const focusedTrigger =
		focusedElement instanceof HTMLElement && registeredSharedTriggers.has(focusedElement)
			? focusedElement
			: null;
	const trigger = focusedTrigger ?? engagedSharedTrigger;
	engagedSharedTrigger = null;
	if (!trigger?.isConnected) return;
	if (focusedTrigger === trigger) suppressedSharedFocusTrigger = trigger;

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

	// Window deactivation deliberately preserves DOM focus. A synthetic blur
	// runs Bits UI's close handler without changing document.activeElement, so
	// a keyboard-opened tooltip does not reappear when the root is enabled again.
	if (document.activeElement === trigger) {
		trigger.dispatchEvent(
			new FocusEvent("blur", {
				bubbles: false,
				relatedTarget: null,
			}),
		);
	}
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
		suppressedSharedFocusTrigger = null;
		delete document.documentElement.dataset.tooltipWindowActive;
	};
}

/** Reject Bits UI focus-open requests until the user genuinely re-engages. */
export function isSharedTooltipOpenSuppressed(): boolean {
	const trigger = suppressedSharedFocusTrigger;
	return trigger?.isConnected === true && document.activeElement === trigger;
}

/** Track the one shared tooltip trigger currently engaged by pointer or focus. */
export function registerTooltipTriggerLifecycle(trigger: HTMLElement): () => void {
	registeredSharedTriggers.add(trigger);
	const handlePointerEnter = (event: PointerEvent) => {
		if (event.pointerType === "touch") return;
		if (active) {
			suppressedSharedFocusTrigger = null;
		}
		engagedSharedTrigger = trigger;
	};
	const handlePointerLeave = () => {
		if (engagedSharedTrigger === trigger && document.activeElement !== trigger) {
			engagedSharedTrigger = null;
		}
	};
	const handleFocus = () => {
		if (suppressedSharedFocusTrigger === trigger) {
			return;
		}
		engagedSharedTrigger = trigger;
	};
	const handleBlur = () => {
		if (engagedSharedTrigger === trigger) engagedSharedTrigger = null;
	};
	const handleKeyDown = () => {
		// A real key interaction is a fresh engagement. In particular, Tab clears
		// suppression before focus moves, allowing a later keyboard return to open.
		if (active && suppressedSharedFocusTrigger === trigger) {
			suppressedSharedFocusTrigger = null;
		}
	};

	trigger.addEventListener("pointerenter", handlePointerEnter, true);
	trigger.addEventListener("pointerleave", handlePointerLeave);
	trigger.addEventListener("focus", handleFocus);
	trigger.addEventListener("blur", handleBlur);
	trigger.addEventListener("keydown", handleKeyDown);

	return () => {
		registeredSharedTriggers.delete(trigger);
		trigger.removeEventListener("pointerenter", handlePointerEnter, true);
		trigger.removeEventListener("pointerleave", handlePointerLeave);
		trigger.removeEventListener("focus", handleFocus);
		trigger.removeEventListener("blur", handleBlur);
		trigger.removeEventListener("keydown", handleKeyDown);
		if (engagedSharedTrigger === trigger) engagedSharedTrigger = null;
		if (suppressedSharedFocusTrigger === trigger) suppressedSharedFocusTrigger = null;
	};
}

export function onTooltipWindowDeactivated(
	listener: TooltipDeactivationListener,
): () => void {
	deactivationListeners.add(listener);
	return () => deactivationListeners.delete(listener);
}
