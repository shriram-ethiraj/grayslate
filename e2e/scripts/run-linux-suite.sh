#!/usr/bin/env bash
set -euo pipefail

suite="${1:-}"
case "$suite" in
  e2e:functional|e2e:stability|e2e:security|e2e:visual|e2e:test) ;;
  *)
    echo "Unsupported E2E suite command: '$suite'" >&2
    exit 2
    ;;
esac

openbox >/tmp/grayslate-openbox.log 2>&1 &
window_manager_pid=$!
trap 'kill "$window_manager_pid" 2>/dev/null || true' EXIT

if ! command -v xprop >/dev/null 2>&1; then
  echo "xprop is required to verify that Openbox owns the X11 root window (install x11-utils)." >&2
  exit 1
fi

# `xprop` can exit successfully while reporting that an atom does not exist, so
# parse the WM-owned support window and verify that window advertises the same
# property. This is the EWMH readiness contract, not a fixed startup delay.
deadline=$((SECONDS + 15))
while true; do
  root_property="$(xprop -root _NET_SUPPORTING_WM_CHECK 2>/dev/null || true)"
  if [[ "$root_property" =~ window[[:space:]]id[[:space:]]\#[[:space:]](0x[0-9a-fA-F]+) ]]; then
    wm_window="${BASH_REMATCH[1]}"
    wm_property="$(xprop -id "$wm_window" _NET_SUPPORTING_WM_CHECK 2>/dev/null || true)"
    if [[ "$wm_property" == *"$wm_window"* ]]; then
      break
    fi
  fi

  if ! kill -0 "$window_manager_pid" 2>/dev/null; then
    echo "Openbox exited before claiming the root window:" >&2
    sed -n '1,120p' /tmp/grayslate-openbox.log >&2
    exit 1
  fi
  if [ "$SECONDS" -ge "$deadline" ]; then
    echo "Openbox never claimed the root window:" >&2
    sed -n '1,120p' /tmp/grayslate-openbox.log >&2
    exit 1
  fi
  sleep 0.1
done

pnpm run "$suite"
pnpm run e2e:retry-audit
