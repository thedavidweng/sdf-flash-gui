#!/bin/sh
# Exercise every install path in site/install.sh against stubbed system tools.
#
#   sh scripts/test-install.sh [shell]   # shell that runs install.sh, default sh
# shellcheck disable=SC2016
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
script="$root/site/install.sh"
shell=$(command -v "${1:-sh}") || { echo "shell not found: ${1:-sh}" >&2; exit 1; }
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

passed=0
failed=0

real_tools="awk sed tr grep mktemp mkdir rm cp cat chmod head test sha256sum shasum perl"

write_stub() {
  printf '#!/bin/sh\n%s\n' "$2" >"$stubs/$1"
  chmod 755 "$stubs/$1"
}

remove_stub() {
  rm -f "$stubs/$1"
}

checksum() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{ print $1 }'
  else
    shasum -a 256 "$1" | awk '{ print $1 }'
  fi
}

add_asset() {
  printf '%s\n' "$2" >"$fixtures/assets/$1"
  printf '"browser_download_url": "https://github.com/x/y/releases/download/v1.1.0/%s",\n' "$1" >>"$fixtures/release.json"
}

add_sums() {
  sums_file=$1
  shift
  : >"$fixtures/assets/$sums_file"
  while [ $# -gt 0 ]; do
    printf '%s  %s\n' "$(checksum "$fixtures/assets/$1")" "$2" >>"$fixtures/assets/$sums_file"
    shift 2
  done
  printf '"browser_download_url": "https://github.com/x/y/releases/download/v1.1.0/%s",\n' "$sums_file" >>"$fixtures/release.json"
}

setup() {
  case_dir="$work/$1"
  stubs="$case_dir/bin"
  fixtures="$case_dir/fixtures"
  home="$case_dir/home"
  log="$case_dir/calls.log"
  mkdir -p "$stubs" "$fixtures/assets" "$home"
  : >"$log"
  printf '{"tag_name": "v1.1.0", "assets": [\n' >"$fixtures/release.json"

  for tool in $real_tools; do
    path=$(command -v "$tool" 2>/dev/null || true)
    [ -n "$path" ] && ln -s "$path" "$stubs/$tool"
  done

  write_stub uname 'case "$1" in -s) echo "$STUB_OS" ;; -m) echo "$STUB_ARCH" ;; esac'
  write_stub sysctl 'echo "$STUB_ARM64"'
  write_stub id 'echo "$STUB_UID"'
  write_stub curl '
out=""
url=""
while [ $# -gt 0 ]; do
  case "$1" in
    -o) out=$2; shift 2 ;;
    --retry) shift 2 ;;
    -*) shift ;;
    *) url=$1; shift ;;
  esac
done
echo "curl $url" >>"$STUB_LOG"
case "$url" in
  https://api.github.com/*) src="$STUB_FIXTURES/release.json" ;;
  *) src="$STUB_FIXTURES/assets/${url##*/}" ;;
esac
[ -f "$src" ] || exit 22
if [ -n "$out" ]; then cp "$src" "$out"; else cat "$src"; fi'
  write_stub brew '
echo "brew $*" >>"$STUB_LOG"
[ "$1" = list ] && [ "${STUB_BREW_HAS_CASK:-0}" = 1 ] && exit 0
[ "$1" = list ] && exit 1
exit 0'
  write_stub hdiutil '
echo "hdiutil $1" >>"$STUB_LOG"
if [ "$1" = attach ]; then
  while [ $# -gt 0 ]; do
    [ "$1" = -mountpoint ] && mnt=$2
    shift
  done
  mkdir -p "$mnt/SDF Flash GUI.app/Contents"
  echo dmg-app >"$mnt/SDF Flash GUI.app/Contents/marker"
fi'
  write_stub ditto 'cp -R "$1" "$2"'
  write_stub sudo 'echo "sudo $*" >>"$STUB_LOG"; "$@"'
  write_stub apt-get 'echo "apt-get $*" >>"$STUB_LOG"'
  write_stub dpkg 'echo "dpkg $*" >>"$STUB_LOG"'

  STUB_OS=Darwin
  STUB_ARCH=arm64
  STUB_ARM64=1
  STUB_UID=501
  STUB_BREW_HAS_CASK=0
}

finish_release() {
  printf '{}]}\n' >>"$fixtures/release.json"
}

mac_release() {
  add_asset SDF.Flash.GUI_1.1.0_aarch64.dmg arm-dmg
  add_asset SDF.Flash.GUI_1.1.0_x64.dmg intel-dmg
  add_sums SHA256SUMS-aarch64-apple-darwin.txt SDF.Flash.GUI_1.1.0_aarch64.dmg "SDF Flash GUI_1.1.0_aarch64.dmg"
  add_sums SHA256SUMS-x86_64-apple-darwin.txt SDF.Flash.GUI_1.1.0_x64.dmg "SDF Flash GUI_1.1.0_x64.dmg"
  finish_release
}

linux_release() {
  add_asset sdf-flash-gui_1.1.0_amd64.deb deb
  if [ "${1:-with-appimage}" = with-appimage ]; then
    add_asset sdf-flash-gui_1.1.0_x86_64.AppImage appimage
  fi
  add_sums SHA256SUMS-x86_64-unknown-linux-gnu.txt \
    sdf-flash-gui_1.1.0_amd64.deb sdf-flash-gui_1.1.0_amd64.deb
  if [ "${1:-with-appimage}" = with-appimage ]; then
    printf '%s  %s\n' "$(checksum "$fixtures/assets/sdf-flash-gui_1.1.0_x86_64.AppImage")" \
      sdf-flash-gui_1.1.0_x86_64.AppImage >>"$fixtures/assets/SHA256SUMS-x86_64-unknown-linux-gnu.txt"
  fi
  finish_release
}

run_install() {
  set +e
  env -i \
    PATH="$stubs" HOME="$home" \
    STUB_OS="$STUB_OS" STUB_ARCH="$STUB_ARCH" STUB_ARM64="$STUB_ARM64" STUB_UID="$STUB_UID" \
    STUB_BREW_HAS_CASK="$STUB_BREW_HAS_CASK" STUB_LOG="$log" STUB_FIXTURES="$fixtures" \
    SDF_FLASH_GUI_APP_DIR="$home/Applications" \
    "$shell" "$script" >"$case_dir/out.txt" 2>&1
  status=$?
  set -e
}

pass() {
  passed=$((passed + 1))
  printf 'ok   %s\n' "$1"
}

flunk() {
  failed=$((failed + 1))
  printf 'FAIL %s: %s\n' "$1" "$2"
  sed 's/^/     | /' "$case_dir/out.txt"
}

expect_ok() {
  [ "$status" -eq 0 ] || { flunk "$1" "exit $status"; return 1; }
}

expect_fail() {
  [ "$status" -ne 0 ] || { flunk "$1" "expected failure"; return 1; }
  grep -qF -- "$2" "$case_dir/out.txt" || { flunk "$1" "missing message: $2"; return 1; }
}

expect_call() {
  grep -qxF -- "$2" "$log" || { flunk "$1" "missing call: $2"; return 1; }
}

expect_no_call() {
  ! grep -q -- "$2" "$log" || { flunk "$1" "unexpected call: $2"; return 1; }
}

expect() {
  label=$1
  why=$2
  shift 2
  "$@" || { flunk "$label" "$why"; return 1; }
}

t() {
  name=$1
  shift
  setup "$name"
  if "$@"; then pass "$name"; fi
}

case_brew_install() {
  run_install
  expect_ok "$name" &&
    expect_call "$name" "brew install --cask thedavidweng/tap/sdf-flash-gui" &&
    expect_no_call "$name" "^curl"
}

case_brew_upgrade() {
  STUB_BREW_HAS_CASK=1
  run_install
  expect_ok "$name" &&
    expect_call "$name" "brew upgrade --cask thedavidweng/tap/sdf-flash-gui" &&
    expect_no_call "$name" "brew install"
}

case_mac_dmg_arm() {
  remove_stub brew
  mac_release
  run_install
  expect_ok "$name" &&
    expect_call "$name" "curl https://github.com/x/y/releases/download/v1.1.0/SDF.Flash.GUI_1.1.0_aarch64.dmg" &&
    expect_call "$name" "hdiutil detach" &&
    expect "$name" "app not copied" test -f "$home/Applications/SDF Flash GUI.app/Contents/marker"
}

case_mac_dmg_intel() {
  remove_stub brew
  STUB_ARM64=0
  STUB_ARCH=x86_64
  mac_release
  run_install
  expect_ok "$name" &&
    expect_call "$name" "curl https://github.com/x/y/releases/download/v1.1.0/SDF.Flash.GUI_1.1.0_x64.dmg"
}

case_mac_dmg_replaces_old_app() {
  remove_stub brew
  mac_release
  mkdir -p "$home/Applications/SDF Flash GUI.app/Contents"
  echo old >"$home/Applications/SDF Flash GUI.app/Contents/stale"
  run_install
  expect_ok "$name" &&
    expect "$name" "old app kept" test ! -e "$home/Applications/SDF Flash GUI.app/Contents/stale"
}

case_mac_checksum_mismatch() {
  remove_stub brew
  mac_release
  echo tampered >"$fixtures/assets/SDF.Flash.GUI_1.1.0_aarch64.dmg"
  run_install
  expect_fail "$name" "checksum mismatch" &&
    expect_no_call "$name" "hdiutil"
}

case_mac_missing_sums_line() {
  remove_stub brew
  mac_release
  : >"$fixtures/assets/SHA256SUMS-aarch64-apple-darwin.txt"
  run_install
  expect_fail "$name" "has no checksum for *_aarch64.dmg"
}

case_mac_release_unreachable() {
  remove_stub brew
  rm "$fixtures/release.json"
  run_install
  expect_fail "$name" "could not read the latest release"
}

case_linux_apt_sudo() {
  STUB_OS=Linux
  STUB_ARCH=x86_64
  linux_release
  run_install
  expect_ok "$name" &&
    expect "$name" "no sudo apt-get install" grep -q '^sudo apt-get install -y /.*/sdf-flash-gui_1.1.0_amd64.deb$' "$log"
}

case_linux_apt_root() {
  STUB_OS=Linux
  STUB_ARCH=x86_64
  STUB_UID=0
  linux_release
  run_install
  expect_ok "$name" &&
    expect_no_call "$name" "^sudo" &&
    expect "$name" "no apt-get install" grep -q '^apt-get install -y ' "$log"
}

case_linux_no_sudo() {
  STUB_OS=Linux
  STUB_ARCH=x86_64
  remove_stub sudo
  linux_release
  run_install
  expect_fail "$name" "need root or sudo"
}

case_linux_dpkg() {
  STUB_OS=Linux
  STUB_ARCH=x86_64
  remove_stub apt-get
  linux_release
  run_install
  expect_ok "$name" &&
    expect "$name" "no dpkg -i" grep -q '^sudo dpkg -i /.*/sdf-flash-gui_1.1.0_amd64.deb$' "$log"
}

case_linux_appimage() {
  STUB_OS=Linux
  STUB_ARCH=x86_64
  remove_stub apt-get
  remove_stub dpkg
  linux_release
  run_install
  expect_ok "$name" &&
    expect "$name" "AppImage not executable" test -x "$home/.local/bin/sdf-flash-gui" &&
    expect "$name" "wrong AppImage content" grep -qx appimage "$home/.local/bin/sdf-flash-gui" &&
    expect "$name" "no PATH hint" grep -qF "Add $home/.local/bin to PATH" "$case_dir/out.txt"
}

case_linux_appimage_missing() {
  STUB_OS=Linux
  STUB_ARCH=x86_64
  remove_stub apt-get
  remove_stub dpkg
  linux_release without-appimage
  run_install
  expect_fail "$name" "has no *_x86_64.appimage download"
}

case_linux_arm() {
  STUB_OS=Linux
  STUB_ARCH=aarch64
  run_install
  expect_fail "$name" "only x86_64 Linux builds"
}

case_windows_shell() {
  STUB_OS=MINGW64_NT-10.0
  run_install
  expect_fail "$name" "irm https://thedavidweng.github.io/sdf-flash-gui/install.ps1 | iex"
}

case_unsupported_os() {
  STUB_OS=FreeBSD
  run_install
  expect_fail "$name" "unsupported system FreeBSD"
}

t brew-install case_brew_install
t brew-upgrade case_brew_upgrade
t mac-dmg-arm case_mac_dmg_arm
t mac-dmg-intel case_mac_dmg_intel
t mac-dmg-replaces-old-app case_mac_dmg_replaces_old_app
t mac-checksum-mismatch case_mac_checksum_mismatch
t mac-missing-sums-line case_mac_missing_sums_line
t mac-release-unreachable case_mac_release_unreachable
t linux-apt-sudo case_linux_apt_sudo
t linux-apt-root case_linux_apt_root
t linux-no-sudo case_linux_no_sudo
t linux-dpkg case_linux_dpkg
t linux-appimage case_linux_appimage
t linux-appimage-missing case_linux_appimage_missing
t linux-arm case_linux_arm
t windows-shell case_windows_shell
t unsupported-os case_unsupported_os

printf '\n%d passed, %d failed\n' "$passed" "$failed"
[ "$failed" -eq 0 ]
