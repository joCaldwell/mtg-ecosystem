#!/bin/sh
# Internal entry point, called by engine-bootstrap.sh with the selected environment.
set -eu
engine_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$engine_root"
node scripts/engine-doctor.ts --preflight
if [ -f scripts/engine-cache.py ]; then
    python3 scripts/engine-cache.py check
fi
echo 'Installing locked root + engine + parser dependencies (npm ci; install scripts disabled).'
# npm ci removes the selected install tree; do not run alongside another npm install.
# https://docs.npmjs.com/cli/v12/commands/npm-ci
npm ci --workspace=@mtg-ecosystem/game-engine --workspace=@mtg-ecosystem/oracle-parser --include-workspace-root --include=dev --ignore-scripts --no-audit --no-fund
node scripts/engine-doctor.ts
echo 'Ready. Run: sh scripts/engine-env.sh npm run check:engine:source'
