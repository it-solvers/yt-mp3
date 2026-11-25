#!/bin/sh
# Finder에서 실행할 수 있는 .app 만들기
# 사용법: ./scripts/bundle.sh
set -eu

# 어디에서 실행하든 프로젝트 폴더를 기준으로 동작
cd "$(dirname "$0")/.."

cargo build --release

app="target/release/bundle/JustGrab Player.app"
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
    <string>JustGrab Player</string>
    <key>CFBundleDisplayName</key>
    <string>JustGrab Player</string>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>LSHasLocalizedDisplayName</key>
    <true/>
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

# 오픈소스 license 문서 (설치 파일을 만들 때는 도구의 license까지 담아 다시 만듦)
python3 -I scripts/make_notices.py \
    "$app/Contents/Resources/licenses/THIRD-PARTY-NOTICES.txt" \
    --target "$(rustc -vV | sed -n 's/^host: //p')"

# 언어별 앱 이름. 언어 폴더가 있어야 폴더 선택 창 같은 macOS 화면도 해당 언어로 표시됨
# 사용법: localize 언어 이름
localize() {
    mkdir -p "$app/Contents/Resources/$1.lproj"
    cat > "$app/Contents/Resources/$1.lproj/InfoPlist.strings" <<STRINGS
"CFBundleName" = "$2";
"CFBundleDisplayName" = "$2";
STRINGS
}
localize en "JustGrab Player"
localize ko "걍담아 플레이어"

# 이 Mac에서 쓰기 위한 임시(ad-hoc) 서명
codesign --force --sign - "$app"

echo "완료: $PWD/$app"
