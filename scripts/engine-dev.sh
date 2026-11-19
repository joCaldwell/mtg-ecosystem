#!/bin/sh
# Cargo's incremental cache is most useful when successive builds reuse one checkout.
# https://doc.rust-lang.org/cargo/reference/build-cache.html
set -eu

case ${1:-} in
    check|build|test) action=$1; shift ;;
    *) echo 'Usage: npm run engine:dev -- {check|build|test} [CARGO_ARGS...]' >&2; exit 2 ;;
esac

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)
cd "$root"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/target/engine-dev}"
export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-1}"
lock="$HOME/.local/share/mtg-ecosystem/engine-verification/cargo.lock"
mkdir -p "$(dirname -- "$lock")"
exec sh scripts/engine-env.sh flock "$lock" sh -ec '
    python3 -B scripts/engine-cache.py check >&2
    exec cargo "$@"
' engine-dev "$action" --locked --profile quick "$@"
