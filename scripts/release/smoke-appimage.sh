#!/usr/bin/env bash
set -euo pipefail

# Run under a dedicated Xvfb display so another Grayslate window cannot satisfy
# the check. Isolate application data from the builder's normal installation.
appimage=$(realpath "${1:?Usage: smoke-appimage.sh <AppImage>}")
temporary=$(mktemp -d)
app_pid=""
cleanup() {
    if [[ -n "$app_pid" ]]; then
        kill -- "-$app_pid" 2>/dev/null || true
        wait "$app_pid" 2>/dev/null || true
    fi
    rm -rf "$temporary"
}
trap cleanup EXIT

setsid env XDG_CONFIG_HOME="$temporary/config" XDG_DATA_HOME="$temporary/data" \
XDG_CACHE_HOME="$temporary/cache" APPIMAGE_EXTRACT_AND_RUN=1 \
WEBKIT_DISABLE_DMABUF_RENDERER=1 WEBKIT_DISABLE_COMPOSITING_MODE=1 \
    "$appimage" >"$temporary/startup.log" 2>&1 &
app_pid=$!

for ((attempt = 0; attempt < 60; attempt++)); do
    if ! kill -0 "$app_pid" 2>/dev/null; then
        cat "$temporary/startup.log"
        echo "AppImage exited before showing a window." >&2
        exit 1
    fi
    window_ids=$(xwininfo -root -tree | awk '/\("[Gg]rayslate" "[Gg]rayslate"\)/ { print $1 }')
    for window_id in $window_ids; do
        if xwininfo -id "$window_id" 2>/dev/null | grep 'Map State: IsViewable' >/dev/null; then
            echo "AppImage opened a visible Grayslate window."
            exit 0
        fi
    done
    sleep 0.5
done
cat "$temporary/startup.log"
echo "AppImage did not show a Grayslate window within 30 seconds." >&2
exit 1
