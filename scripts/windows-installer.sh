#!/bin/sh
# Mac에서 Windows(64bit) 설치 파일 만들기
# yt-dlp, ffmpeg, deno를 내려받아 설치 파일 하나에 모두 담음
#
# 사용법: ./scripts/windows-installer.sh
# 준비물: brew install mingw-w64 makensis
#         rustup target add x86_64-pc-windows-gnu
set -eu

# 어디에서 실행하든 프로젝트 폴더를 기준으로 동작
cd "$(dirname "$0")/.."
root="$PWD"

target="x86_64-pc-windows-gnu"
work="$root/target/windows"
downloads="$work/downloads"
stage="$work/stage"

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

cargo build --release --target "$target"

# 항상 새로 받아서 최신 버전을 담기
rm -rf "$downloads" "$stage"
mkdir -p "$downloads" "$stage/tools" "$stage/licenses"

ytdlp="https://github.com/yt-dlp/yt-dlp/releases/latest/download"
fetch yt-dlp.exe "$ytdlp/yt-dlp.exe"
fetch yt-dlp.sums "$ytdlp/SHA2-256SUMS"
verify "$downloads/yt-dlp.exe" \
    "$(awk '$2 == "yt-dlp.exe" { print $1 }' "$downloads/yt-dlp.sums")"

# ffmpeg는 gyan.dev 제작자의 GitHub 배포 저장소에서 받기.
# 파일 주소와 SHA-256은 GitHub의 release 정보에서 가져옴
fetch ffmpeg.json "https://api.github.com/repos/GyanD/codexffmpeg/releases/latest"
asset=$(python3 -I -c '
import json, sys

for asset in json.load(open(sys.argv[1]))["assets"]:
    if asset["name"].endswith("-essentials_build.zip"):
        print(asset["browser_download_url"], asset["digest"])
' "$downloads/ffmpeg.json")
if [ -z "$asset" ]; then
    echo "오류: ffmpeg 다운로드 주소를 찾지 못했어요" >&2
    exit 1
fi
fetch ffmpeg.zip "${asset%% *}"
verify "$downloads/ffmpeg.zip" "${asset##*sha256:}"

deno="https://github.com/denoland/deno/releases/latest/download/deno-x86_64-pc-windows-msvc.zip"
fetch deno.zip "$deno"
fetch deno.sums "$deno.sha256sum"
verify "$downloads/deno.zip" "$(first_hash "$downloads/deno.sums")"

# 압축 파일에서 필요한 파일만 꺼내기 (-j: 폴더 구조 없이)
cp "$downloads/yt-dlp.exe" "$stage/tools/"
unzip -q -j "$downloads/ffmpeg.zip" '*/bin/ffmpeg.exe' '*/bin/ffprobe.exe' -d "$stage/tools"
unzip -q -j "$downloads/deno.zip" 'deno.exe' -d "$stage/tools"
unzip -p "$downloads/ffmpeg.zip" '*/LICENSE' > "$stage/licenses/ffmpeg-LICENSE.txt"
unzip -p "$downloads/ffmpeg.zip" '*/README.txt' > "$stage/licenses/ffmpeg-README.txt"
cp "target/$target/release/yt-mp3.exe" "$stage/"

# 담은 도구의 license 문서와 source 위치 모으기.
# Windows 실행 파일은 Mac에서 실행할 수 없으므로 version은 release 정보에서 확인
latest_tag() {
    fetch "$1.release.json" "https://api.github.com/repos/$2/releases/latest"
    python3 -I -c 'import json, sys; print(json.load(open(sys.argv[1]))["tag_name"])' \
        "$downloads/$1.release.json"
}

fetch gpl-3.0.txt "https://www.gnu.org/licenses/gpl-3.0.txt"

ytdlp_version=$(latest_tag yt-dlp yt-dlp/yt-dlp)
ytdlp_raw="https://raw.githubusercontent.com/yt-dlp/yt-dlp/$ytdlp_version"
fetch yt-dlp-LICENSE.txt "$ytdlp_raw/LICENSE"
fetch yt-dlp-THIRD-PARTY.txt "$ytdlp_raw/THIRD_PARTY_LICENSES.txt"
cat > "$downloads/yt-dlp-info.txt" <<INFO
The bundled executable contains GPLv3+ components, so it is distributed
under GPL-3.0-or-later (full text in the ffmpeg section).
Binary: https://github.com/yt-dlp/yt-dlp/releases/tag/$ytdlp_version
Source: https://github.com/yt-dlp/yt-dlp/releases/download/$ytdlp_version/yt-dlp.tar.gz
INFO

ffmpeg_version=$(python3 -I -c 'import json, sys; print(json.load(open(sys.argv[1]))["tag_name"])' \
    "$downloads/ffmpeg.json")
cat > "$downloads/ffmpeg-info.txt" <<INFO
ffmpeg and ffprobe are distributed under GPL-3.0-or-later.
Binary: https://github.com/GyanD/codexffmpeg/releases/tag/$ffmpeg_version
Source: https://ffmpeg.org/releases/ffmpeg-$ffmpeg_version.tar.xz
The build information below lists the exact source commit and linked libraries.

Build information:
INFO

deno_version=$(latest_tag deno denoland/deno)
fetch deno-LICENSE.md "https://raw.githubusercontent.com/denoland/deno/$deno_version/LICENSE.md"
cat > "$downloads/deno-info.txt" <<INFO
MIT License. Deno also includes third-party components such as V8.
Source: https://github.com/denoland/deno/tree/$deno_version
INFO

python3 -I scripts/make_notices.py "$stage/licenses/THIRD-PARTY-NOTICES.txt" \
    --target "$target" \
    --section "ffmpeg, ffprobe $ffmpeg_version (GPL-3.0-or-later)" "$downloads/ffmpeg-info.txt" \
        "$stage/licenses/ffmpeg-README.txt" "$downloads/gpl-3.0.txt" \
    --section "yt-dlp $ytdlp_version (GPL-3.0-or-later)" "$downloads/yt-dlp-info.txt" \
        "$downloads/yt-dlp-LICENSE.txt" "$downloads/yt-dlp-THIRD-PARTY.txt" \
    --section "deno $deno_version (MIT)" "$downloads/deno-info.txt" "$downloads/deno-LICENSE.md"

python3 scripts/make_ico.py assets/icon.png "$work/icon.ico"

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
outfile="$work/JustGrab-Setup-$version.exe"

makensis -V2 -INPUTCHARSET UTF8 \
    "-DSTAGE=$stage" "-DICON=$work/icon.ico" \
    "-DOUTFILE=$outfile" "-DVERSION=$version" \
    installer/windows.nsi

echo "완료: $outfile"
