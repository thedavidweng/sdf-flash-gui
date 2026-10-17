import hashlib
import subprocess
import sys
from pathlib import Path


def check_update(image):
    image = Path(image)
    expected = 'gh-releases-zsync|thedavidweng|sdf-flash-gui|latest|sdf-flash-gui_*_x86_64.AppImage.zsync'
    result = subprocess.run([str(image.resolve()), '--appimage-updateinformation'],
                            capture_output=True, text=True, check=True)
    if (result.stdout + result.stderr).strip() != expected:
        raise ValueError('AppImage update transport does not select this release channel')
    header = image.with_name(image.name + '.zsync').read_bytes().split(b'\n\n', 1)[0].decode()
    fields = dict(line.split(': ', 1) for line in header.splitlines() if ': ' in line)
    if fields.get('SHA-1') != hashlib.sha1(image.read_bytes()).hexdigest():
        raise ValueError('zsync checksum does not match final AppImage')
    if fields.get('Filename') != image.name:
        raise ValueError('zsync filename does not match AppImage asset')


if __name__ == '__main__':
    check_update(sys.argv[1])
