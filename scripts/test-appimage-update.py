import hashlib
import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('check_update', Path(__file__).with_name('check-appimage-update.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class UpdateTests(unittest.TestCase):
    def test_modified_image_cannot_reuse_old_delta_control_file(self):
        with tempfile.TemporaryDirectory() as directory:
            image = Path(directory) / 'sdf-flash-gui_1_x86_64.AppImage'
            image.write_text("#!/bin/sh\necho 'gh-releases-zsync|thedavidweng|sdf-flash-gui|latest|sdf-flash-gui_*_x86_64.AppImage.zsync' >&2\n")
            image.chmod(0o755)
            control = image.with_name(image.name + '.zsync')
            control.write_text(f'Filename: {image.name}\nSHA-1: {hashlib.sha1(image.read_bytes()).hexdigest()}\n\n')
            module.check_update(image)
            image.write_text(image.read_text() + '\n')
            with self.assertRaises(ValueError):
                module.check_update(image)


if __name__ == '__main__':
    unittest.main()
