#!/usr/bin/env bash
set -euo pipefail

suite="${1:-}"
case "$suite" in
  e2e:functional|e2e:security|e2e:visual|e2e:test) ;;
  *)
    echo "Unsupported E2E suite command: '$suite'" >&2
    exit 2
    ;;
esac

openbox >/tmp/grayslate-openbox.log 2>&1 &
window_manager_pid=$!
trap 'kill "$window_manager_pid" 2>/dev/null || true' EXIT

# Xvfb is ready before this script starts, but Openbox needs one short startup
# window before native maximize/minimize commands can be observed by WebDriver.
sleep 1
pnpm run "$suite"
