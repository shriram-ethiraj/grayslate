import {
    HotkeyManager,
    type HotkeyOptions,
    type HotkeyCallback,
    type RegisterableHotkey,
    type HotkeyRegistrationHandle,
} from "@tanstack/hotkeys";

// Singleton manager instance for the application
export const hotkeyManager = HotkeyManager.getInstance();

export type HotkeyBinding = {
    key: RegisterableHotkey;
    callback: HotkeyCallback;
    options?: Omit<HotkeyOptions, "target">;
};

function cleanupHotkeyHandles(
    handles: HotkeyRegistrationHandle[],
): () => void {
    return () => {
        handles.forEach((handle) => handle.unregister());
    };
}

export function registerHotkey(binding: HotkeyBinding): () => void;
export function registerHotkey(
    key: RegisterableHotkey,
    callback: HotkeyCallback,
    options?: Omit<HotkeyOptions, "target">,
): () => void;
export function registerHotkey(
    bindingOrKey: HotkeyBinding | RegisterableHotkey,
    callback?: HotkeyCallback,
    options?: Omit<HotkeyOptions, "target">,
): () => void {
    const binding =
        typeof bindingOrKey === "object" &&
        bindingOrKey !== null &&
        "callback" in bindingOrKey
            ? bindingOrKey
            : {
                    key: bindingOrKey,
                    callback: callback!,
                    options,
                };

    const handle = hotkeyManager.register(
        binding.key,
        binding.callback,
        binding.options,
    );

    return cleanupHotkeyHandles([handle]);
}

export function registerHotkeys(bindings: HotkeyBinding[]): () => void {
    const handles = bindings.map((binding) =>
        hotkeyManager.register(binding.key, binding.callback, binding.options),
    );

    return cleanupHotkeyHandles(handles);
}

/**
 * A Svelte action to register hotkeys on a specific DOM element.
 * Handles lifecycle (unregistering on destroy).
 * 
 * Usage:
 * <div use:hotkey={{ key: "Mod+S", callback: handleSave }}>
 * 
 * <div use:hotkey={[
 *   { key: "ArrowUp", callback: handleUp },
 *   { key: "ArrowDown", callback: handleDown }
 * ]}>
 */
export function hotkey(
    node: HTMLElement,
    params: HotkeyBinding | HotkeyBinding[],
) {
    let cleanup = () => {};

    function setup(p: typeof params) {
        cleanup();

        const paramArray = Array.isArray(p) ? p : [p];

        cleanup = cleanupHotkeyHandles(
            paramArray.map((item) =>
                hotkeyManager.register(item.key, item.callback, {
                    target: node,
                    ...item.options,
                }),
            ),
        );
    }

    setup(params);

    return {
        update(newParams: typeof params) {
            setup(newParams);
        },
        destroy() {
            cleanup();
            cleanup = () => {};
        },
    };
}
