import { EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view";

/**
 * Linux / WebKitGTK gutter fallback.
 *
 * Keep gutters in normal flow so Linux avoids CodeMirror's sticky repaint
 * path, then re-align the live gutter with a single rAF-scheduled transform
 * during horizontal scroll.
 */
class LinuxStickyGutter {
	private gutters: HTMLElement | null = null;
	private frame = 0;
	private lastScrollLeft = -1;

	constructor(private readonly view: EditorView) {
		this.handleScroll = this.handleScroll.bind(this);
		this.captureGutters();
		this.applyOffset();
		this.view.scrollDOM.addEventListener("scroll", this.handleScroll, { passive: true });
	}

	update(update: ViewUpdate) {
		if (update.geometryChanged || update.viewportChanged) {
			this.captureGutters();
			this.scheduleApplyOffset();
		}
	}

	destroy() {
		this.view.scrollDOM.removeEventListener("scroll", this.handleScroll);
		if (this.frame !== 0) {
			cancelAnimationFrame(this.frame);
			this.frame = 0;
		}
		if (this.gutters) {
			this.gutters.style.transform = "";
		}
		this.gutters = null;
	}

	private handleScroll() {
		this.scheduleApplyOffset();
	}

	private captureGutters() {
		const nextGutters = this.view.dom.querySelector<HTMLElement>(".cm-gutters");
		if (this.gutters && this.gutters !== nextGutters) {
			this.gutters.style.transform = "";
		}

		if (this.gutters !== nextGutters) {
			this.lastScrollLeft = -1;
		}

		this.gutters = nextGutters;
	}

	private scheduleApplyOffset() {
		if (this.frame !== 0) {
			return;
		}

		this.frame = requestAnimationFrame(() => {
			this.frame = 0;
			this.applyOffset();
		});
	}

	private applyOffset() {
		if (!this.gutters) {
			return;
		}

		const scrollLeft = this.view.scrollDOM.scrollLeft;
		if (scrollLeft === this.lastScrollLeft) {
			return;
		}

		this.lastScrollLeft = scrollLeft;
		this.gutters.style.transform = scrollLeft === 0 ? "" : `translateX(${scrollLeft}px)`;
	}
}

export const linuxStickyGutter = [
	ViewPlugin.fromClass(LinuxStickyGutter),
	EditorView.theme({
	".cm-gutters": {
		position: "relative",
		zIndex: "2",
		flexShrink: "0",
	},
}),
];