"""Compatibility entry point: start the already-deployed notification unit."""
import os
from pathlib import Path


def main():
    root = Path(__file__).resolve().parent.parent
    binary = root / 'native/foundation/target/release/desktop-foundationctl'
    os.execv(binary, [str(binary), '--root', str(root), 'notifications', 'start'])


if __name__ == '__main__':
    main()
