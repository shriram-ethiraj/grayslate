import { EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view";

/**
 * Linux / WebKitGTK gutter workaround.
 *
 * WebKitGTK fails to repaint sticky gutters during vertical scroll.
 *
 * Strategy:
 * - Default: non-sticky fallback (position: relative + translateX) — safe
 *   for vertical scrolling since there is nothing sticky to mis-paint.
 * - Pure horizontal scroll: switch to position: sticky immediately — the
 *   browser compositor handles horizontal pinning natively with zero JS
 *   overhead, so there is no jank.
 * - Any vertical scroll: switch back to fallback immediately.
 *
 * No timers or idle detection needed — the mode is determined purely by
 * which axis changed in the current scroll event.
 */
class LinuxGutterSync {
	private gutters: HTMLElement | null = null;
	private lastScrollLeft = -1;
	private lastScrollTop = -1;
	private isStickyMode = false;

	constructor(private readonly view: EditorView) {
		this.handleScroll = this.handleScroll.bind(this);
		this.captureGutters();
		this.applyFallbackMode(this.getSnappedScrollLeft());
		view.scrollDOM.addEventListener("scroll", this.handleScroll, { passive: true });
	}

	update(update: ViewUpdate) {
		if (update.geometryChanged || update.viewportChanged) {
			this.captureGutters();
			if (this.isStickyMode) {
				this.applyStickyMode();
			} else {
				this.applyFallbackMode(this.getSnappedScrollLeft());
			}
		}
	}

	destroy() {
		this.view.scrollDOM.removeEventListener("scroll", this.handleScroll);
		if (this.gutters) {
			this.resetModeStyles(this.gutters);
		}
		this.gutters = null;
	}

	private handleScroll() {
		const scrollLeft = this.getSnappedScrollLeft();
		const scrollTop = this.view.scrollDOM.scrollTop;
		const horizontalChanged = scrollLeft !== this.lastScrollLeft;
		const verticalChanged = scrollTop !== this.lastScrollTop;

		this.lastScrollLeft = scrollLeft;
		this.lastScrollTop = scrollTop;

		if (verticalChanged) {
			// Vertical movement → fallback to avoid WebKitGTK repaint bug
			this.applyFallbackMode(scrollLeft);
			return;
		}

		if (horizontalChanged) {
			// Pure horizontal movement → sticky for native smooth pinning
			this.applyStickyMode();
		}
	}

	private captureGutters() {
		const next = this.view.dom.querySelector<HTMLElement>(".cm-gutters");
		if (this.gutters && this.gutters !== next) {
			this.resetModeStyles(this.gutters);
			this.lastScrollLeft = -1;
			this.lastScrollTop = -1;
		}
		this.gutters = next;
	}

	private getSnappedScrollLeft() {
		const rawScrollLeft = this.view.scrollDOM.scrollLeft;
		const dpr = window.devicePixelRatio || 1;
		return Math.round(rawScrollLeft * dpr) / dpr;
	}

	/** Native sticky — browser handles horizontal pinning, no JS needed. */
	private applyStickyMode() {
		if (!this.gutters) return;
		this.isStickyMode = true;
		this.gutters.style.setProperty("position", "sticky");
		this.gutters.style.setProperty("left", "0px");
		this.gutters.style.removeProperty("transform");
		this.gutters.style.removeProperty("will-change");
	}

	/** JS fallback — safe for vertical scroll on WebKitGTK. */
	private applyFallbackMode(scrollLeft: number) {
		if (!this.gutters) return;
		this.isStickyMode = false;
		this.gutters.style.setProperty("position", "relative");
		this.gutters.style.removeProperty("left");
		this.gutters.style.removeProperty("will-change");
		this.gutters.style.setProperty(
			"transform",
			scrollLeft === 0
				? ""
				: `translateX(${scrollLeft}px)`,
		);
	}

	private resetModeStyles(gutters: HTMLElement) {
		gutters.style.removeProperty("position");
		gutters.style.removeProperty("left");
		gutters.style.removeProperty("transform");
		gutters.style.removeProperty("will-change");
	}
}

export const linuxStickyGutter = [
	ViewPlugin.fromClass(LinuxGutterSync),
	EditorView.theme({
		".cm-gutters": {
			zIndex: "2",
			flexShrink: "0",
		},
	}),
];