# Cross-platform file-drop smoke checklist

Run these checks against packaged artifacts after the automated Rust and E2E
suites pass. WebDriver can exercise the production drop queue through its
test-only backend seam, but it cannot synthesize the OS-owned pointer lifecycle
that Tauri receives from Windows, macOS, or Linux.

## Every desktop target

- Drag one supported text file anywhere over the workspace. Confirm the overlay
  covers the sidebar, resize handle, editor toolbar, editor, and status area,
  while leaving the custom titlebar visible.
- Move the pointer out without dropping. Confirm the overlay disappears and the
  sidebar width, editor height, and CSV viewport do not change.
- Drop the file over the sidebar and again over the editor. Confirm both open it
  through the same loading, encoding, size-limit, and recent-files flow.
- Drop several valid files together. Confirm every file appears in the library,
  preceding files open in separate windows, and the last file opens in the
  receiving window with that window focused.
- With a dirty local document, drop several files whose final valid file is not
  open. Verify Save/Discard opens the final file here, **Open All in New
  Windows** preserves the dirty window, and Cancel opens no files while leaving
  all valid files registered.
- Repeat with the final file already open in this or another window. Confirm no
  dirty prompt appears, the dirty content is preserved, and the final owner is
  focused after any preceding unopened files receive windows.
- Use **File → Open** to select several files and confirm it matches the drop
  behavior. Repeat with **Open File in New Window** and confirm every selection
  opens outside the current window without a dirty prompt.
- Drop a file while About or Settings is open. Confirm the dialog stays open and
  the file opens only after it closes.
- Drop the active file while it has unsaved edits. Confirm there is no prompt or
  reload and the edit remains intact.
- Drop a folder, a missing path if the desktop permits it, and a file larger than
  200 MB. Confirm the current document remains usable and skipped items produce
  a readable warning.
- Repeat representative drops in light and dark themes, with the sidebar open
  and collapsed, and while a large CSV is in table mode. Confirm there is no
  layout growth, excessive CPU use, or virtualizer instability.

## Platform launcher and file-association behavior

- **macOS:** Drop an associated file on the Grayslate Dock icon. Confirm the app
  activates and opens it in an existing/new window without consulting another
  window's dirty state.
- **macOS:** With Grayslate fully closed, use Finder's **Open With** on a small
  `.json` file and then on a `.jsonl` file. Confirm each cold launch reuses the
  initial window, opens the requested document, and produces no new Grayslate
  `.ips` crash report. Repeat **Open With** while Grayslate is already running:
  unopened files must receive new windows and already-open files must focus
  their existing owner.
- On every platform, multi-select several files and invoke **Open With**.
  Confirm every valid unique file opens, the final file is focused, and a
  minimized existing owner is restored rather than duplicated. Repeat while a
  different Grayslate window contains a dirty external file and confirm it is
  neither prompted nor modified.
- **Linux:** On desktop environments that support launcher drops, drop one or
  more associated files on the launcher and confirm `%F` delivers them in order.
  Record the distribution, desktop environment, and packaging format.
- **Windows:** Hover the Grayslate taskbar icon until the window is revealed,
  then drop anywhere in the workspace. Direct release onto a taskbar icon is
  shell-controlled and is not a guaranteed application event.

Record the artifact version, operating system, desktop environment or window
manager, result, and reviewer in the release issue.
