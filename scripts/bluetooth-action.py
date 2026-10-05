#!/usr/bin/env python3
"""Compatibility entry point; Bluetooth actions live in the shared Rust backend."""
import os
from pathlib import Path
import sys

if __name__ == '__main__':
    wrapper = str(Path(__file__).resolve().with_name('foundation'))
    os.execv(wrapper, [wrapper, 'bluetooth', *sys.argv[1:]])
