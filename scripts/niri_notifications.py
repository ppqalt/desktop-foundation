"""Compatibility entry point: start the already-deployed notification unit."""
import os
import subprocess


def main():
    if not os.environ.get('NIRI_SOCKET'):
        raise RuntimeError('Notification startup requires a Niri session')
    subprocess.run(['systemctl', '--user', 'start', 'desktop-foundation-notifications.service'], check=True)


if __name__ == '__main__':
    main()
