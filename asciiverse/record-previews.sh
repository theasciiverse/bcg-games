#!/bin/sh
# Records the 100x30 catalog previews, asciiverse/<id>.cast, with hub's
# castrec.py: each game started with --demo --mute (it plays itself and
# never opens the sound device), for SECONDS (default 45), then quit.
#   sh asciiverse/record-previews.sh [id ...]   (default: all eight)
set -eu
cd "$(dirname "$0")/.."
here=$(pwd)
castrec=$(cd ../../hub/tools && pwd)/castrec.py
secs=${SECONDS_EACH:-45}
cargo build --release
ids=${*:-hammurabi lunar-descent animal hexapawn bagels orbit splat civil-war}
for id in $ids; do
  tmp=$(mktemp -d)
  cp "target/release/$id" "$tmp/"
  (cd "$tmp" && HOME="$tmp" XDG_STATE_HOME="$tmp/state" \
    python3 "$castrec" "$here/asciiverse/$id.cast" . "w$secs;kq;w0.5" -- "./$id" --demo --mute --seed 7)
  rm -rf "$tmp"
done
