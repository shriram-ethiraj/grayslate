# Linux release smoke checklist

Run this small checklist against the exact release artifact after the automated
Linux jobs pass. It covers only boundaries WebDriver cannot observe.

- Open a file with the native picker, cancel once, then choose a file. Confirm
  both paths match the packaged dialog behavior.
- Use Save As, cancel once, then save to a new path. Confirm the original stays
  untouched and the new file reopens.
- Choose “Show in File Manager” for a local file and confirm the system file
  manager selects the validated file.
- Click an external Markdown link, verify the full destination in the native
  confirmation, cancel once, then confirm and verify the system browser opens.
- If Git sync is enabled in the release candidate, sync against a disposable
  remote with real credentials and verify the resulting commit remotely.
- If a signed update candidate is available, check and install it, then verify
  the installed version and signature provenance.
- Open and switch away from a representative large text, Markdown, and CSV
  file while observing the process. Confirm memory returns to a stable range
  and no stale loader remains.

Record the artifact version, distro/window manager, result, and reviewer in the
release issue. Failures here block release even though this checklist is not a
WebDriver job.
