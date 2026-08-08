import {
	markE2EClosing,
	markE2EReady,
	readPendingWork,
	type E2EPendingWork,
} from "./workTracker";

const MOTION_STYLE_ID = "grayslate-e2e-determinism";

interface E2EBridge {
	pending(): E2EPendingWork;
	markClosing(): void;
	markReady(): void;
}

declare global {
	interface Window {
		__grayslateE2E?: E2EBridge;
	}
}

function installMotionOverride(): void {
	if (document.getElementById(MOTION_STYLE_ID)) return;

	const style = document.createElement("style");
	style.id = MOTION_STYLE_ID;
	style.textContent = `
*, *::before, *::after {
	animation-duration: 1ms !important;
	animation-delay: 0s !important;
	animation-iteration-count: 1 !important;
	transition-duration: 1ms !important;
	transition-delay: 0s !important;
	scroll-behavior: auto !important;
}
`;
	document.head.append(style);
}

function installInvokeTracking(): void {
	if (window.__grayslateE2E) return;

	window.__grayslateE2E = {
		pending: readPendingWork,
		markClosing: markE2EClosing,
		markReady: markE2EReady,
	};
}

installMotionOverride();
installInvokeTracking();
