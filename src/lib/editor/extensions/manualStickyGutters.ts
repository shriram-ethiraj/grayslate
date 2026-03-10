import { EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view";

class ManualStickyGutters {
	private gutters: HTMLElement | null = null;
	private frame = 0;
	private lastScrollLeft = -1;

	constructor(private readonly view: EditorView) {
		this.handleScroll = this.handleScroll.bind(this);
		this.captureGutters();
		this.applyOffset();
		this.view.scrollDOM.addEventListener("scroll", this.handleScroll, { passive: true });
	}

	update(update: ViewUpdate): void {
		if (update.geometryChanged || update.viewportChanged) {
			this.captureGutters();
			this.scheduleApplyOffset();
		}
	}

	destroy(): void {
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

	private handleScroll(): void {
		this.scheduleApplyOffset();
	}

	private captureGutters(): void {
		const nextGutters = this.view.dom.querySelector<HTMLElement>(".cm-gutters");
		if (this.gutters && this.gutters !== nextGutters) {
			this.gutters.style.transform = "";
		}

		if (this.gutters !== nextGutters) {
			this.lastScrollLeft = -1;
		}

		this.gutters = nextGutters;
	}

	private scheduleApplyOffset(): void {
		if (this.frame !== 0) {
			return;
		}

		this.frame = requestAnimationFrame(() => {
			this.frame = 0;
			this.applyOffset();
		});
	}

	private applyOffset(): void {
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

export const manualStickyGutters = [
	ViewPlugin.fromClass(ManualStickyGutters),
	EditorView.theme({
		".cm-gutters": {
			position: "relative",
			zIndex: "2",
			flexShrink: "0",
		},
	}),
];