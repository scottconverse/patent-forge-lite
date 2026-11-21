#!/usr/bin/env bash
# Assemble an UNSIGNED Patent Forge Lite .app bundle on macOS.
#
# Nothing here signs or notarizes. Run only on a real macOS host:
#   cargo build -p pfl-desktop --features native --release --locked
#   packaging/macos/bundle.sh
#
# Output: packaging/macos/Patent Forge Lite.app
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
app_dir="$root/packaging/macos/Patent Forge Lite.app"
binary="$root/target/release/pfl-desktop"

if [[ ! -f "$binary" ]]; then
  echo "missing $binary — build the native target first" >&2
  exit 1
fi

rm -rf "$app_dir"
mkdir -p "$app_dir/Contents/MacOS" "$app_dir/Contents/Resources/docs"

cp "$root/packaging/macos/Info.plist" "$app_dir/Contents/Info.plist"
cp "$binary" "$app_dir/Contents/MacOS/pfl-desktop"
chmod +x "$app_dir/Contents/MacOS/pfl-desktop"
cp "$root/README.md" "$root/LICENSE" "$root/LICENSE-PROMPTS" \
   "$root/LEGAL_NOTICE.md" "$root/THIRD_PARTY_NOTICES.md" "$app_dir/Contents/Resources/"
cp -R "$root/docs/." "$app_dir/Contents/Resources/docs/"
cp -R "$root/prompts" "$app_dir/Contents/Resources/"

echo "assembled (UNSIGNED): $app_dir"
echo "signing and notarization require an explicit policy decision; none was made here."
