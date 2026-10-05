#!/usr/bin/env bash
# Assemble the GitHub Pages site (site/ + icon + wasm demo, ADR 0011) into target/site.
# Requires: rustup target wasm32-unknown-unknown, wasm-bindgen-cli matching Cargo.lock.
# Optional: wasm-opt (binaryen) for a smaller binary.
#
#   ./scripts/build-site.sh            # build into target/site
#   python3 -m http.server -d target/site 8000
set -euo pipefail

cd "$(dirname "$0")/.."

out_dir="${1:-target/site}"
locked=$(awk '/^name = "wasm-bindgen"$/{getline; gsub(/version = |"/, ""); print; exit}' Cargo.lock)
installed=$(wasm-bindgen --version | awk '{print $2}')
if [ "$locked" != "$installed" ]; then
  echo "wasm-bindgen-cli $installed does not match Cargo.lock $locked" >&2
  echo "install with: cargo install wasm-bindgen-cli --version $locked --locked" >&2
  exit 1
fi

cargo build -p sdf-flash-gui-web --release --target wasm32-unknown-unknown

rm -rf "$out_dir"
mkdir -p "$out_dir"
cp -R site/. "$out_dir/"
cp assets/icon.svg assets/icon.png assets/screenshot.png "$out_dir/"
touch "$out_dir/.nojekyll"

pkg_dir="$out_dir/demo"
wasm-bindgen \
  --target web \
  --no-typescript \
  --out-dir "$pkg_dir" \
  --out-name sdf_flash_gui \
  target/wasm32-unknown-unknown/release/sdf_flash_gui_web.wasm

if command -v wasm-opt >/dev/null 2>&1; then
  wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int \
    -o "$pkg_dir/sdf_flash_gui_bg.wasm" "$pkg_dir/sdf_flash_gui_bg.wasm"
fi

version=$(awk -F'"' '/^version = /{print $2; exit}' Cargo.toml)
sed -i.bak "s/__APP_VERSION__/$version/g" "$out_dir/index.html" && rm "$out_dir/index.html.bak"

ls -lh "$pkg_dir"
