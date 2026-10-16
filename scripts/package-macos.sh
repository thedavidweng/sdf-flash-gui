#!/usr/bin/env bash
set -euo pipefail

target="${1:?provide the macOS Rust target}"
: "${APPLE_SIGNING_IDENTITY:?}" "${APPLE_SIGNING_KEYCHAIN:?}" "${APPLE_TEAM_ID:?}"
: "${APPLE_API_KEY_PATH:?}" "${APPLE_API_KEY_ID:?}" "${APPLE_API_ISSUER:?}"
case "$target" in
  aarch64-apple-darwin) arch=aarch64 ;;
  x86_64-apple-darwin) arch=x64 ;;
  *) echo "Unsupported macOS target: $target" >&2; exit 1 ;;
esac

cd "$(dirname "$0")/.."
version="$(cargo metadata --no-deps --format-version 1 | jq -er '.packages[] | select(.name == "sdf-flash-gui") | .version')"
cargo packager --release --target "$target" --formats app
bash scripts/sign-and-notarize.sh "dist/SDF Flash GUI.app"

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
ditto "dist/SDF Flash GUI.app" "$stage/SDF Flash GUI.app"
ln -s /Applications "$stage/Applications"
dmg="SDF.Flash.GUI_${version}_${arch}.dmg"
hdiutil create -volname "SDF Flash GUI" -srcfolder "$stage" -ov -format UDZO "dist/$dmg"
bash scripts/sign-and-notarize.sh "dist/$dmg"
(cd dist && shasum -a 256 "$dmg" > "SHA256SUMS-$target.txt")
