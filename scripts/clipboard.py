#!/usr/bin/env python3
"""Compatibility entry point for units generated before the Rust migration.

New units and QML invoke scripts/foundation directly. Keep this small exec shim
so an existing generated unit remains usable until its next normal deployment.
"""
import os
from pathlib import Path
import sys

if __name__ == '__main__':
    wrapper = str(Path(__file__).resolve().with_name('foundation'))
    os.execv(wrapper, [wrapper, 'clipboard', *sys.argv[1:]])
