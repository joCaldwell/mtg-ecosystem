#!/bin/sh
# Incremental edit loop in this worktree's own target (engine-cache.py path),
# on the same dev/test profile and feature shape as check:engine and the gate,
# so it reuses their artifacts. A selection that builds mtg-core gets
# `--features mtg-core/testkit`; a contracts/compiler/oracle-ast-only selection
# never compiles core and cannot name its feature.
# https://doc.rust-lang.org/cargo/reference/build-cache.html
set -eu

case ${1:-} in
    check|build|test|clippy) action=$1; shift ;;
    *) echo 'Usage: npm run engine:dev -- {check|build|test|clippy} [CARGO_ARGS...]' >&2; exit 2 ;;
esac

core=no
packages=0
previous=
for arg in "$@"; do
    [ "$arg" = -- ] && break
    package=
    case $previous in -p|--package) package=$arg ;; esac
    case $arg in
        --package=*) package=${arg#--package=} ;;
        -p?*) package=${arg#-p} ;;
        --workspace) core=yes ;;
    esac
    if [ -n "$package" ]; then
        packages=$((packages + 1))
        case $package in mtg-contracts|mtg-compiler|mtg-oracle-ast) ;; *) core=yes ;; esac
    fi
    previous=$arg
done
[ "$packages" -gt 0 ] || core=yes
if [ "$core" = yes ]; then
    set -- --features mtg-core/testkit "$@"
fi

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)
cd "$root"
# engine-env selects the target and its lock; only commands sharing that target
# wait. check runs under the lock, so an over-budget target is evicted first.
exec sh scripts/engine-env.sh sh -ec '
    lock=$(python3 -B scripts/engine-cache.py lock)
    mkdir -p "$(dirname -- "$lock")"
    exec flock "$lock" sh -ec "python3 -B scripts/engine-cache.py check --locked >&2; exec cargo \"\$@\"" engine-dev "$@"
' engine-dev "$action" --locked "$@"
