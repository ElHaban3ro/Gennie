#!/usr/bin/env bash
# Sonda visual nativa sin ventana, modelos ni datos de las apps.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
flutter_root="${FLUTTER_ROOT:-$HOME/.local/share/gennie-flutter}"
frameworks="$flutter_root/bin/cache/artifacts/engine/darwin-x64/FlutterMacOS.xcframework/macos-arm64_x86_64"
probe_dir="$(mktemp -d "${TMPDIR:-/tmp}/gennie-event-logos.XXXXXX")"
trap 'rm -rf "$probe_dir"' EXIT
output_dir="${1:-$probe_dir}"
mkdir -p "$output_dir"
swiftc -F "$frameworks" -framework FlutterMacOS -Xlinker -rpath -Xlinker "$frameworks" \
  "$root/frontend/macos/Runner/AssignmentLogoResolver.swift" "$root/scripts/probes/assignment_logos.swift" \
  -o "$probe_dir/probe"
"$probe_dir/probe" "$output_dir"
