import { beginTrackedWork } from "virtual:grayslate-e2e-runtime";
import { waitForAppDialogSlot } from "$lib/state/appDialogs.svelte";

export type DocumentOpenOutcome = "opened" | "existing-owner" | "cancelled" | "failed";

// Module state belongs to one webview. All document-changing entry points use
// this queue; internal Save callbacks deliberately run inside the current task
// so an unsaved-changes decision never waits behind itself.
let tail: Promise<unknown> = Promise.resolve();
let disposed = false;

export function enqueueDocumentTransition<T>(operation: () => Promise<T>): Promise<T> {
    const finish = beginTrackedWork("document-transition");
    const run = async (): Promise<T> => {
        await waitForAppDialogSlot();
        if (disposed) throw new Error("The editor window has closed.");
        return operation();
    };
    const result = tail.then(run, run).finally(finish);
    tail = result.catch(() => undefined);
    return result;
}

export function disposeDocumentTransitions(): void {
    disposed = true;
}
