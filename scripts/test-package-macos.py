#!/usr/bin/env python3
"""Run the release packager with controlled build and Apple services."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


class PackagingTests(unittest.TestCase):
    def run_package(self, target, rejected=""):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            scripts = root / "scripts"
            scripts.mkdir()
            for name in ["package-macos.sh", "sign-and-notarize.sh"]:
                source = Path(__file__).with_name(name)
                if source.exists():
                    shutil.copy(source, scripts / name)
            tools = root / "tools"
            tools.mkdir()
            for tool in ["cargo", "codesign", "ditto", "xcrun", "spctl", "hdiutil"]:
                mock = tools / tool
                mock.write_text('''#!/bin/bash
set -eu
printf '%s\\n' "$(basename "$0") $*" >> "$CALL_LOG"
case "$(basename "$0") $1" in
  "cargo metadata") echo '{"packages":[{"name":"sdf-flash-gui","version":"1.2.3"}]}' ;;
  "cargo packager") mkdir -p 'dist/SDF Flash GUI.app/Contents/MacOS'; touch 'dist/SDF Flash GUI.app/Contents/MacOS/sdf-flash-gui' ;;
  "codesign --display") echo 'TeamIdentifier=TESTTEAM' >&2 ;;
  "xcrun notarytool")
    status=Accepted
    if [[ -n "$REJECTED" && "$3" == *"$REJECTED" ]]; then status=Invalid; fi
    printf '{"status":"%s"}\\n' "$status" ;;
  "ditto -c") touch "${@: -1}" ;;
  "ditto dist/SDF Flash GUI.app") cp -R "$1" "$2" ;;
  "hdiutil create") touch "${@: -1}" ;;
esac
''')
                mock.chmod(0o755)
            log = root / "calls"
            env = dict(os.environ, PATH=f"{tools}:{os.environ['PATH']}", CALL_LOG=str(log),
                       REJECTED=rejected, APPLE_SIGNING_IDENTITY="Developer ID Application: Test (TESTTEAM)",
                       APPLE_TEAM_ID="TESTTEAM", APPLE_SIGNING_KEYCHAIN="test.keychain-db",
                       APPLE_API_KEY_PATH="test.p8", APPLE_API_KEY_ID="KEYID", APPLE_API_ISSUER="issuer")
            result = subprocess.run(["bash", str(scripts / "package-macos.sh"), target],
                                    cwd=root, env=env, capture_output=True, text=True)
            sums = root / "dist" / f"SHA256SUMS-{target}.txt"
            return result, log.read_text() if log.exists() else "", sums.read_text() if sums.exists() else ""

    def test_both_architectures_package_the_accepted_app_before_checksums(self):
        for target, name in [("aarch64-apple-darwin", "SDF.Flash.GUI_1.2.3_aarch64.dmg"),
                             ("x86_64-apple-darwin", "SDF.Flash.GUI_1.2.3_x64.dmg")]:
            with self.subTest(target=target):
                result, calls, sums = self.run_package(target)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertLess(calls.index("stapler validate dist/SDF Flash GUI.app"), calls.index("hdiutil create"))
                self.assertIn(f"stapler validate dist/{name}", calls)
                self.assertEqual(len(sums.splitlines()), 1)
                self.assertTrue(sums.rstrip().endswith(name), sums)

    def test_rejected_app_stops_before_dmg_creation(self):
        result, calls, sums = self.run_package("aarch64-apple-darwin", rejected="app.zip")
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("hdiutil", calls)
        self.assertEqual(sums, "")

    def test_rejected_dmg_has_no_publishable_checksums(self):
        result, calls, sums = self.run_package("aarch64-apple-darwin", rejected=".dmg")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("hdiutil create", calls)
        self.assertEqual(sums, "")


if __name__ == "__main__":
    unittest.main()
