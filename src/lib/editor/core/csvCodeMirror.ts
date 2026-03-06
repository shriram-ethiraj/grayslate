import type { EditorView } from "codemirror";

export type TextChangeSpec = {
    from: number;
    to: number;
    insert: string;
};

export function getMinimalTextChange(
    previousText: string,
    nextText: string,
): TextChangeSpec | null {
    if (previousText === nextText) {
        return null;
    }

    let prefixLength = 0;
    const maxPrefixLength = Math.min(previousText.length, nextText.length);
    while (
        prefixLength < maxPrefixLength &&
        previousText.charCodeAt(prefixLength) === nextText.charCodeAt(prefixLength)
    ) {
        prefixLength += 1;
    }

    let previousSuffixIndex = previousText.length;
    let nextSuffixIndex = nextText.length;
    while (
        previousSuffixIndex > prefixLength &&
        nextSuffixIndex > prefixLength &&
        previousText.charCodeAt(previousSuffixIndex - 1) ===
            nextText.charCodeAt(nextSuffixIndex - 1)
    ) {
        previousSuffixIndex -= 1;
        nextSuffixIndex -= 1;
    }

    return {
        from: prefixLength,
        to: previousSuffixIndex,
        insert: nextText.slice(prefixLength, nextSuffixIndex),
    };
}

export function dispatchCsvDocChange(
    view: EditorView,
    nextText: string,
    options?: {
        previousText?: string;
        userEvent?: string;
        focus?: boolean;
    },
): boolean {
    const previousText = options?.previousText ?? view.state.doc.toString();
    const changes = getMinimalTextChange(previousText, nextText);

    if (!changes) {
        return false;
    }

    view.dispatch({
        changes,
        userEvent: options?.userEvent ?? "input",
    });

    if (options?.focus !== false) {
        view.focus();
    }

    return true;
}