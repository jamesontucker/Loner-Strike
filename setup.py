#!/usr/bin/env python3
"""Loner-Strike setup. Run:  python3 setup.py   (or  python3 setup.py --help)"""

import sys

if sys.version_info < (3, 8):
    sys.exit("Loner-Strike needs Python 3.8 or newer.")

from installer.main import main

if __name__ == "__main__":
    main()
