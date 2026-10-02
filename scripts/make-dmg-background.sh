#!/bin/sh
# Combine the two rendered PNGs into the multi-resolution TIFF the DMG uses.
#
#   1. Serve the repo (python3 -m http.server) and screenshot
#      assets/dmg-background.html at device scale 1 and 2 into
#      assets/dmg-background.png (2400x1500) and dmg-background@2x.png (4800x3000).
#   2. Run this script.
#
# A TIFF carries both, so the window is sharp on Retina and on a plain display.
set -e
cd "$(dirname "$0")/../assets"
tiffutil -cathidpicheck dmg-background.png "dmg-background@2x.png" -out dmg-background-raw.tiff
tiffutil -lzw dmg-background-raw.tiff -out dmg-background.tiff
rm dmg-background-raw.tiff dmg-background.png "dmg-background@2x.png"
ls -la dmg-background.tiff
