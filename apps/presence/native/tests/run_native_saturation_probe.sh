#!/bin/sh
# Capturas limitadas a las ventanas propias; sin DB, audio ni red.
set -eu
base=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
work=$(mktemp -d -t gennie-saturation-probe)
trap 'rm -rf "$work"' EXIT HUP INT TERM
xcrun clang -fobjc-arc -O2 "$base/native_saturation_probe.m" \
  -framework AppKit -framework QuartzCore -framework CoreGraphics -o "$work/probe"
if [ "$#" -eq 2 ] && [ "$1" = '--capture' ]; then
  "$work/probe" --capture "$2"
else
  "$work/probe" --capture "$work/saturation"
fi
