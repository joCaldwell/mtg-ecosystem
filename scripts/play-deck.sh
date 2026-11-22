#!/usr/bin/env bash
# Stage any 100%-compiled deck from Archive R4 and play/replay one seeded,
# headless four-seat match from the committed HEAD:
#
#   scripts/play-deck.sh "Jo's Cascade" [SEED] [stage-deck.py options...]
#   scripts/play-deck.sh --decklist deck.json [SEED] [stage-deck.py options...]
#
# A detached checkout of HEAD under the printed evidence directory builds release
# mtg-sim, stages the deck with scripts/stage-deck.py, installs only new fixture
# files (the Commander policy is compared byte for byte), rebuilds mtg-sim with
# the embedded profile, then runs match-game and match-replay. The checkout, its
# target, the stage, game and replay stay under the evidence directory, including
# on failure; retire it through docs/game-engine-storage.md.
set -euo pipefail
if [[ $# -lt 1 ]]; then
    echo "Usage: $0 DECK|--decklist FILE [SEED] [stage-deck.py options...]" >&2
    exit 2
fi
if [[ $1 == --decklist ]]; then
    select=(--decklist "$(realpath -e "$2")")
    shift 2
else
    select=(--deck "$1")
    shift
fi
seed=0
if [[ $# -gt 0 && $1 =~ ^[0-9]+$ ]]; then
    seed=$1
    shift
fi
root=$(git rev-parse --show-toplevel)
# Resolve shared inputs before leaving this checkout.
options=()
while [[ $# -gt 0 ]]; do
    case $1 in
        --decks|--archive) options+=("$1" "$(realpath -e "$2")"); shift 2 ;;
        *) echo "Unknown option $1" >&2; exit 2 ;;
    esac
done
cd "$root"
mkdir -p generated
run=$(mktemp -d "$root/generated/deck-play.XXXXXXXX")
echo "Evidence: $run" >&2
git worktree add --detach "$run/source" HEAD >&2
cd "$run/source"
build_sim() {
    sh scripts/engine-env.sh sh -ec 'lock=$(python3 -B scripts/engine-cache.py lock); mkdir -p "$(dirname -- "$lock")";
        exec flock "$lock" cargo build --locked --release -p mtg-sim --message-format=json' > "$run/$1.jsonl"
    jq -rs '[.[] | select(.reason=="compiler-artifact" and .target.name=="mtg-sim" and .executable!=null) | .executable] | last // error("missing sim executable")' "$run/$1.jsonl"
}
cp "$(build_sim compiler-build)" "$run/compiler"
# stage-deck.py resolves default inputs from the primary checkout's generated/.
profile=$(python3 -B "$root/scripts/stage-deck.py" "$run/compiler" "$run/stage" "${select[@]}" "${options[@]}")
fixtures=packages/game-engine/tests/fixtures
for policy in "$run/stage"/commander-banned-*.json; do cmp "$policy" "$fixtures/${policy##*/}"; done
(cd "$run/stage" && find . -type f ! -name 'commander-banned-*.json' ! -name producer.json) | while read -r file; do
    [[ ! -e "$fixtures/$file" && ! -L "$fixtures/$file" ]] || { echo "Refusing to replace $fixtures/$file" >&2; exit 1; }
    mkdir -p "$(dirname "$fixtures/$file")"
    cp "$run/stage/$file" "$fixtures/$file"
done
cp "$(build_sim sim-build)" "$run/mtg-sim"
"$run/mtg-sim" match-game "$profile" "$seed" > "$run/game.json"
jq -e '.outcome=="finished" or .outcome=="draw"' "$run/game.json" >/dev/null
"$run/mtg-sim" match-replay "$profile" "$run/game.json" > "$run/replay.json"
jq -e --slurpfile game "$run/game.json" '.status=="match_replay_verified" and .state_sha256==$game[0].final_state_sha256' "$run/replay.json"
echo "Verified $profile seed $seed: $run" >&2
