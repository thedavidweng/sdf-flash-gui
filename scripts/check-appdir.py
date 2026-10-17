import stat
import sys
from pathlib import Path


def check_appdir(appdir):
    appdir = Path(appdir).resolve()
    for relative in ('AppRun', 'usr/bin/sdf-flash-gui'):
        executable = appdir / relative
        if not executable.is_file():
            raise FileNotFoundError(executable)
        if stat.S_IMODE(executable.stat().st_mode) & 0o555 != 0o555:
            raise PermissionError(f'{executable}: requires read and execute permissions for all users')
        parent = executable.parent
        while True:
            if stat.S_IMODE(parent.stat().st_mode) & 0o111 != 0o111:
                raise PermissionError(f'{parent}: requires directory traversal for all users')
            if parent == appdir:
                break
            parent = parent.parent


if __name__ == '__main__':
    check_appdir(sys.argv[1])
