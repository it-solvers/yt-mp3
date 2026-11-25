#!/bin/sh
# Finder에서 실행할 수 있는 .app 만들기
# 사용법: ./scripts/bundle.sh
set -eu

# 어디에서 실행하든 프로젝트 폴더를 기준으로 동작
cd "$(dirname "$0")/.."

cargo build --release

app="target/release/bundle/YouTube MP3.app"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)

rm -rf "$app"
mkdir -p "$app/Contents/MacOS"
cp target/release/yt-mp3 "$app/Contents/MacOS/yt-mp3"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>YouTube MP3</string>
    <key>CFBundleIdentifier</key>
    <string>local.yt-mp3</string>
    <key>CFBundleExecutable</key>
    <string>yt-mp3</string>
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
