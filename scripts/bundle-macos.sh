#!/bin/sh
# Builds Arch Hiero in release mode and assembles target/bundle/ArchHiero.app, with the app
# and the MCP server side by side in Contents/MacOS. Pass --sign "<identity>" to code sign it.
set -eu

cd "$(dirname "$0")/.."
VERSION=$(cargo metadata --no-deps --format-version 1 | sed -n 's/.*"name":"hiero-app","version":"\([^"]*\)".*/\1/p')
cargo build --release -p hiero-app -p hiero-mcp

APP=target/bundle/ArchHiero.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/arch-hiero target/release/arch-hiero-mcp "$APP/Contents/MacOS/"

# The app icon, in each size macOS asks for, from the 1024 px PNG.
ICONSET=target/bundle/AppIcon.iconset
rm -rf "$ICONSET"
mkdir -p "$ICONSET"
for size in 16 32 128 256 512; do
  sips -z $size $size crates/hiero-app/assets/app-icon.png --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z $double $double crates/hiero-app/assets/app-icon.png --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"
rm -rf "$ICONSET"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Arch Hiero</string>
  <key>CFBundleDisplayName</key><string>Arch Hiero</string>
  <key>CFBundleIdentifier</key><string>com.archhiero.app</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleExecutable</key><string>arch-hiero</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

if [ "${1:-}" = "--sign" ] && [ -n "${2:-}" ]; then
  codesign --force --options runtime --sign "$2" "$APP/Contents/MacOS/arch-hiero-mcp"
  codesign --force --options runtime --sign "$2" "$APP"
fi

echo "Built $APP"
echo "MCP server: $(pwd)/$APP/Contents/MacOS/arch-hiero-mcp"
