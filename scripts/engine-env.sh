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
# entry points select this worktree's own target, then take that target's lock.
if [ -f "$engine_root/scripts/engine-cache.py" ] && { [ -n "${CARGO_TARGET_DIR:-}" ] || rustc --version >/dev/null 2>&1; }; then
    CARGO_TARGET_DIR=$(python3 "$engine_root/scripts/engine-cache.py" path)
    export CARGO_TARGET_DIR
fi
# CARGO_INCREMENTAL stays unset unless chosen: Cargo profiles make dev/test/quick
# incremental and keep release (retained binaries) non-incremental.
# https://doc.rust-lang.org/cargo/reference/profiles.html#incremental
# Worktrees build concurrently, so bound each one's jobserver (rustc processes and
# LLVM threads): a quarter of the CPUs, at least 4. Measured on 32 threads/30 GiB:
# jobs 8 keeps a cold contracts/compiler test build near 5.8 GiB instead of 8.1 GiB
# for ~7% more solo wall time. An explicit CARGO_BUILD_JOBS wins.
if [ -z "${CARGO_BUILD_JOBS:-}" ]; then
    engine_cpus=$(nproc 2>/dev/null || echo 4)
    CARGO_BUILD_JOBS=$((engine_cpus / 4))
    [ "$CARGO_BUILD_JOBS" -ge 4 ] || CARGO_BUILD_JOBS=4
    [ "$CARGO_BUILD_JOBS" -le "$engine_cpus" ] || CARGO_BUILD_JOBS=$engine_cpus
    export CARGO_BUILD_JOBS
fi
if [ "$#" -eq 0 ]; then
    echo 'Usage: sh scripts/engine-env.sh COMMAND [ARG ...]' >&2
    exit 2
fi
exec "$@"
