#!/usr/bin/env bash
set -euo pipefail

image="$(realpath "$1")"
scripts="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d)"
app_pid=''
trap 'if [ -n "$app_pid" ]; then sudo kill -- "-$app_pid" 2>/dev/null || true; wait "$app_pid" 2>/dev/null || true; fi; rm -rf "$work"' EXIT

if [ "$(id -u)" -eq 0 ]; then
  echo 'Run the AppImage smoke check as a non-root user' >&2
  exit 1
fi
chmod +x "$image"
cd "$work"
"$image" --appimage-extract >/dev/null
python3 "$scripts/check-appdir.py" "$work/squashfs-root"

setsid sudo unshare --net -- runuser -u "$(id -un)" -- \
  env DISPLAY="$DISPLAY" XAUTHORITY="${XAUTHORITY:-}" HOME="$HOME" LIBGL_ALWAYS_SOFTWARE=1 \
  "$work/squashfs-root/AppRun" >app.log 2>&1 &
app_pid=$!
for _ in $(seq 1 30); do
  if ! kill -0 "$app_pid" 2>/dev/null; then
    cat app.log
    echo 'AppImage exited before displaying a window' >&2
    exit 1
  fi
  if xdotool search --onlyvisible --pid "$(pgrep -n -u "$(id -u)" -x sdf-flash-gui || echo 0)" >windows.txt 2>/dev/null; then
    sleep 3
    kill -0 "$app_pid"
    cat app.log
    echo 'AppImage displayed a window offline as a non-root user'
    exit 0
  fi
  sleep 1
done
cat app.log
echo 'AppImage did not display a window within 30 seconds' >&2
exit 1
