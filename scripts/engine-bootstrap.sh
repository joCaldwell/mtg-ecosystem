#!/bin/sh
set -eu
engine_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
if [ "$#" -ne 0 ]; then
    echo 'Usage: sh scripts/engine-bootstrap.sh (installs locked engine/parser Node dependencies)' >&2
    exit 2
fi
# Re-exec once through the environment launcher so npm and all its children inherit it.
exec sh "$engine_root/scripts/engine-env.sh" sh "$engine_root/scripts/engine-bootstrap-install.sh"
