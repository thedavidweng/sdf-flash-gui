#!/usr/bin/env python3
"""Exercise the release gate with controlled Apple tool responses."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


class NotarizationTests(unittest.TestCase):
    def run_gate(self, artifact_name, status="Accepted", identity="Developer ID Application: Test (TESTTEAM)", team="TESTTEAM", verify_exit="0"):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            artifact = root / artifact_name
            artifact.mkdir() if artifact.suffix == ".app" else artifact.touch()
            log = root / "calls"
            for tool in ["codesign", "ditto", "xcrun", "spctl"]:
                mock = root / tool
                mock.write_text('''#!/bin/bash
printf '%s\n' "$(basename "$0") $*" >> "$CALL_LOG"
case "$(basename "$0") $1" in
  "codesign --verify") exit "$VERIFY_EXIT" ;;
  "codesign --display") echo 'TeamIdentifier=TESTTEAM' >&2 ;;
  "xcrun notarytool") printf '{"status":"%s"}\n' "$NOTARY_STATUS" ;;
esac
''')
                mock.chmod(0o755)
            env = dict(os.environ, PATH=f"{root}:{os.environ['PATH']}", CALL_LOG=str(log),
                       NOTARY_STATUS=status, APPLE_SIGNING_IDENTITY=identity,
                       VERIFY_EXIT=verify_exit, APPLE_TEAM_ID=team, APPLE_SIGNING_KEYCHAIN="test.keychain-db",
                       APPLE_API_KEY_PATH="test-api-key.p8", APPLE_API_KEY_ID="KEYID",
                       APPLE_API_ISSUER="issuer")
            result = subprocess.run(["bash", str(Path(__file__).with_name("sign-and-notarize.sh")),
                                     str(artifact)], env=env, capture_output=True, text=True)
            return result, log.read_text() if log.exists() else ""

    def test_app_and_dmg_are_accepted_before_stapling(self):
        for name in ["Test App.app", "Test App.dmg"]:
            with self.subTest(name=name):
                result, calls = self.run_gate(name)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn("--timestamp", calls)
                self.assertIn("codesign --verify --deep --strict", calls)
                self.assertLess(calls.index("notarytool submit"), calls.index("stapler staple"))
                self.assertIn("stapler validate", calls)
                self.assertIn("spctl --assess", calls)
                self.assertEqual("ditto" in calls, name.endswith(".app"))
                self.assertEqual("--type execute" in calls, name.endswith(".app"))

    def test_invalid_submission_never_staples_or_assesses(self):
        result, calls = self.run_gate("Test.app", status="Invalid")
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("stapler", calls)
        self.assertNotIn("spctl", calls)

    def test_wrong_team_never_submits(self):
        result, calls = self.run_gate("Test.app", team="WRONGTEAM")
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("notarytool", calls)

    def test_invalid_signature_never_submits(self):
        result, calls = self.run_gate("Test.app", verify_exit="1")
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("notarytool", calls)

    def test_ad_hoc_identity_is_rejected(self):
        result, calls = self.run_gate("Test.app", identity="-")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(calls, "")


if __name__ == "__main__":
    unittest.main()
