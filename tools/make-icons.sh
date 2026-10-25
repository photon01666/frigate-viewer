#!/usr/bin/env bash
# Regenerate all launcher icons from the official Frigate bird logo.
set -euo pipefail
cd "$(dirname "$0")/.."

ROOT="$PWD"
RES="$ROOT/android/app/src/main/res"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

BACKGROUND="#F4F2EC"

# Render the source mark at high resolution and trim the transparent margins.
rsvg-convert -w 1600 -h 1600 "$ROOT/assets/frigate-logo.svg" -o "$TMP/bird.png"
magick "$TMP/bird.png" -trim +repage "$TMP/bird-trim.png"

# Base foreground: bird at 62% of the canvas height, centered on transparency.
make_foreground() {
  local size="$1" out="$2"
  local h=$((size * 62 / 100))
  local w
  w="$(magick identify -format '%w' "$TMP/bird-trim.png")"
  magick "$TMP/bird-trim.png" -filter Lanczos -resize "x${h}" "$TMP/bird-${size}.png"
  magick -size "${size}x${size}" xc:none \
    \( "$TMP/bird-${size}.png" \) -gravity center -composite \
    PNG32:"$out"
}

# Legacy launcher icons (rounded square / circle) for launchers that do not
# consume adaptive icons. These are also useful for previews and sharing.
make_legacy() {
  local size="$1" out="$2" shape="$3"
  local h=$((size * 60 / 100))
  local bg="$TMP/bg-${size}-${shape}.png"
  if [ "$shape" = "round" ]; then
    magick -size "${size}x${size}" xc:none -fill "$BACKGROUND" \
      -draw "circle $((size / 2)),$((size / 2)) $((size / 2)),1" "$bg"
  else
    magick -size "${size}x${size}" xc:none -fill "$BACKGROUND" \
      -draw "roundrectangle 0,0 $((size - 1)),$((size - 1)) $((size / 5)),$((size / 5))" "$bg"
  fi
  magick "$TMP/bird-trim.png" -filter Lanczos -resize "x${h}" "$TMP/legacy-${size}.png"
  magick "$bg" "$TMP/legacy-${size}.png" -gravity center -composite \
    -alpha on PNG32:"$out"
}

for spec in "mdpi 108 48" "hdpi 162 72" "xhdpi 216 96" "xxhdpi 324 144" "xxxhdpi 432 192"; do
  set -- $spec
  density="$1" fg="$2" legacy="$3"
  mkdir -p "$RES/mipmap-$density"
  make_foreground "$fg" "$RES/mipmap-$density/ic_launcher_foreground.png"
  make_legacy "$legacy" "$RES/mipmap-$density/ic_launcher.png" square
  make_legacy "$legacy" "$RES/mipmap-$density/ic_launcher_round.png" round
done

# Web UI / store icon: rounded-square version at 512 px.
mkdir -p "$ROOT/assets" "$ROOT/dist"
make_legacy 512 "$ROOT/dist/frigate-viewer-icon-512.png" square
make_legacy 512 "$ROOT/dist/frigate-viewer-icon-round-512.png" round
magick "$ROOT/dist/frigate-viewer-icon-512.png" -filter Lanczos -resize 256x256 \
  PNG32:"$ROOT/assets/frigate-mark.png"

echo "Icons generated."
