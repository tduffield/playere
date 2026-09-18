#!/bin/bash
# Generates the YouTube Player app icon: the master artwork (icon.png) and the
# two containers the .app bundle ships — Assets.car and AppIcon.icns.
#
# macOS 26 renders an app that only carries a classic .icns inside a white
# rounded plate, so the icon appears nested inside a second squircle. Declaring
# CFBundleIconName and shipping a compiled Assets.car opts into the modern path,
# where the artwork is drawn edge to edge.
#
# Requires ImageMagick (`brew install imagemagick`) and Xcode's actool for
# Assets.car; iconutil alone still produces AppIcon.icns.
#
#   ./icons/generate-icon.sh

set -euo pipefail

cd "$(dirname "$0")"

command -v magick >/dev/null || { echo "❌ ImageMagick (magick) not found. brew install imagemagick" >&2; exit 1; }

S=1024
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# --- artwork -----------------------------------------------------------------

# Squircle silhouette: an 824px art box centred in the 1024px canvas, the
# proportion Apple's macOS icon grid expects.
magick -size ${S}x${S} xc:none -fill white \
  -draw "roundrectangle 100,100 924,924 190,190" "$WORK/mask.png"

# Body: charcoal, lit from the top-left corner.
magick -size ${S}x${S} xc: \
  -sparse-color barycentric '0,0 #3f3f4b 1023,1023 #0a0a0d' \
  "$WORK/mask.png" -alpha off -compose CopyOpacity -composite "$WORK/body.png"

# A red spill under the screen, so the player reads as lit rather than printed.
magick -size ${S}x${S} xc:none -fill 'rgba(239,68,68,0.75)' \
  -draw "translate 512,672 scale 1,0.14 circle 0,0 210,0" \
  -blur 0x28 "$WORK/glow.png"

# Highlight along the top edge only, giving the body a rounded, physical edge.
magick -size ${S}x${S} xc:none -fill none \
  -stroke 'rgba(255,255,255,0.30)' -strokewidth 4 \
  -draw "roundrectangle 106,106 918,918 186,186" \
  -blur 0x1 -gravity north -crop ${S}x420+0+0 +repage \
  -background none -extent ${S}x${S} "$WORK/rim.png"

# The screen and its play glyph. The glyph is oversized against the screen so it
# still carries the icon at 16px, where the screen itself washes into the body.
magick -size ${S}x${S} xc:none \
  -fill '#0a0a0c' -stroke 'rgba(255,255,255,0.16)' -strokewidth 5 \
  -draw "roundrectangle 232,326 792,650 46,46" \
  -fill '#ff4d4d' -stroke '#ff4d4d' -strokewidth 34 \
  -draw "stroke-linejoin round polygon 438,384 438,592 639,488" "$WORK/glyph.png"

magick "$WORK/body.png" \
  "$WORK/glow.png" -compose Screen -composite \
  "$WORK/rim.png" -compose Over -composite \
  "$WORK/glyph.png" -compose Over -composite \
  "$WORK/mask.png" -alpha off -compose CopyOpacity -composite \
  icon.png
echo "🎨 icon.png (${S}x${S})"

# --- containers --------------------------------------------------------------

SET="$WORK/Assets.xcassets/AppIcon.appiconset"
mkdir -p "$SET"
printf '{\n  "info" : { "author" : "xcode", "version" : 1 }\n}\n' \
  > "$WORK/Assets.xcassets/Contents.json"

IMAGES=""
for spec in "16 16 1x" "32 16 2x" "32 32 1x" "64 32 2x" "128 128 1x" \
            "256 128 2x" "256 256 1x" "512 256 2x" "512 512 1x" "1024 512 2x"; do
  set -- $spec
  magick icon.png -resize "$1x$1" "$SET/icon_$2x$2@$3.png"
  IMAGES="$IMAGES{\"filename\":\"icon_$2x$2@$3.png\",\"idiom\":\"mac\",\"scale\":\"$3\",\"size\":\"$2x$2\"},"
done
printf '{"images":[%s],"info":{"author":"xcode","version":1}}\n' "${IMAGES%,}" \
  > "$SET/Contents.json"

if xcrun -f actool >/dev/null 2>&1; then
  mkdir -p "$WORK/out"
  xcrun actool "$WORK/Assets.xcassets" \
    --compile "$WORK/out" \
    --app-icon AppIcon \
    --minimum-deployment-target 11.0 \
    --platform macosx --target-device mac \
    --output-partial-info-plist "$WORK/partial.plist" >/dev/null
  cp "$WORK/out/Assets.car" "$WORK/out/AppIcon.icns" .
  echo "🍎 Assets.car + AppIcon.icns"
else
  echo "⚠️  actool not found (needs Xcode) — Assets.car not regenerated." >&2
  ICONSET="$WORK/AppIcon.iconset"
  mkdir -p "$ICONSET"
  cp "$SET"/*.png "$ICONSET"/
  for f in "$ICONSET"/*.png; do
    mv "$f" "$(echo "$f" | sed 's/@1x//')"
  done
  iconutil -c icns "$ICONSET" -o AppIcon.icns
  echo "🍎 AppIcon.icns"
fi
