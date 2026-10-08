#!/bin/sh
# Install SDF Flash GUI from the latest GitHub release (ADR 0012).
#
#   curl -fsSL https://sdf-flash-gui.blahaj.uk/install.sh | sh
#
# macOS: Homebrew cask when brew is available, otherwise the DMG for this Mac.
# Linux: the .deb through apt-get or dpkg, otherwise the AppImage in ~/.local/bin.
# Every direct download is checked against the release's SHA256SUMS file.
set -eu

repo="thedavidweng/sdf-flash-gui"
cask="thedavidweng/tap/sdf-flash-gui"
app_name="SDF Flash GUI.app"
api_url="https://api.github.com/repos/$repo/releases/latest"
releases_url="https://github.com/$repo/releases/latest"
ps_command="irm https://sdf-flash-gui.blahaj.uk/install.ps1 | iex"

say() { printf '%s\n' "$*"; }
fail() { printf 'error: %s\n' "$*" >&2; exit 1; }
has() { command -v "$1" >/dev/null 2>&1; }
fetch() { curl -fsSL --retry 3 "$@"; }

ends_with_ci() {
  awk -v s="$1" '{
    n = tolower($0); t = tolower(s)
    if (length(n) >= length(t) && substr(n, length(n) - length(t) + 1) == t) { print; exit }
  }'
}

asset_url() {
  printf '%s\n' "$release_json" | tr ',' '\n' |
    sed -n 's/.*"browser_download_url": *"\([^"]*\)".*/\1/p' |
    ends_with_ci "$1"
}

expected_sha() {
  awk -v s="$2" '{
    sub(/\r$/, "")
    name = $0
    sub(/^[0-9A-Fa-f]+ +\*?/, "", name)
    n = tolower(name); t = tolower(s)
    if (length(n) >= length(t) && substr(n, length(n) - length(t) + 1) == t) { print tolower($1); exit }
  }' "$1"
}

sha256_of() {
  if has sha256sum; then
    sha256sum "$1" | awk '{ print tolower($1) }'
  elif has shasum; then
    shasum -a 256 "$1" | awk '{ print tolower($1) }'
  else
    fail "need sha256sum or shasum to verify the download"
  fi
}

as_root() {
  if [ "$(id -u)" -eq 0 ]; then
    "$@"
  elif has sudo; then
    sudo "$@"
  else
    fail "need root or sudo to run: $*"
  fi
}

load_release() {
  release_json=$(fetch "$api_url") || fail "could not read the latest release from GitHub"
}

download() {
  suffix=$1
  sums_name=$2
  url=$(asset_url "$suffix")
  [ -n "$url" ] || fail "the latest release has no *$suffix download; see $releases_url"
  sums_url=$(asset_url "$sums_name")
  [ -n "$sums_url" ] || fail "the latest release has no $sums_name; see $releases_url"

  file="$tmp/${url##*/}"
  say "Downloading ${url##*/}"
  fetch -o "$file" "$url"
  fetch -o "$tmp/$sums_name" "$sums_url"

  want=$(expected_sha "$tmp/$sums_name" "$suffix")
  [ -n "$want" ] || fail "$sums_name has no checksum for *$suffix"
  [ "$(sha256_of "$file")" = "$want" ] || fail "checksum mismatch for ${url##*/}"
}

warn_if_no_makemkv() {
  if ! has sdftool && ! has makemkvcon && [ ! -d "/Applications/MakeMKV.app" ]; then
    say "SDF Flash GUI needs MakeMKV for sdftool and makemkvcon: https://www.makemkv.com/"
  fi
}

install_macos() {
  if has brew; then
    if brew list --cask sdf-flash-gui >/dev/null 2>&1; then
      say "Homebrew found; upgrading $cask"
      brew upgrade --cask "$cask"
    else
      say "Homebrew found; installing $cask"
      brew install --cask "$cask"
    fi
    return
  fi

  if [ "$(sysctl -n hw.optional.arm64 2>/dev/null || true)" = 1 ]; then
    suffix=_aarch64.dmg
    sums=SHA256SUMS-aarch64-apple-darwin.txt
  else
    suffix=_x64.dmg
    sums=SHA256SUMS-x86_64-apple-darwin.txt
  fi

  load_release
  download "$suffix" "$sums"

  mnt="$tmp/mnt"
  mkdir "$mnt"
  hdiutil attach -nobrowse -readonly -quiet -mountpoint "$mnt" "$file"
  mounted=1
  [ -d "$mnt/$app_name" ] || fail "$app_name is missing from ${file##*/}"

  app_dir=${SDF_FLASH_GUI_APP_DIR:-/Applications}
  if [ ! -w "$app_dir" ]; then
    app_dir="$HOME/Applications"
    mkdir -p "$app_dir"
  fi
  rm -rf "${app_dir:?}/$app_name"
  ditto "$mnt/$app_name" "$app_dir/$app_name"
  hdiutil detach -quiet "$mnt"
  mounted=0
  say "Installed $app_dir/$app_name"
}

install_linux() {
  [ "$(uname -m)" = x86_64 ] || fail "only x86_64 Linux builds are published; see $releases_url"
  sums=SHA256SUMS-x86_64-unknown-linux-gnu.txt
  load_release

  if has apt-get; then
    download _amd64.deb "$sums"
    as_root apt-get install -y "$file"
  elif has dpkg; then
    download _amd64.deb "$sums"
    as_root dpkg -i "$file"
  else
    download _x86_64.appimage "$sums"
    bin_dir="$HOME/.local/bin"
    mkdir -p "$bin_dir"
    cp "$file" "$bin_dir/sdf-flash-gui"
    chmod 755 "$bin_dir/sdf-flash-gui"
    say "Installed $bin_dir/sdf-flash-gui"
    case ":$PATH:" in
      *":$bin_dir:"*) ;;
      *) say "Add $bin_dir to PATH to run sdf-flash-gui from a shell." ;;
    esac
  fi
}

cleanup() {
  if [ "${mounted:-0}" = 1 ]; then
    hdiutil detach -quiet "$mnt" >/dev/null 2>&1 || true
  fi
  rm -rf "$tmp"
}

main() {
  case "$(uname -s)" in
    Darwin | Linux) ;;
    MINGW* | MSYS* | CYGWIN*) fail "on Windows, run this in PowerShell instead: $ps_command" ;;
    *) fail "unsupported system $(uname -s); see $releases_url" ;;
  esac

  tmp=$(mktemp -d)
  chmod 755 "$tmp"
  trap cleanup EXIT

  case "$(uname -s)" in
    Darwin) install_macos ;;
    Linux) install_linux ;;
  esac
  warn_if_no_makemkv
}

main "$@"
