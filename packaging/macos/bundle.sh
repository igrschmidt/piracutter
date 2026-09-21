#!/usr/bin/env bash
# Builds PiraCutter.app from an Apple silicon binary already compiled.
set -euo pipefail
version="${1:?usage: bundle.sh <version>}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
app="$root/dist/PiraCutter.app"

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$root/target/aarch64-apple-darwin/release/piracutter" "$app/Contents/MacOS/piracutter"
chmod +x "$app/Contents/MacOS/piracutter"
sed "s/VERSION/$version/g" "$root/packaging/macos/Info.plist" > "$app/Contents/Info.plist"
if [ -f "$root/packaging/macos/piracutter.icns" ]; then
  cp "$root/packaging/macos/piracutter.icns" "$app/Contents/Resources/"
fi

# Ad-hoc signature: unsigned arm64 binaries will not launch at all, and this
# at least gets the app past that without a developer account.
codesign --force --deep --sign - "$app"
echo "built $app"
