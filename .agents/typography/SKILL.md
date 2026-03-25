# Typography & Spacing — Design System

> Canonical reference for font sizes, icon sizes, control heights, and spacing.
> Every UI component must use tokens from this scale. **No arbitrary Tailwind values
> like `text-[13px]` or `h-[1.2rem]`.**

---

## Typography Scale

| Token       | Size  | Tailwind  | Usage                                                      |
|-------------|-------|-----------|------------------------------------------------------------|
| **Caption** | 12 px | `text-xs` | Status bar, sidebar metadata, badges, shortcuts, descriptions |
| **Body**    | 14 px | `text-sm` | Menu items, inputs, labels, sidebar file names, buttons    |
| **Subhead** | 16 px | `text-base` | Dialog titles (rare)                                     |
| **Heading** | 18 px+| `text-lg`+ | Major headings (About dialog app name)                   |

All sizes are standard Tailwind defaults — no custom overrides needed.

### When to use each

- **`text-xs`**: Any "chrome" or secondary content — status bar, sidebar metadata,
  group labels, badge counts, keyboard shortcut hints, descriptions.
- **`text-sm`**: The default for anything the user reads or interacts with —
  menu items, inputs, labels, sidebar file titles, buttons.
- **`text-base` / `text-lg`+**: Reserved for dialog titles and brand headings.
  Very rare; one or two per dialog at most.

### When to use each

- **`text-micro`**: Only for "chrome" strips that are always visible and contain
  dense, read-only info (status bar, sidebar metadata labels, count badges).
- **`text-xs`**: Secondary content inside interactive surfaces — shortcut hints
  in menus, descriptions in command palettes, group headings.
- **`text-sm`**: The default for anything the user reads or interacts with —
  menu items, inputs, labels, sidebar file titles, buttons.
- **`text-base` / `text-lg`+**: Reserved for dialog titles and brand headings.
  Very rare; one or two per dialog at most.

---

## Icon Scale

| Size  | Tailwind    | Usage                                                    |
|-------|-------------|----------------------------------------------------------|
| 16 px | `size-4`    | Standard — menus, sidebar, toolbars, dialogs, buttons    |

**Anti-pattern:** `h-[1.2rem] w-[1.2rem]` or any arbitrary icon sizing. Use `size-4`.

---

## Control Heights (Interactive Elements)

| Size  | Tailwind  | Variant            | Usage                                        |
|-------|-----------|--------------------|----------------------------------------------|
| 24 px | `h-6`     | —                  | Status bar                                   |
| 28 px | `size-7`  | `size="icon-xs"`   | Compact icon buttons (Find/Replace panels)   |
| 32 px | `size-8`  | `size="icon-sm"`   | Sidebar menu buttons, small button variant   |
| 36 px | `size-9`  | `size="icon"`      | Toolbar icon buttons, standard action buttons |
| 36 px | `h-9`     | `size="default"`   | Dialog inputs, primary buttons               |

The `icon-xs` button variant (`size-7`, 28 px) is defined in
`src/lib/components/ui/button/button.svelte` for compact panels
that need tight icon buttons (e.g., Find/Replace, future floating toolbars).

---

## Spacing

Standard Tailwind spacing — no custom tokens needed.

| Size | Tailwind   | Usage                                         |
|------|------------|-----------------------------------------------|
| 2 px | `gap-0.5`  | Between icon buttons in dense groups           |
| 4 px | `gap-1`    | Between closely related items                  |
| 6 px | `gap-1.5`  | Between row elements                           |
| 8 px | `gap-2`    | Between sections, panel padding (`p-2`)        |
| 12 px| `gap-3`    | Between form groups in dialogs                 |

**Panel padding:** Use `p-2` (8 px) for floating panels (Find/Replace).
Use `p-6` for full-page dialogs (shadcn default).

---

## Anti-Patterns

| ❌ Don't                              | ✅ Do                            |
|---------------------------------------|----------------------------------|
| `text-[11px]`                         | `text-micro`                     |
| `text-[13px]`, `text-[15px]`          | `text-sm`                        |
| `text-[10px]`                         | `text-micro`                     |
| `h-[1.2rem] w-[1.2rem]` (icons)      | `size-4`                         |
| `class="h-7 w-7"` on buttons         | `size="icon-xs"`                 |
| Any `text-[Npx]` arbitrary value      | Choose the nearest scale token   |

---

## Allowed Exceptions

| Component          | Exception                | Reason                              |
|--------------------|--------------------------|-------------------------------------|
| CSV table headers  | `var(--csv-header-*)` CSS props | Data grid has its own visual rules |
| CodeMirror editor  | User-configurable font   | Editor content ≠ UI chrome          |
| Markdown preview   | `@tailwindcss/typography` | Prose rendering, not app chrome     |
| Window controls    | Platform-specific icons  | Intentional per-OS visual weight    |
| Layout widths      | `max-w-[44rem]` etc.     | Content constraints, not typography |
