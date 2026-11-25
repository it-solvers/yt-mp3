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


def stroke(x, y, points, radius=22):
    """점들을 차례로 이은 굵은 선까지의 거리."""
    distance = math.inf
    for (ax, ay), (bx, by) in zip(points, points[1:]):
        dx, dy = bx - ax, by - ay
        t = ((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy)
        t = min(max(t, 0.0), 1.0)
        distance = min(distance, math.hypot(x - ax - dx * t, y - ay - dy * t))
    return distance - radius


def screen(x, y):
    """받침대가 있는 영상 화면."""
    body = rounded_box(x, y, 512, 352, 270, 190, 56)
    stand = rounded_box(x, y, 512, 566, 110, 16, 16)
    return min(body, stand)


def play(x, y):
    """화면 가운데의 재생 삼각형."""
    return polygon(x, y, [(442, 262), (620, 352), (442, 442)])


def download_mark(x, y):
    """받침대로 내려오는 화살표. 내려받기를 뜻함."""
    shaft = rounded_box(x, y, 512, 652, 40, 46, 8)
    head = polygon(x, y, [(408, 686), (616, 686), (512, 782)])
    tray = stroke(x, y, [(330, 766), (330, 842), (694, 842), (694, 766)])
    return min(shaft, head, tray)


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
    # 배경은 위의 흰색에서 아래의 옅은 회색으로
    color = mix((255, 255, 255), (228, 228, 232), (y - 100) / 824)
    # 아래에 있는 것부터 차례로 겹쳐 그리기
    color = mix(color, (224, 36, 36), coverage(screen(x, y)))
    color = mix(color, (255, 255, 255), coverage(play(x, y)))
    color = mix(color, (44, 44, 50), coverage(download_mark(x, y)))
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
