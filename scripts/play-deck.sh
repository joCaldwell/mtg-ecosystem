#!/usr/bin/env bash
# Stage any 100%-compiled deck from Archive R4 and play/replay one seeded,
# headless four-seat match from the committed HEAD:
#
#   scripts/play-deck.sh "Jo's Cascade" [SEED] [--decks FILE] [--archive R4.jsonl]
#   scripts/play-deck.sh --decklist deck.json [SEED] [--archive R4.jsonl]
#
# A detached checkout of HEAD under the printed evidence directory runs its own
# scripts/stage-deck.py and builds release mtg-sim: nothing from the invoking
# checkout's working tree enters the evidence except the resolved archive and
# deck inputs. Each build holds the checkout's target lock across the cache
# budget check, Cargo, executable selection and copy. Staging installs only new
# fixture files (the Commander policy is compared byte for byte); the rebuilt
# mtg-sim embeds the profile and runs match-game with --journal-dir inside the
# evidence directory, so live journals stay outside the checkout, then
# match-replay. The checkout, stage, journals, game and replay stay
# under the evidence directory, including on failure; retire the checkout as
# docs/game-engine-storage.md describes.
set -euo pipefail
if [[ $# -lt 1 ]]; then
    echo "Usage: $0 DECK|--decklist FILE [SEED] [--decks FILE] [--archive R4.jsonl]" >&2
    exit 2
fi
root=$(git rev-parse --show-toplevel)
# Default inputs: this checkout's generated/ file, else the primary checkout's.
shared() {
    if [[ -e "$root/generated/$1" ]]; then
        echo "$root/generated/$1"
    else
        echo "$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")/generated/$1"
    fi
}
decks=$(shared phase1-target-decks.json)
archive=$(shared oracle-ast-phase1-r4.jsonl)
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
while [[ $# -gt 0 ]]; do
    case $1 in
        --decks) decks=$2; shift 2 ;;
        --archive) archive=$2; shift 2 ;;
        *) echo "Unknown option $1" >&2; exit 2 ;;
    esac
done
# Resolve every external input before leaving the invoking checkout.
archive=$(realpath -e "$archive")
inputs=(--archive "$archive")
if [[ ${select[0]} == --deck ]]; then
    inputs+=(--decks "$(realpath -e "$decks")")
fi
cd "$root"
mkdir -p generated
run=$(mktemp -d "$root/generated/deck-play.XXXXXXXX")
echo "Evidence: $run" >&2
git worktree add --detach "$run/source" HEAD >&2
cd "$run/source"
lock=$(sh scripts/engine-env.sh python3 -B scripts/engine-cache.py lock)
mkdir -p "$(dirname -- "$lock")"
# build_sim LABEL DESTINATION: one admitted, locked release build and copy.
build_sim() {
    (
        flock -x 9
        sh scripts/engine-env.sh python3 -B scripts/engine-cache.py check --locked > "$run/$1-cache.json"
        sh scripts/engine-env.sh cargo build --locked --release -p mtg-sim --message-format=json > "$run/$1.jsonl"
        executable=$(jq -rs '[.[] | select(.reason=="compiler-artifact" and .target.name=="mtg-sim" and .executable!=null) | .executable] | last // error("missing sim executable")' "$run/$1.jsonl")
        cp "$executable" "$2"
    ) 9>"$lock"
}
build_sim compiler-build "$run/compiler"
profile=$(python3 -B scripts/stage-deck.py "$run/compiler" "$run/stage" "${select[@]}" "${inputs[@]}")
fixtures=packages/game-engine/tests/fixtures
for policy in "$run/stage"/commander-banned-*.json; do cmp "$policy" "$fixtures/${policy##*/}"; done
(cd "$run/stage" && find . -type f ! -name 'commander-banned-*.json' ! -name producer.json) | while read -r file; do
    [[ ! -e "$fixtures/$file" && ! -L "$fixtures/$file" ]] || { echo "Refusing to replace $fixtures/$file" >&2; exit 1; }
    mkdir -p "$(dirname "$fixtures/$file")"
    cp "$run/stage/$file" "$fixtures/$file"
done
build_sim sim-build "$run/mtg-sim"
# Live journals (accepted inputs, the only record of an interrupted match) land
# in $run/live-journals, outside the disposable detached checkout.
"$run/mtg-sim" match-game "$profile" "$seed" --journal-dir "$run/live-journals" > "$run/game.json"
jq -e '.outcome=="finished" or .outcome=="draw"' "$run/game.json" >/dev/null
"$run/mtg-sim" match-replay "$profile" "$run/game.json" > "$run/replay.json"
jq -e --slurpfile game "$run/game.json" '.status=="match_replay_verified" and .state_sha256==$game[0].final_state_sha256' "$run/replay.json"
echo "Verified $profile seed $seed: $run" >&2
