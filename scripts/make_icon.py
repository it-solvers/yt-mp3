#!/usr/bin/env python3
"""앱 아이콘(assets/icon.png) 그리기. Python 기본 기능만 사용.

사용법: python3 scripts/make_icon.py
"""
import math
import struct
import zlib
from pathlib import Path

SIZE = 1024


def rounded_box(x, y, cx, cy, half_w, half_h, radius):
    """둥근 사각형까지의 거리. 안쪽이면 음수."""
    dx = abs(x - cx) - half_w + radius
    dy = abs(y - cy) - half_h + radius
    outside = math.hypot(max(dx, 0), max(dy, 0))
    return outside + min(max(dx, dy), 0) - radius


def polygon(x, y, points):
    """볼록 다각형까지의 거리(근삿값). 꼭짓점은 시계 방향."""
    distance = -math.inf
    for (ax, ay), (bx, by) in zip(points, points[1:] + points[:1]):
        length = math.hypot(bx - ax, by - ay)
        distance = max(distance, ((x - ax) * (by - ay) - (y - ay) * (bx - ax)) / length)
    return distance


def arrow(x, y):
    """내려받기를 뜻하는 아래 화살표."""
    shaft = rounded_box(x, y, 512, 322, 62, 138, 24)
    head = polygon(x, y, [(340, 418), (684, 418), (512, 590)])
    return min(shaft, head)


def bowl(x, y):
    """받은 것을 담는 그릇. 둥근 사각형의 아래쪽만 사용."""
    body = rounded_box(x, y, 512, 630, 292, 196, 180)
    return max(body, 650 - y)


def symbol(x, y):
    return min(arrow(x, y), bowl(x, y))


def coverage(distance):
    """경계를 부드럽게 만들기 위한 픽셀 채움 비율."""
    return min(max(0.5 - distance, 0.0), 1.0)


def mix(a, b, t):
    return tuple(p + (q - p) * t for p, q in zip(a, b))


def pixel(x, y):
    # macOS 아이콘 모양: 가장자리에 여백이 있는 둥근 사각형
    alpha = coverage(rounded_box(x, y, 512, 512, 412, 412, 186))
    if alpha == 0:
        return (0, 0, 0, 0)
    # 배경은 위의 청록색에서 아래의 남색으로
    color = mix((32, 190, 170), (36, 72, 150), (y - 100) / 824)
    color = mix(color, (255, 255, 255), coverage(symbol(x, y)))
    return (*(round(c) for c in color), round(alpha * 255))


def chunk(kind, data):
    body = kind + data
    return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))


def main():
    rows = bytearray()
    for y in range(SIZE):
        rows.append(0)  # PNG의 줄마다 붙는 filter 번호
        for x in range(SIZE):
            rows.extend(pixel(x + 0.5, y + 0.5))

    png = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(bytes(rows), 9))
        + chunk(b"IEND", b"")
    )
    path = Path(__file__).resolve().parent.parent / "assets" / "icon.png"
    path.write_bytes(png)
    print(f"완료: {path}")


if __name__ == "__main__":
    main()
