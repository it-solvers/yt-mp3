#!/bin/sh
# Ubuntu(Intel/AMD 64bit)용 설치 파일(.deb) 만들기
# yt-dlp, ffmpeg, deno를 내려받아 package 안에 모두 담음
#
# 준비물: Docker (Mac에서는 brew install docker colima 후
#         colima start justgrab --activate=false --vm-type vz --vz-rosetta)
# 사용법: ./scripts/linux-deb.sh
#         다른 Docker를 쓰면 DOCKER_CONTEXT=이름 ./scripts/linux-deb.sh
set -eu

# 어디에서 실행하든 프로젝트 폴더를 기준으로 동작
cd "$(dirname "$0")/.."
root="$PWD"

export DOCKER_CONTEXT="${DOCKER_CONTEXT:-colima-justgrab}"
image=justgrab-linux-build
work="$root/target/linux"
downloads="$work/downloads"
stage="$work/stage"
program="$stage/opt/justgrab-player"

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
    curl --fail --location --silent --show-error --connect-timeout 20 \
        --output "$downloads/$1" "$2"
}

# container에서 명령 실행. library 내려받기와 build 결과는 다음에도 다시 사용
in_container() {
    docker run --rm --platform linux/amd64 \
        -v "$root:/project" -w /project \
        -v justgrab-cargo:/usr/local/cargo/registry \
        -v justgrab-target:/cargo-target -e CARGO_TARGET_DIR=/cargo-target \
        "$image" sh -ec "$1"
}

rust_version=$(rustc --version | cut -d ' ' -f 2)
docker build --quiet --platform linux/amd64 --build-arg "RUST_VERSION=$rust_version" \
    -t "$image" - < installer/linux.Dockerfile

./scripts/fetch-soundfont.sh

# 항상 새로 받아서 최신 버전을 담기
rm -rf "$downloads" "$stage"
mkdir -p "$downloads" "$program/tools" "$program/soundfont" "$program/licenses"

in_container 'cargo test --release --locked
    cargo build --release --locked
    cp /cargo-target/release/yt-mp3 target/linux/stage/opt/justgrab-player/'

ytdlp="https://github.com/yt-dlp/yt-dlp/releases/latest/download"
fetch yt-dlp_linux "$ytdlp/yt-dlp_linux"
fetch yt-dlp.sums "$ytdlp/SHA2-256SUMS"
verify "$downloads/yt-dlp_linux" \
    "$(awk '$2 == "yt-dlp_linux" { print $1 }' "$downloads/yt-dlp.sums")"

# ffmpeg는 yt-dlp가 안내하는 build 중 가장 새 release version
builds="https://github.com/BtbN/FFmpeg-Builds/releases/download/latest"
fetch ffmpeg.sums "$builds/checksums.sha256"
ffmpeg_file=$(awk '$2 ~ /^ffmpeg-n[0-9.]+-latest-linux64-gpl-[0-9.]+\.tar\.xz$/ { print $2 }' \
    "$downloads/ffmpeg.sums" | sort -V | tail -n 1)
if [ -z "$ffmpeg_file" ]; then
    echo "오류: ffmpeg 다운로드 주소를 찾지 못했어요" >&2
    exit 1
fi
fetch ffmpeg.tar.xz "$builds/$ffmpeg_file"
verify "$downloads/ffmpeg.tar.xz" \
    "$(awk -v name="$ffmpeg_file" '$2 == name { print $1 }' "$downloads/ffmpeg.sums")"

deno="https://github.com/denoland/deno/releases/latest/download/deno-x86_64-unknown-linux-gnu.zip"
fetch deno.zip "$deno"
fetch deno.sums "$deno.sha256sum"
verify "$downloads/deno.zip" "$(first_hash "$downloads/deno.sums")"

# 압축 파일에서 필요한 파일만 꺼내기
folder="${ffmpeg_file%.tar.xz}"
tar -xJf "$downloads/ffmpeg.tar.xz" -C "$downloads" \
    "$folder/bin/ffmpeg" "$folder/bin/ffprobe" "$folder/LICENSE.txt"
cp "$downloads/$folder/bin/ffmpeg" "$downloads/$folder/bin/ffprobe" "$program/tools/"
cp "$downloads/$folder/LICENSE.txt" "$program/licenses/ffmpeg-LICENSE.txt"
cp "$downloads/yt-dlp_linux" "$program/tools/yt-dlp"
unzip -q -j "$downloads/deno.zip" deno -d "$program/tools"
cp target/soundfont/GeneralUser-GS.sf2 target/soundfont/LICENSE.txt "$program/soundfont/"

# 담은 도구의 license 문서와 source 위치 모으기
latest_tag() {
    fetch "$1.release.json" "https://api.github.com/repos/$2/releases/latest"
    python3 -I -c 'import json, sys; print(json.load(open(sys.argv[1]))["tag_name"])' \
        "$downloads/$1.release.json"
}

# gnu.org에 연결할 수 없으면 다른 설치 파일을 만들 때 받아 둔 문서를 사용.
# 어느 쪽이든 내용이 같은지 SHA-256으로 확인
if ! fetch gpl-3.0.txt "https://www.gnu.org/licenses/gpl-3.0.txt"; then
    cp target/macos/downloads/gpl-3.0.txt "$downloads/gpl-3.0.txt" 2>/dev/null ||
        cp target/windows/downloads/gpl-3.0.txt "$downloads/gpl-3.0.txt"
fi
verify "$downloads/gpl-3.0.txt" \
    3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986

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

ffmpeg_version=$(in_container \
    'target/linux/stage/opt/justgrab-player/tools/ffmpeg -version | head -n 1' \
    | cut -d ' ' -f 3)
cat > "$downloads/ffmpeg-info.txt" <<INFO
ffmpeg and ffprobe are distributed under GPL-3.0-or-later.
Version: $ffmpeg_version
Binary: $builds/$ffmpeg_file
Build scripts: https://github.com/BtbN/FFmpeg-Builds
Source: https://github.com/FFmpeg/FFmpeg
INFO

deno_version=$(latest_tag deno denoland/deno)
fetch deno-LICENSE.md "https://raw.githubusercontent.com/denoland/deno/$deno_version/LICENSE.md"
cat > "$downloads/deno-info.txt" <<INFO
MIT License. Deno also includes third-party components such as V8.
Source: https://github.com/denoland/deno/tree/$deno_version
INFO

python3 -I scripts/make_notices.py "$program/licenses/THIRD-PARTY-NOTICES.txt" \
    --target x86_64-unknown-linux-gnu \
    --section "ffmpeg, ffprobe $ffmpeg_version (GPL-3.0-or-later)" "$downloads/ffmpeg-info.txt" \
        "$downloads/gpl-3.0.txt" \
    --section "yt-dlp $ytdlp_version (GPL-3.0-or-later)" "$downloads/yt-dlp-info.txt" \
        "$downloads/yt-dlp-LICENSE.txt" "$downloads/yt-dlp-THIRD-PARTY.txt" \
    --section "deno $deno_version (MIT)" "$downloads/deno-info.txt" "$downloads/deno-LICENSE.md" \
    --section "GeneralUser GS 2.0.3 SoundFont" target/soundfont/LICENSE.txt

# 실행 명령, 앱 목록의 바로 가기, 아이콘.
# libasound2t64를 먼저 적어야 새 Ubuntu에서 기능이 빠진 대체 library가 설치되지 않음
mkdir -p "$stage/usr/bin" "$stage/usr/share/applications" \
    "$stage/usr/share/icons/hicolor/512x512/apps" "$stage/DEBIAN"
ln -s /opt/justgrab-player/yt-mp3 "$stage/usr/bin/justgrab-player"
cp assets/icon.png "$stage/usr/share/icons/hicolor/512x512/apps/justgrab-player.png"
# 파일 이름과 StartupWMClass는 앱이 창에 알려주는 이름(yt-mp3)과 같아야 아이콘이 연결됨
cat > "$stage/usr/share/applications/yt-mp3.desktop" <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=JustGrab Player
Name[ko]=모아모아 플레이어
Comment=Save audio and video, and play your playlist
Comment[ko]=소리와 영상을 저장하고 재생목록으로 듣기
Exec=justgrab-player
Icon=justgrab-player
Terminal=false
Categories=AudioVideo;Audio;Player;
StartupWMClass=yt-mp3
DESKTOP

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
cat > "$stage/DEBIAN/control" <<CONTROL
Package: justgrab-player
Version: $version
Architecture: amd64
Maintainer: ITSolvers <noreply@localhost>
Section: sound
Priority: optional
Depends: libc6 (>= 2.35), libasound2t64 | libasound2, libfontconfig1, libgl1, libegl1,
 libvulkan1, libwayland-client0, libwayland-cursor0, libwayland-egl1, libx11-6,
 libx11-xcb1, libxcursor1, libxi6, libxrandr2, libxkbcommon0, libxkbcommon-x11-0,
 ca-certificates, xdg-utils, zenity
Recommends: mesa-vulkan-drivers, libgl1-mesa-dri, fonts-noto-cjk
Description: JustGrab Player
 Save audio and video from a link and play them with a playlist.
CONTROL

outfile="target/linux/JustGrab-Player-$version-amd64.deb"
# 공유 폴더에서는 파일 권한이 달라질 수 있으므로 container 안으로 복사해서 묶기
in_container "cp -a target/linux/stage /tmp/package
    find /tmp/package -type d -exec chmod 755 {} +
    find /tmp/package -type f -exec chmod 644 {} +
    chmod 755 /tmp/package/opt/justgrab-player/yt-mp3 /tmp/package/opt/justgrab-player/tools/*
    dpkg-deb --root-owner-group --build /tmp/package '$outfile'"

echo "완료: $root/$outfile"
