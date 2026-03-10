import { EditorView } from "@codemirror/view";

/**
 * CSS-only horizontal gutter pinning.
 *
 * Uses `position: sticky; left: 0` so the browser compositor keeps the
 * gutter column glued to the left viewport edge during horizontal scroll —
 * zero JS, zero frame lag.
 *
 * We keep `gutters({ fixed: false })` in the editor session so the gutters
 * scroll normally in the vertical direction.  This avoids the WebKitGTK
 * vertical-sticky repaint bug that occurs with CodeMirror's built-in
 * `fixed: true` mode (which adds vertical pinning via `top`).
 *
 * The distinction:
 *   CM `fixed: true`  →  sticky + top + left  →  breaks on Linux/WebKitGTK
 *   This extension     →  sticky + left only   →  safe everywhere
 */
export const manualStickyGutters = EditorView.theme({
	".cm-gutters": {
		position: "sticky",
		left: "0",
		zIndex: "2",
		flexShrink: "0",
	},
});