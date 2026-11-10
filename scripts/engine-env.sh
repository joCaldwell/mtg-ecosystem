#!/bin/sh
# Process-local tool selection. No installation, shell startup edits or cache cleanup.
set -eu
engine_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
engine_tools=${MTG_ENGINE_TOOLCHAIN_ROOT:-"$HOME/.local/share/mtg-ecosystem/engine-toolchains/rust1990-node26111-v1"}
if [ -d "$engine_tools/usr/bin" ]; then
    PATH="$engine_tools/usr/bin:$PATH"
    LD_LIBRARY_PATH="$engine_tools/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
    export PATH LD_LIBRARY_PATH
elif [ -n "${MTG_ENGINE_TOOLCHAIN_ROOT:-}" ]; then
    echo 'Engine toolchain override is unavailable: expected usr/bin beneath MTG_ENGINE_TOOLCHAIN_ROOT.' >&2
    echo 'See docs/game-engine-development.md#toolchain-setup; nothing was installed.' >&2
    exit 1
fi
# rustup proxies must not download a toolchain while probing or running a command.
RUSTUP_TOOLCHAIN=$(sed -n 's/^channel = "\([^"]*\)"/\1/p' "$engine_root/rust-toolchain.toml")
RUSTUP_AUTO_INSTALL=0
export RUSTUP_TOOLCHAIN RUSTUP_AUTO_INSTALL
engine_node=$(cat "$engine_root/.node-version")
if [ "$(node --version 2>/dev/null || true)" != "v$engine_node" ]; then
    echo "Engine commands require Node $engine_node (.node-version)." >&2
    echo 'Select existing tools with MTG_ENGINE_TOOLCHAIN_ROOT; see docs/game-engine-development.md#toolchain-setup.' >&2
    exit 1
fi
# Source-only tools and doctor remain available when Rust is absent. Native
# entry points select and validate their target before taking the build lock.
if [ -f "$engine_root/scripts/engine-cache.py" ] && { [ -n "${CARGO_TARGET_DIR:-}" ] || rustc --version >/dev/null 2>&1; }; then
    CARGO_TARGET_DIR=$(python3 "$engine_root/scripts/engine-cache.py" path)
    export CARGO_TARGET_DIR
fi
CARGO_INCREMENTAL=${CARGO_INCREMENTAL:-0}
export CARGO_INCREMENTAL
if [ "$#" -eq 0 ]; then
    echo 'Usage: sh scripts/engine-env.sh COMMAND [ARG ...]' >&2
    exit 2
fi
exec "$@"
