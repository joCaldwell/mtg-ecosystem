#!/bin/sh
# Incremental edit loop in this worktree's own target (engine-cache.py path).
# The quick profile is incremental by default; Cargo keeps it beside the test
# profile in one target. https://doc.rust-lang.org/cargo/reference/build-cache.html
set -eu

case ${1:-} in
    check|build|test) action=$1; shift ;;
    *) echo 'Usage: npm run engine:dev -- {check|build|test} [CARGO_ARGS...]' >&2; exit 2 ;;
esac

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)
cd "$root"
# engine-env selects the target and its lock; only commands sharing that target wait.
exec sh scripts/engine-env.sh sh -ec '
    lock=$(python3 -B scripts/engine-cache.py lock)
    mkdir -p "$(dirname -- "$lock")"
    exec flock "$lock" sh -ec "python3 -B scripts/engine-cache.py check >&2; exec cargo \"\$@\"" engine-dev "$@"
' engine-dev "$action" --locked --profile quick "$@"
