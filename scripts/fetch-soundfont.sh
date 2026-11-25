#!/bin/sh
# MIDI 재생에 쓰는 음색 파일(GeneralUser GS)과 license를 target/soundfont에 내려받기
# 이미 받은 파일이 올바르면 다시 받지 않음
#
# 사용법: ./scripts/fetch-soundfont.sh
set -eu

cd "$(dirname "$0")/.."
folder="target/soundfont"
# 내용이 바뀌지 않도록 특정 commit의 파일을 사용
source="https://raw.githubusercontent.com/mrbumpy409/GeneralUser-GS/684543d5e5efaef08d02be50dcda8d552478fa60"

# 사용법: fetch 저장이름 주소경로 SHA-256
fetch() {
    file="$folder/$1"
    if [ -f "$file" ] && [ "$(shasum -a 256 "$file" | cut -d ' ' -f 1)" = "$3" ]; then
        return
    fi
    curl --fail --location --silent --show-error --output "$file" "$source/$2"
    if [ "$(shasum -a 256 "$file" | cut -d ' ' -f 1)" != "$3" ]; then
        echo "오류: SHA-256이 일치하지 않아요: $file" >&2
        rm -f "$file"
        exit 1
    fi
    echo "받음: $file"
}

mkdir -p "$folder"
fetch GeneralUser-GS.sf2 GeneralUser-GS.sf2 \
    9575028c7a1f589f5770fccc8cff2734566af40cd26ed836944e9a5152688cfe
fetch LICENSE.txt documentation/LICENSE.txt \
    7b32efefdf95ce38a043799f0659853ddc00fbaa14d8c50f0aca16b9b8b405be
