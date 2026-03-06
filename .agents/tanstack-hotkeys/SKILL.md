---
name: TanStack Hotkeys Integration
description: Complete reference for @tanstack/hotkeys v0.3.1 — how the library works internally, the Svelte wrapper, key pitfalls, and all patterns used in Grayslate.
---

# TanStack Hotkeys Integration

This document covers everything an agent needs to know to correctly use, debug, or extend `@tanstack/hotkeys` in Grayslate.

**Version in use:** `@tanstack/hotkeys@0.3.1`
**Wrapper location:** `src/lib/hotkeys.ts`

---

## 1. How the Library Works Internally

Understanding the internals prevents the most common bugs.

### 1.1 Singleton Manager

`HotkeyManager` is a **singleton** (`HotkeyManager.getInstance()`). There is exactly one instance per application. All registrations across all components use the same manager.

```ts
import { HotkeyManager } from "@tanstack/hotkeys";
const manager = HotkeyManager.getInstance();
```

### 1.2 Per-Target Event Listeners

The manager attaches **one `keydown` + one `keyup` listener per unique target element**. When you register a hotkey with `target: someDiv`, a listener is added to `someDiv`. When you register without a target, the listener is added to `document`.

- This means hotkeys registered on a specific element only react to events that **bubble through or originate from that element**.
- When the last registration for a target is removed, the listeners on that target are also removed.

### 1.3 `ignoreInputs` — The Most Important Option

**Default behaviour (the library resolves this dynamically):**
```ts
function getDefaultIgnoreInputs(parsedHotkey): boolean {
  if (parsedHotkey.ctrl || parsedHotkey.meta) return false; // Ctrl/Cmd combos fire in inputs
  if (parsedHotkey.key === "Escape") return false;           // Escape fires in inputs
  return true; // All other keys are ignored when an input is focused
}
```

**What `ignoreInputs: true` means:**  
The hotkey callback will **NOT** fire when `event.target` is an `<input>`, `<textarea>`, `<select>`, or a `contenteditable` element — **unless** the `event.target` is the hotkey's own `target` element.

**What `ignoreInputs: false` means:**  
The hotkey callback fires regardless of whether an input is focused.

> ⚠️ **Critical gotcha:** If you have a hotkey bound to a wrapper `<div>` with `ignoreInputs: false`, it will fire even when the user is typing into a child `<input>`. This is almost always wrong for navigation/action keys (arrows, Enter, Delete, etc.).

### 1.4 The `isInputElement` Check in Detail

```ts
function isInputElement(element): boolean {
  if (element instanceof HTMLInputElement) {
    // Button-type inputs are NOT considered inputs for this check
    const type = element.type.toLowerCase();
    if (type === "button" || type === "submit" || type === "reset") return false;
    return true; // text, number, date, etc. → true
  }
  if (element instanceof HTMLTextAreaElement) return true;
  if (element instanceof HTMLSelectElement) return true;
  if (element instanceof HTMLElement && element.contentEditable === "true") return true;
  return false;
}
```

The check is: if `ignoreInputs !== false` AND `event.target` is an input element AND `event.target !== registration.target` → **skip this registration**.

So a hotkey on a `<div>` with `ignoreInputs: true` will still fire if a child `<button>` is focused (because `<button>` is not an input element), but will NOT fire if a child `<input>` is focused.

### 1.5 `stopPropagation` is `true` by Default

The library defaults:
```ts
const defaultHotkeyOptions = {
  preventDefault: true,
  stopPropagation: true,  // ← stops event bubbling by default!
  eventType: "keydown",
  enabled: true,
  ignoreInputs: true,
  conflictBehavior: "warn"
};
```

This means a hotkey on the `document` (no target) that matches a key will stop that event from propagating further — which affects other `document`-level or `window`-level listeners.

### 1.6 Conflict Behaviour

When you register the same hotkey+target combination twice, the default is `conflictBehavior: "warn"` — **both handlers will fire** and a console warning is printed. Use `conflictBehavior: "replace"` during re-registration scenarios or `conflictBehavior: "allow"` to suppress the warning.

---

## 2. The Grayslate Wrapper (`src/lib/hotkeys.ts`)

This file is the **only place** components should import hotkey utilities from. Never import raw `@tanstack/hotkeys` types directly in feature code.

### 2.1 Exported Type: `HotkeyBinding`

`HotkeyBinding` is the canonical type for a single hotkey registration. Always use this when typing arrays or function parameters that carry hotkeys.

```ts
import type { HotkeyBinding } from "$lib/hotkeys";

// ✅ Correct — use the wrapper's type
const cellHotkeys: HotkeyBinding[] = [
  { key: "ArrowDown", callback: navigateDown, options: { ignoreInputs: true } },
];

// ❌ Wrong — reconstructing the type from raw library imports
import type { RegisterableHotkey, HotkeyCallback, HotkeyOptions } from "@tanstack/hotkeys";
type HotkeyParam = { key: RegisterableHotkey; callback: HotkeyCallback; options?: Omit<HotkeyOptions, 'target'> };
```

`HotkeyBinding` is defined as:
```ts
export type HotkeyBinding = {
  key: RegisterableHotkey;
  callback: HotkeyCallback;
  options?: Omit<HotkeyOptions, "target">; // target is injected by the action/wrapper
};
```

### 2.2 `registerHotkey(binding | key, callback?, options?)` → `() => void`

Registers a single hotkey on `document` (no target). Returns an unregister function.

```ts
import { registerHotkey } from "$lib/hotkeys";

// In a $effect — returns cleanup function automatically
$effect(() => {
  return registerHotkey("Mod+S", (e) => {
    e.preventDefault();
    save();
  }, { ignoreInputs: false });
});
```

Also accepts a single `HotkeyBinding` object:
```ts
$effect(() => {
  return registerHotkey({ key: "Mod+S", callback: handleSave, options: { ignoreInputs: false } });
});
```

### 2.3 `registerHotkeys(bindings[])` → `() => void`

Registers multiple hotkeys at once. Returns a single unregister function that removes all of them.

```ts
import { registerHotkeys, type HotkeyBinding } from "$lib/hotkeys";

$effect(() => {
  return registerHotkeys([
    { key: "Mod+O", callback: handleOpen, options: { ignoreInputs: false } },
    { key: "Alt+Z", callback: toggleWordWrap, options: { ignoreInputs: false } },
  ]);
});
```

### 2.4 `hotkey` (Svelte action)

Registers hotkeys **scoped to a specific DOM element** (the node the action is applied to). The library attaches its listener to that specific element, not `document`.

```svelte
<!-- Single hotkey -->
<div use:hotkey={{ key: "Escape", callback: close }}>

<!-- Multiple hotkeys -->
<input 
  use:hotkey={[
    { key: "Enter", callback: submit, options: { ignoreInputs: false } },
    { key: "Escape", callback: cancel, options: { ignoreInputs: false } },
  ]}
/>
```

The action handles:
- **`onMount`**: registrations are set up when the element mounts
- **`onUpdate`**: params changes re-register (avoids stale closures)
- **`onDestroy`**: all registrations are cleaned up automatically

### 2.5 Internal: `cleanupHotkeyHandles`

This is an internal helper used by the wrapper functions to build the cleanup closure. It is **not exported** and should not be duplicated in feature code. The wrapper functions already handle cleanup — just return the result of `registerHotkey`/`registerHotkeys` from a `$effect`.

---

## 3. Key Format

Hotkeys are expressed as strings like `"Mod+S"`, `"Shift+Enter"`, `"ArrowDown"`.

- **`Mod`** → resolves to `Ctrl` on Windows/Linux, `⌘` on macOS
- **`Alt`** → `Alt` on Win/Linux, `⌥` on macOS
- **Key names** follow the `KeyboardEvent.key` spec: `"Enter"`, `"Escape"`, `"ArrowUp"`, `"Tab"`, `"F2"`, `"Delete"`, `"Backspace"`, `"Home"`, `"End"`, `"PageUp"`, `"PageDown"`, etc.

### 3.1 `formatForDisplay`

Import from `@tanstack/hotkeys` (this is a display utility, not a registration utility — importing directly here is acceptable):

```ts
import { formatForDisplay } from "@tanstack/hotkeys";
```

Use `formatForDisplay` whenever you render a shortcut label in UI — button `title` attributes, tooltip text, kbd elements, or menu item labels. It maps key strings to platform-appropriate symbols:

```ts
formatForDisplay("Mod+S")           // → "Ctrl+S" on Windows/Linux, "⌘S" on macOS
formatForDisplay("Mod+Shift+Z")     // → "Ctrl+Shift+Z" or "⌘⇧Z"
formatForDisplay("Shift+Enter")     // → "Shift+Enter" or "⇧↵"
formatForDisplay("Escape")          // → "Esc"
formatForDisplay("Alt+Z")           // → "Alt+Z" or "⌥Z"
```

Examples in Grayslate:
```svelte
<!-- Titlebar.svelte —  redo label adapts to platform -->
const redoShortcut = $derived(
  isMac ? formatForDisplay("Mod+Shift+Z") : formatForDisplay("Mod+Y"),
);

<!-- FindReplace.svelte — tooltip text -->
title="Next match ({formatForDisplay('Enter')})"
title="Previous match ({formatForDisplay('Shift+Enter')})"
title="Close ({formatForDisplay('Escape')})"
```

> **Note:** `formatForDisplay` accepts the same key strings used in `registerHotkey`. On non-Mac platforms the output is always `Ctrl+...` style; on macOS it uses symbols. Do not hardcode `"Ctrl+S"` in UI labels — always use `formatForDisplay` so labels stay correct cross-platform.

### 3.2 Non-standard Key Combinations and `as any`

The `RegisterableHotkey` union type does not include every theoretically valid combination. A small set of keys are not in the union and require a cast. **This is the only permitted use of `as any` in hotkey code.**

**Permitted `as any` casts (library gap, not a type error):**
```ts
{ key: "Mod+Home" as any, ... }  // ✅ permitted — not in RegisterableHotkey union
{ key: "Mod+End"  as any, ... }  // ✅ permitted — not in RegisterableHotkey union
```

**Never cast standard combinations:**
```ts
// ❌ Wrong — these are in the union type, no cast needed
{ key: "Mod+Z"       as any, ... }
{ key: "Mod+Shift+Z" as any, ... }
{ key: "Mod+Y"       as any, ... }
{ key: "ArrowDown"   as any, ... }
```

If you find yourself writing `as any` on a standard key, it is a sign that the type import is wrong (e.g. a locally re-declared type instead of `HotkeyBinding` from `$lib/hotkeys`).

---

## 4. Patterns Used in Grayslate

### 4.1 Global Hotkeys (Titlebar, no element target)

Global shortcuts like `Mod+O` and `Alt+Z` are registered in `Titlebar.svelte` using `registerHotkeys` inside a `$effect`. The effect's cleanup return handles unregistration.

```ts
$effect(() => {
  return registerHotkeys([
    { key: "Mod+O", callback: (e) => { e.preventDefault(); handleOpen(); }, options: { ignoreInputs: false } },
    { key: "Alt+Z", callback: (e) => { /* ... */ }, options: { ignoreInputs: false } },
  ]);
});
```

### 4.2 Contextual Hotkeys (open only while a panel is visible)

For hotkeys that only make sense while a panel is open (e.g., Escape to close FindReplace), guard the registration with a condition:

```ts
$effect(() => {
  if (!fr.visible) return; // Don't register when panel is hidden
  return registerHotkey("Escape", () => close(), { ignoreInputs: false });
});
```

### 4.3 Scoped Input Hotkeys (edit inputs in CSV table)

For hotkeys on `<input>` elements where you want the keys to override the browser default (e.g., Enter to commit, Tab to navigate), use the `hotkey` Svelte action with `ignoreInputs: false`:

```svelte
<input
  use:hotkey={[
    { key: "Enter", callback: () => commitEdit(), options: { ignoreInputs: false } },
    { key: "Tab",   callback: () => tabToNext(),   options: { ignoreInputs: false } },
    { key: "Escape",callback: () => cancelEdit(),  options: { ignoreInputs: false } },
  ]}
/>
```

Because these are registered with `target: <the input element>`, the `isInputElement` check passes (`event.target === registration.target`), so the hotkey fires correctly.

### 4.4 Cell Navigation Hotkeys (parent container)

For hotkeys on a **container** element (like the CSV table wrapper `<div>`) that should fire when the container or its non-input children are focused, use `ignoreInputs: true` (or simply omit it — it's the default for non-Mod keys):

```svelte
<div use:hotkey={csvEditorState.cellHotkeys} ...>
```

```ts
// In useCsvEditorState.svelte.ts — import HotkeyBinding, not raw library types
import type { HotkeyBinding } from "$lib/hotkeys";

const cellHotkeys: HotkeyBinding[] = [
  { key: "ArrowDown", callback: navigateDown, options: { preventDefault: true, ignoreInputs: true } },
  { key: "Enter",     callback: startEdit,    options: { preventDefault: true, ignoreInputs: true } },
  // ...
];
```

`ignoreInputs: true` ensures these keys do NOT fire when the edit `<input>` child is focused, preventing double-handling conflicts where both the edit hotkeys and the cell hotkeys would try to handle the same key.

---

## 5. The Critical Bug Pattern to Avoid

### ❌ Wrong: `ignoreInputs: false` on a container with child inputs

```ts
// BAD — every cell navigation key also fires when the user is typing in an edit input
const cellHotkeys = [
  { key: "ArrowDown", callback: navigateDown, options: { ignoreInputs: false } },
  { key: "Delete",    callback: clearCell,    options: { ignoreInputs: false } },
  { key: "Enter",     callback: startEdit,    options: { ignoreInputs: false } },
];
```

**Symptoms:**
- Clicking a cell appears to "move the table" — actually navigates because Arrow keys fire before click focus settles
- Typing in an edit input runs the cell navigation actions simultaneously
- Delete/Backspace behave unexpectedly while editing

### ✅ Correct: Split responsibilities

```ts
// Container — ignores when child inputs are focused
const cellHotkeys = [
  { key: "ArrowDown", callback: navigateDown, options: { ignoreInputs: true } },
  { key: "Delete",    callback: clearCell,    options: { ignoreInputs: true } },
  { key: "Enter",     callback: startEdit,    options: { ignoreInputs: true } },
];

// The edit <input> — handles its own keys
const editHotkeys = [
  { key: "Enter",  callback: commitAndMoveDown, options: { ignoreInputs: false } },
  { key: "Escape", callback: cancelEdit,         options: { ignoreInputs: false } },
  { key: "Tab",    callback: commitAndTabNext,   options: { ignoreInputs: false } },
];
```

---

## 6. `ignoreInputs` Decision Table

| Key type                        | Recommended `ignoreInputs` | Why                                                             |
|---------------------------------|----------------------------|-----------------------------------------------------------------|
| Plain navigation key (Arrow, Home, End, PgUp, PgDn, Enter, F2, Delete, Backspace, Tab, Escape) on a **container** | `true` | Must not fire when user is editing a child input |
| Any key directly on the target `<input>` / `<textarea>` | `false` | The target IS the input, so `isInputElement` check is bypassed anyway when `event.target === registration.target`. Explicit `false` is clearer. |
| `Ctrl`/`Cmd` combo (global shortcuts) | `false` (library default for Mod keys) | Ctrl combos should always work regardless of focus |
| `Escape` (global close/dismiss) | `false` (library default for Escape) | Should always dismiss panels regardless of focus |

---

## 7. `scrollToIndex` + `navigateAndFocus` Pattern

When a hotkey moves the focused cell, always call `navigateAndFocus()` in the same callback. This:
1. Calls `scrollToIndex(rowIndex, { align: "auto" })` — which is a **no-op** if the row is already visible (fixed in the custom `useScrollVirtualizer`)
2. Uses `tick().then(() => cell.focus())` to wait for DOM re-render before calling `.focus()`

If `scrollToIndex` always scrolls (even for visible rows), every arrow key press will visually "jump" the table. Always use `align: "auto"` for keyboard navigation.

---

## 8. Cleanup Rules

| Usage                         | How to unregister                                      |
|-------------------------------|--------------------------------------------------------|
| `registerHotkey()` in `$effect` | Return the cleanup function from `$effect`            |
| `registerHotkeys()` in `$effect`| Return the cleanup function from `$effect`            |
| `use:hotkey` Svelte action    | Automatic — the action's `destroy()` handles it       |
| `use:hotkey` with dynamic params | Automatic — the action's `update()` re-registers on param change |

Never call `registerHotkey` outside of a reactive context without storing and calling the returned cleanup. Leaked registrations accumulate in the singleton manager and will fire unexpectedly.

---

## 9. TypeScript Rules — No `any`

This section consolidates all typing rules so agents do not introduce `any` unnecessarily.

### 9.1 Always use `HotkeyBinding` from `$lib/hotkeys`

Components and composables must import the wrapper type, not reconstruct it from raw library types.

```ts
// ✅ Correct
import type { HotkeyBinding } from "$lib/hotkeys";
const myHotkeys: HotkeyBinding[] = [ ... ];

// ❌ Wrong — raw re-declaration bypasses the wrapper and causes type drift
import type { RegisterableHotkey, HotkeyCallback, HotkeyOptions } from "@tanstack/hotkeys";
type MyHotkeyParam = { key: RegisterableHotkey; callback: HotkeyCallback; options?: Omit<HotkeyOptions, 'target'> };
```

### 9.2 `as any` is forbidden except for two documented keys

| Key string       | `as any` needed? | Reason |
|------------------|------------------|--------|
| `"Mod+Home"`     | ✅ yes           | Not in `RegisterableHotkey` union (library gap) |
| `"Mod+End"`      | ✅ yes           | Not in `RegisterableHotkey` union (library gap) |
| `"Mod+Z"`        | ❌ no            | Standard combo, in the union |
| `"Mod+Shift+Z"`  | ❌ no            | Standard combo, in the union |
| `"Mod+Y"`        | ❌ no            | Standard combo, in the union |
| All other keys   | ❌ no            | If the type errors, the key string or import is wrong |

### 9.3 Use direct function references in callbacks where possible

Avoid unnecessary wrapper functions in hotkey arrays. Prefer direct references:

```ts
// ✅ Preferred
{ key: "Mod+Z", callback: handleUndo, ... }

// ❌ Unnecessary wrapper — only add a wrapper when you need to pass arguments
// or perform inline logic that doesn't belong in a named function
{ key: "Mod+Z", callback: () => handleUndo(), ... }
```

Add a wrapper only when the callback has meaningful inline logic (e.g., updating state mid-navigation before calling a helper).
