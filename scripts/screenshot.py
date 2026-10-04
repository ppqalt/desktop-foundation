#!/usr/bin/env python3
"""Compatibility entry point for older screenshot callers; no capture logic."""
import os
from pathlib import Path
import sys

if __name__ == '__main__':
    wrapper = str(Path(__file__).resolve().with_name('foundation'))
    os.execv(wrapper, [wrapper, 'screenshot', *sys.argv[1:]])
