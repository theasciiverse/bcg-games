#!/bin/sh
# Records the 100x30 catalog previews, asciiverse/<id>.cast, with hub's
# castrec.py: each game started with --demo --mute (it plays itself and
# never opens the sound device), for SECONDS_EACH (default 45), then quit.
#   sh asciiverse/record-previews.sh [id ...]   (default: all of them)
set -eu
cd "$(dirname "$0")/.."
here=$(pwd)
# hub sits beside this repo, or two folders up on the main machine
hub=../hub
[ -d "$hub/tools" ] || hub=../../hub
castrec=$(cd "$hub/tools" && pwd)/castrec.py
secs=${SECONDS_EACH:-45}
cargo build --release
ids=${*:-hammurabi lunar-descent animal hexapawn bagels orbit splat civil-war basketball football boxing bullfight golf acey-ducey king fur-trader 3d-tic-tac-toe hide-and-seek}
for id in $ids; do
  tmp=$(mktemp -d)
  cp "target/release/$id" "$tmp/"
  (cd "$tmp" && HOME="$tmp" XDG_STATE_HOME="$tmp/state" \
    python3 "$castrec" "$here/asciiverse/$id.cast" . "w$secs;kq;w0.5" -- "./$id" --demo --mute --seed 7)
  rm -rf "$tmp"
done
