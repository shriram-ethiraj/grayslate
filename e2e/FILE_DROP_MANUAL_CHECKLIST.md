# Cross-platform file-drop smoke checklist

Run these checks against packaged artifacts after the automated Rust and E2E
suites pass. WebDriver can exercise the production drop queue through its
test-only backend seam, but it cannot synthesize the OS-owned pointer lifecycle
that Tauri receives from Windows, macOS, or Linux.

## Every desktop target

- Confirm the File menu says **Open Files…** and **Open Files in New Windows…**,
  and shortcut help says **Open Files**. On macOS check the native menu after
  switching between windows; on Windows/Linux check the in-window menu.
- Open 8 and 16 files on each platform. Confirm multiple documents begin loading
  together, the UI remains usable, and final focus follows selection order.
  Repeat with a slow first file, a failed final file, and encoding confirmation
  left open. On Windows verify repeated concurrent window creation completes
  without a native window/IPC deadlock.

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
- Use **File → Open Files…** to select several files and confirm it matches the drop
  behavior. Repeat with **Open Files in New Windows…** and confirm every selection
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

## Ownership and batch regression checks

- Open 49, 50, and 51 distinct valid files through File Open and native drop.
  Confirm at most 50 unique files are processed in each request, the 51st is
  reported as skipped and is not newly tracked, and duplicate/invalid entries
  do not use the allowance. Repeat another request without closing windows:
  total open windows may exceed 50.
- Keep two untitled slates open and type different content in each. Let both
  autosave, then close and reopen each. Their identities and bytes must remain
  independent. On macOS, Save, New Slate, Find, and other native menu commands
  must affect only the focused window.
- Mix a valid file with a final binary/unreadable file. Confirm the original
  receiving document survives and the valid preceding file still opens. Repeat
  with a declined encoding confirmation.
- Reopen the current file with another supported encoding and Save As to its
  existing path. Both must work without creating another window.
- Start opening a file in another window and request it again before loading
  finishes. Confirm a single owner, correct final focus, and search-line
  navigation after its editor is ready.
- Edit a large local text document and immediately open another file or close
  the window. Repeat with CSV table edits above 100,000 rows. The unsaved guard
  must see the latest edit; Save must preserve it, Cancel must retain it.
- Close the original primary window while others remain. Confirm survivors
  continue saving and receiving OS opens. Open into a blank untitled window,
  then into an occupied window, and verify the OS flow only reuses the blank.

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
