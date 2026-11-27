#!/bin/sh
# Compile Card IR from one frozen input manifest (docs/game-engine-catalogs.md#frozen-input-compile).
#
#   sh scripts/compile-oracle.sh --inputs FROZEN_MANIFEST --out NEW_DIR
#   sh scripts/compile-oracle.sh identity --inputs FROZEN_MANIFEST
#
# Only the manifest's pinned content affects output bytes. Environment only:
#   MTG_COMPILE_CACHE  reader cache holding the pinned archive (default: this checkout's generated/)
#   MTG_COMPILE_SIM    prebuilt mtg-sim (default: a locked release build of this checkout)
# Exit 0: complete verified bundle (or verified identity); 1: input, registration,
# reader, compiler or destination failure, named on stderr; 2: usage.
set -eu

usage() {
    sed -n '2,11s/^# \{0,1\}//p' "$0"
}
bad() {
    echo "compile-oracle: $1" >&2
    usage >&2
    exit 2
}

mode=compile
if [ "${1:-}" = identity ]; then
    mode=identity
    shift
fi
inputs=
out=
while [ "$#" -gt 0 ]; do
    case $1 in
        -h|--help) usage; exit 0 ;;
        --inputs)
            [ "$#" -ge 2 ] && [ -z "$inputs" ] && [ -n "$2" ] || bad 'one nonempty --inputs is required'
            inputs=$2; shift 2 ;;
        --out)
            [ "$mode" = compile ] || bad 'identity takes no --out'
            [ "$#" -ge 2 ] && [ -z "$out" ] && [ -n "$2" ] || bad 'one nonempty --out is required'
            out=$2; shift 2 ;;
        *) bad "unknown argument $1" ;;
    esac
done
[ -n "$inputs" ] || bad 'missing --inputs'
[ "$mode" = identity ] || [ -n "$out" ] || bad 'missing --out'

# The build below runs from the checkout root: anchor relative paths first.
absolute() {
    case $1 in /*) printf '%s\n' "$1" ;; *) printf '%s/%s\n' "$PWD" "$1" ;; esac
}
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)
inputs=$(absolute "$inputs")
[ -z "$out" ] || out=$(absolute "$out")
cache=$(absolute "${MTG_COMPILE_CACHE:-$root/generated}")
set -- catalog "$mode" --inputs "$inputs" --cache "$cache"
[ "$mode" = identity ] || set -- "$@" --out "$out"

if [ -n "${MTG_COMPILE_SIM:-}" ]; then
    [ -f "$MTG_COMPILE_SIM" ] && [ -x "$MTG_COMPILE_SIM" ] || {
        echo "compiler: MTG_COMPILE_SIM is not an executable file: $MTG_COMPILE_SIM" >&2
        exit 1
    }
    exec "$MTG_COMPILE_SIM" "$@"
fi
# Build and run under this checkout's target lock, so a concurrent build in the
# same target cannot replace the producer between build and run. Cargo output
# goes to stderr; stdout carries only the report.
cd "$root"
exec sh scripts/engine-env.sh sh -ec '
    lock=$(python3 -B scripts/engine-cache.py lock)
    mkdir -p "$(dirname -- "$lock")"
    exec 9>>"$lock"
    flock 9
    MTG_ENGINE_LOCK_FD=9 python3 -B scripts/engine-cache.py check >&2
    cargo build --locked --release -p mtg-sim >&2 || { echo "compiler: mtg-sim build failed" >&2; exit 1; }
    exec "$CARGO_TARGET_DIR/release/mtg-sim" "$@"
' compile-oracle "$@"
