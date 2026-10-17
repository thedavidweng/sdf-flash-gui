#!/usr/bin/env bash
set -euo pipefail

cache="${XDG_CACHE_HOME:-$HOME/.cache}/.cargo-packager/AppImage"
mkdir -p "$cache"
apprun="$cache/AppRun-x86_64"
if [ ! -f "$apprun" ]; then
  curl --fail --location --retry 3 \
    https://github.com/tauri-apps/binary-releases/releases/download/apprun-old/AppRun-x86_64 \
    --output "$apprun.download"
  mv "$apprun.download" "$apprun"
fi
chmod 755 "$apprun"
