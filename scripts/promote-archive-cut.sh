#!/bin/sh
set -eu
exec python3 -B "$(dirname -- "$0")/compare-archive-cut.py" promote "$@"
