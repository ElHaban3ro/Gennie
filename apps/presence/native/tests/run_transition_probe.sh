#!/bin/sh
# Sonda local del helper real, sin datos de usuario ni capturas.
set -eu
base=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
probe=$(mktemp -t soc-transition-probe)
trap 'rm -f "$probe"' EXIT HUP INT TERM
xcrun clang -fobjc-arc -O2 "$base/transition_probe.m" \
  -framework AppKit -framework QuartzCore -framework CoreGraphics -framework CoreImage -framework ImageIO -o "$probe"
"$probe"
