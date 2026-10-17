#!/bin/sh
# Собирает MediaRemoteAdapter.framework из исходников mediaremote-adapter
# (https://github.com/ungive/mediaremote-adapter, BSD-3).
#   mediaremote.sh <исходники> <каталог вывода> <arm64|x86_64|aarch64>
set -eu
src="$1"; out="$2"; arch="$3"
[ "$arch" = "aarch64" ] && arch=arm64
fw="$out/MediaRemoteAdapter.framework"
mkdir -p "$fw/Versions/A/Resources"
cd "$src"
clang -arch "$arch" -O2 -fobjc-arc -fvisibility=default -dynamiclib -mmacosx-version-min=13.0 \
  -Iinclude -Isrc $(find src -name '*.m' | sort) \
  -framework Foundation -framework AppKit -framework UniformTypeIdentifiers \
  -install_name @rpath/MediaRemoteAdapter.framework/Versions/A/MediaRemoteAdapter \
  -o "$fw/Versions/A/MediaRemoteAdapter"
ln -sfn A "$fw/Versions/Current"
ln -sfn Versions/Current/MediaRemoteAdapter "$fw/MediaRemoteAdapter"
ln -sfn Versions/Current/Resources "$fw/Resources"
cat > "$fw/Versions/A/Resources/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>app.kirk.MediaRemoteAdapter</string>
<key>CFBundleName</key><string>MediaRemoteAdapter</string>
<key>CFBundleExecutable</key><string>MediaRemoteAdapter</string>
<key>CFBundlePackageType</key><string>FMWK</string>
<key>CFBundleShortVersionString</key><string>1.0</string>
</dict></plist>
PLIST
cp bin/mediaremote-adapter.pl "$out/mediaremote-adapter.pl"
