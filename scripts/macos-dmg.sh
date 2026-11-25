#!/bin/sh
# Apple Silicon Mac용 설치 파일(.dmg) 만들기
# yt-dlp, ffmpeg, deno를 내려받아 .app 안에 모두 담음
#
# 사용법: ./scripts/macos-dmg.sh
set -eu

# 어디에서 실행하든 프로젝트 폴더를 기준으로 동작
cd "$(dirname "$0")/.."
root="$PWD"

if [ "$(uname -m)" != "arm64" ]; then
    echo "오류: Apple Silicon Mac에서 실행해 주세요" >&2
    exit 1
fi

work="$root/target/macos"
downloads="$work/downloads"
stage="$work/stage"
app="$stage/YouTube MP3.app"
tools="$app/Contents/Resources/tools"
licenses="$app/Contents/Resources/licenses"

# 파일의 SHA-256이 배포처가 공개한 값과 같은지 확인
# 사용법: verify 파일 공개된값
verify() {
    actual=$(shasum -a 256 "$1" | cut -d ' ' -f 1)
    expected=$(printf '%s' "$2" | tr 'A-F' 'a-f')
    if [ -z "$expected" ] || [ "$actual" != "$expected" ]; then
        echo "오류: SHA-256이 일치하지 않아요: $1" >&2
        exit 1
    fi
    echo "확인: $(basename "$1")"
}

# 내용에서 SHA-256처럼 생긴 첫 번째 값 꺼내기
first_hash() {
    grep -oE '[0-9a-fA-F]{64}' "$1" | head -n 1
}

fetch() {
    curl --fail --location --silent --show-error --output "$downloads/$1" "$2"
}

# 먼저 도구가 없는 .app 만들기
./scripts/bundle.sh

# 항상 새로 받아서 최신 버전을 담기
rm -rf "$downloads" "$stage"
mkdir -p "$downloads" "$stage"
cp -R "target/release/bundle/YouTube MP3.app" "$app"
mkdir -p "$tools" "$licenses"

ytdlp="https://github.com/yt-dlp/yt-dlp/releases/latest/download"
fetch yt-dlp_macos "$ytdlp/yt-dlp_macos"
fetch yt-dlp.sums "$ytdlp/SHA2-256SUMS"
verify "$downloads/yt-dlp_macos" \
    "$(awk '$2 == "yt-dlp_macos" { print $1 }' "$downloads/yt-dlp.sums")"

# ffmpeg는 배포 페이지에서 정식 release의 주소를 찾아서 받기
site="https://ffmpeg.martin-riedl.de"
fetch ffmpeg.html "$site/"
for name in ffmpeg ffprobe; do
    path=$(grep -oE "/download/macos/arm64/[0-9]+_[0-9]+(\.[0-9]+)*/$name\.zip" \
        "$downloads/ffmpeg.html" | head -n 1)
    if [ -z "$path" ]; then
        echo "오류: $name 다운로드 주소를 찾지 못했어요" >&2
        exit 1
    fi
    fetch "$name.zip" "$site$path"
    fetch "$name.sums" "$site$path.sha256"
    verify "$downloads/$name.zip" "$(first_hash "$downloads/$name.sums")"
    unzip -q -j "$downloads/$name.zip" "$name" -d "$tools"
done

deno="https://github.com/denoland/deno/releases/latest/download/deno-aarch64-apple-darwin.zip"
fetch deno.zip "$deno"
fetch deno.sums "$deno.sha256sum"
verify "$downloads/deno.zip" "$(first_hash "$downloads/deno.sums")"
unzip -q -j "$downloads/deno.zip" deno -d "$tools"

# 앱이 yt-dlp라는 이름으로 찾으므로 이름 바꾸기
cp "$downloads/yt-dlp_macos" "$tools/yt-dlp"
chmod +x "$tools/yt-dlp" "$tools/ffmpeg" "$tools/ffprobe" "$tools/deno"

fetch gpl-3.0.txt "https://www.gnu.org/licenses/gpl-3.0.txt"
cp "$downloads/gpl-3.0.txt" "$licenses/ffmpeg-LICENSE.txt"
echo "ffmpeg, ffprobe 배포처: $site (source: https://ffmpeg.org)" \
    > "$licenses/ffmpeg-README.txt"

# 파일을 추가했으므로 다시 서명하고 확인
# (함께 담은 도구는 배포처의 서명을 그대로 유지)
codesign --force --sign - "$app"
codesign --verify --strict "$app"

# dmg를 열면 보이는 내용: 앱, Applications 바로 가기, 안내문
ln -s /Applications "$stage/Applications"
cat > "$stage/설치 안내.txt" <<'GUIDE'
설치 방법
1. "YouTube MP3"를 Applications 폴더로 끌어다 놓으세요.
2. Applications 폴더에서 "YouTube MP3"를 실행하세요.

"열 수 없다"는 경고가 나올 때
Apple의 공증을 받지 않은 앱이라 처음 한 번은 직접 허용해야 해요.
시스템 설정 > 개인정보 보호 및 보안에서 "그래도 열기"를 누르세요.

Apple Silicon(M1 이후) Mac용입니다.
GUIDE

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
outfile="$work/YouTube-MP3-$version-arm64.dmg"
hdiutil create -quiet -ov -format UDZO \
    -volname "YouTube MP3" -srcfolder "$stage" "$outfile"

echo "완료: $outfile"
