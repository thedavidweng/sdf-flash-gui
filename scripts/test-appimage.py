import importlib.util
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parent.parent


class AppImageTests(unittest.TestCase):
    def test_cached_apprun_is_executable_by_non_owner_after_staging(self):
        with tempfile.TemporaryDirectory() as directory:
            cache = Path(directory)
            source = cache / '.cargo-packager/AppImage/AppRun-x86_64'
            source.parent.mkdir(parents=True)
            source.write_text('#!/bin/sh\nexit 0\n')
            source.chmod(0o764)
            subprocess.run(['bash', str(ROOT / 'scripts/prepare-appimage.sh')],
                           env={**os.environ, 'XDG_CACHE_HOME': directory}, check=True)
            staged = cache / 'AppRun'
            shutil.copy2(source, staged)
            self.assertEqual(staged.stat().st_mode & 0o555, 0o555)

    def test_appdir_rejects_owner_only_entry_and_accepts_public_entry(self):
        spec = importlib.util.spec_from_file_location('check_appdir', ROOT / 'scripts/check-appdir.py')
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        with tempfile.TemporaryDirectory() as directory:
            appdir = Path(directory)
            appdir.chmod(0o755)
            binary = appdir / 'usr/bin/sdf-flash-gui'
            binary.parent.mkdir(parents=True)
            binary.write_text('binary')
            binary.chmod(0o755)
            entry = appdir / 'AppRun'
            entry.write_text('entry')
            entry.chmod(0o744)
            with self.assertRaises(PermissionError):
                module.check_appdir(appdir)
            entry.chmod(0o755)
            module.check_appdir(appdir)
            binary.chmod(0o744)
            with self.assertRaises(PermissionError):
                module.check_appdir(appdir)
            binary.chmod(0o755)
            binary.parent.chmod(0o700)
            with self.assertRaises(PermissionError):
                module.check_appdir(appdir)


if __name__ == '__main__':
    unittest.main()
