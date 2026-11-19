#!/bin/bash
# Package an already-built native executable without accessing application data.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
binary="${1:-$repo_root/target/release/facturecraft}"
output_dir="${2:-$repo_root/distribution}"
if [[ ! -x "$binary" ]]; then
  echo "Executable not found: $binary" >&2
  exit 1
fi
version="$("$binary" --version | awk '{print $NF}')"
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][A-Za-z0-9.-]+)?$ ]]; then
  echo "Unexpected application version: $version" >&2
  exit 1
fi
mkdir -p "$output_dir"
output_dir="$(cd "$output_dir" && pwd)"
staging="$(mktemp -d)"
trap 'rm -rf "$staging"' EXIT
app="$staging/FactureCraft.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$staging/FactureCraft"
cp "$binary" "$app/Contents/MacOS/facturecraft"
cp "$repo_root/docs/utilisation.md" "$repo_root/docs/guide-utilisateur.md" "$repo_root"/assets/fonts/*OFL.txt "$staging/FactureCraft/"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleDevelopmentRegion</key><string>fr</string>
<key>CFBundleExecutable</key><string>facturecraft</string>
<key>CFBundleIdentifier</key><string>fr.facturecraft.app</string>
<key>CFBundleIconFile</key><string>AppIcon</string>
<key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
<key>CFBundleName</key><string>FactureCraft</string>
<key>CFBundleDisplayName</key><string>FactureCraft</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$version</string>
<key>CFBundleVersion</key><string>$version</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
iconset="$staging/AppIcon.iconset"
mkdir "$iconset"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$repo_root/assets/facturecraft-logo.png" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" "$repo_root/assets/facturecraft-logo.png" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/AppIcon.icns"
plutil -lint "$app/Contents/Info.plist"
mv "$app" "$staging/FactureCraft/"
architecture="$(uname -m)"
archive="$output_dir/FactureCraft-macOS-${architecture}.zip"
if [[ -e "$archive" ]]; then
  echo "Refusing to overwrite existing archive: $archive" >&2
  exit 1
fi
ditto -c -k --sequesterRsrc --keepParent "$staging/FactureCraft" "$archive"
echo "Created $archive (unsigned and not notarized)"
