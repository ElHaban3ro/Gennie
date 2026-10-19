#!/bin/sh
# Abre dos ventanas propias; --capture PREFIJO guarda solo la región de prueba.
set -eu
base=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
probe=$(mktemp -t soc-native-blur-probe)
trap 'rm -f "$probe"' EXIT HUP INT TERM
xcrun clang -fobjc-arc -O2 "$base/native_blur_probe.m" \
  -framework AppKit -framework QuartzCore -framework CoreGraphics -framework CoreImage -framework ImageIO -o "$probe"
"$probe" "$@"
