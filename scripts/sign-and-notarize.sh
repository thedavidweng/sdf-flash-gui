#!/bin/bash
set -euo pipefail

artifact="${1:?provide an app or DMG}"
: "${APPLE_SIGNING_IDENTITY:?}" "${APPLE_SIGNING_KEYCHAIN:?}" "${APPLE_TEAM_ID:?}"
: "${APPLE_API_KEY_PATH:?}" "${APPLE_API_KEY_ID:?}" "${APPLE_API_ISSUER:?}"
case "$APPLE_SIGNING_IDENTITY" in
  "Developer ID Application: "*) ;;
  *) echo "error: release requires a Developer ID Application identity" >&2; exit 1 ;;
esac

work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT
sign_args=(--force --timestamp --sign "$APPLE_SIGNING_IDENTITY" --keychain "$APPLE_SIGNING_KEYCHAIN")
if [[ "$artifact" == *.app ]]; then
  sign_args+=(--options runtime)
fi
codesign "${sign_args[@]}" "$artifact"
codesign --verify --deep --strict --verbose=2 "$artifact"
codesign --display --verbose=4 "$artifact" 2>&1 | grep -Fx "TeamIdentifier=$APPLE_TEAM_ID"

submission="$artifact"
if [[ "$artifact" == *.app ]]; then
  submission="$work_dir/app.zip"
  ditto -c -k --keepParent "$artifact" "$submission"
fi
xcrun notarytool submit "$submission" \
  --key "$APPLE_API_KEY_PATH" --key-id "$APPLE_API_KEY_ID" --issuer "$APPLE_API_ISSUER" \
  --wait --timeout 30m --output-format json > "$work_dir/result.json"
cat "$work_dir/result.json"
jq -e '.status == "Accepted"' "$work_dir/result.json" > /dev/null
xcrun stapler staple "$artifact"
xcrun stapler validate "$artifact"
if [[ "$artifact" == *.app ]]; then
  spctl --assess --type execute --verbose=2 "$artifact"
else
  spctl --assess --type open --context context:primary-signature --verbose=2 "$artifact"
fi
