#!/bin/sh
# Finder에서 실행할 수 있는 .app 만들기
# 사용법: ./scripts/bundle.sh
set -eu

# 어디에서 실행하든 프로젝트 폴더를 기준으로 동작
cd "$(dirname "$0")/.."

cargo build --release

app="target/release/bundle/유투브내려받기.app"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/yt-mp3 "$app/Contents/MacOS/yt-mp3"

# assets/icon.png를 크기별로 줄여서 macOS 아이콘 파일(.icns) 만들기
iconset="target/release/bundle/AppIcon.iconset"
rm -rf "$iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
    double=$((size * 2))
    sips -z "$size" "$size" assets/icon.png \
        --out "$iconset/icon_${size}x${size}.png" > /dev/null
    sips -z "$double" "$double" assets/icon.png \
        --out "$iconset/icon_${size}x${size}@2x.png" > /dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/AppIcon.icns"
rm -rf "$iconset"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>유투브내려받기</string>
    <key>CFBundleDisplayName</key>
    <string>유투브내려받기</string>
    <key>CFBundleIdentifier</key>
    <string>local.yt-mp3</string>
    <key>CFBundleExecutable</key>
    <string>yt-mp3</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>$version</string>
    <key>CFBundleVersion</key>
    <string>$version</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
PLIST

# 이 Mac에서 쓰기 위한 임시(ad-hoc) 서명
codesign --force --sign - "$app"

echo "완료: $PWD/$app"
