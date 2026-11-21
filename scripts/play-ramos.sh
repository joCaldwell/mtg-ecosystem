#!/usr/bin/env bash
# Run from the desired committed engine revision with eight registered singleton
# reparse catalog manifests. The detached checkout, seed-zero match and verified
# replay stay under the printed evidence directory, including on failure.
set -euo pipefail
if [[ $# != 10 ]]; then
    echo "Usage: $0 COMPILER SHA256 MANIFEST1 ... MANIFEST8" >&2
    exit 2
fi
root=$(git rev-parse --show-toplevel)
binary=$(realpath -e "$1")
expected=$2
shift 2
manifests=()
for manifest in "$@"; do manifests+=("$(realpath -e "$manifest")"); done
cd "$root"
mkdir -p generated
run=$(mktemp -d "$root/generated/ramos-play.XXXXXXXX")
echo "Evidence: $run" >&2
git worktree add --detach "$run/source" HEAD
cd "$run/source"
cp "$binary" "$run/compiler"
binary=$run/compiler
[[ $(sha256sum < "$binary" | cut -d' ' -f1) == "$expected" ]] || { echo "Compiler SHA-256 mismatch" >&2; exit 1; }
lock=$(sh scripts/engine-env.sh python3 -B scripts/engine-cache.py lock)
mkdir -p "$(dirname "$lock")"
exec 9>"$lock"
flock -x 9
fixtures=packages/game-engine/tests/fixtures
stage=$PWD/generated/stage
python3 -B "$fixtures/ramos/prepare.py" "$binary" "$stage" "${manifests[@]}"
"$binary" catalog build "$stage/catalogs/ramos.json"
# Install only new files, with the existing policy checked byte for byte.
for policy in "$stage"/commander-banned-*.json; do cmp "$policy" "$fixtures/${policy##*/}"; done
for file in catalogs/ramos.json catalogs/ramos.sha256 expected-ramos-ir.json profiles/commander-ramos-v1.json records; do
    [[ ! -e "$fixtures/$file" && ! -L "$fixtures/$file" ]]
done
mkdir "$fixtures/records"
cp -a "$stage/records/." "$fixtures/records/"
for file in catalogs/ramos.json catalogs/ramos.sha256 expected-ramos-ir.json profiles/commander-ramos-v1.json; do
    ln "$stage/$file" "$fixtures/$file"
done
sh scripts/engine-env.sh python3 -B scripts/engine-cache.py check
sh scripts/engine-env.sh cargo build --locked --profile quick -p mtg-sim --message-format=json > "$run/build.jsonl"
sim=$(jq -rs '[.[] | select(.reason=="compiler-artifact" and .target.name=="mtg-sim" and .executable!=null) | .executable] | last // error("missing sim executable")' "$run/build.jsonl")
cp "$sim" "$run/mtg-sim"
exec 9>&-
"$run/mtg-sim" match-game commander-ramos-v1 0 > "$run/game.json"
jq -e '.outcome=="finished" or .outcome=="draw"' "$run/game.json" >/dev/null
"$run/mtg-sim" match-replay commander-ramos-v1 "$run/game.json" > "$run/replay.json"
jq -e --slurpfile game "$run/game.json" '.status=="match_replay_verified" and .state_sha256==$game[0].final_state_sha256' "$run/replay.json"
