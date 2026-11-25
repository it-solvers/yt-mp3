#!/usr/bin/env python3
"""PNG 아이콘으로 Windows 아이콘(.ico) 만들기. macOS의 sips 사용.

사용법: python3 scripts/make_ico.py 입력.png 출력.ico
"""
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

SIZES = (16, 32, 48, 64, 128, 256)


def main():
    source, output = sys.argv[1], Path(sys.argv[2])

    images = []
    with tempfile.TemporaryDirectory() as folder:
        for size in SIZES:
            resized = Path(folder) / f"{size}.png"
            subprocess.run(
                ["sips", "-z", str(size), str(size), source, "--out", str(resized)],
                check=True,
                stdout=subprocess.DEVNULL,
            )
            images.append(resized.read_bytes())

    # .ico 구조: 머리말, 크기별 목록, 이미지(PNG) 데이터
    header = struct.pack("<HHH", 0, 1, len(SIZES))
    offset = len(header) + 16 * len(SIZES)
    entries = b""
    for size, data in zip(SIZES, images):
        side = size % 256  # 256은 0으로 적는 규칙
        entries += struct.pack("<BBBBHHII", side, side, 0, 0, 1, 32, len(data), offset)
        offset += len(data)

    output.write_bytes(header + entries + b"".join(images))


if __name__ == "__main__":
    main()
