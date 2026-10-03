#!/usr/bin/env python3
"""Exec-only compatibility entry point; normal bindings call Rust directly."""
import os
from pathlib import Path
import sys

if __name__ == '__main__':
    wrapper = str(Path(__file__).resolve().with_name('foundation'))
    os.execv(wrapper, [wrapper, 'volume', *sys.argv[1:]])
