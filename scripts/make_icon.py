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


def download_mark(x, y):
    """받침대로 내려오는 화살표. 내려받기를 뜻함."""
    shaft = rounded_box(x, y, 512, 220, 46, 62, 8)
    head = polygon(x, y, [(396, 272), (628, 272), (512, 376)])
    tray = stroke(x, y, [(318, 330), (318, 424), (706, 424), (706, 330)])
    return min(shaft, head, tray)


def letters_mp(x, y, top):
    """"MP" 글자. top은 글자의 위쪽 위치, 글자 높이는 120."""
    bottom = top + 120
    m = [(254, bottom), (254, top), (332, top + 76), (410, top), (410, bottom)]
    p = [(484, bottom), (484, top), (562, top), (596, top + 13), (604, top + 34),
         (596, top + 55), (562, top + 68), (484, top + 68)]
    return min(stroke(x, y, m), stroke(x, y, p))


def label_mp3(x, y, top=498):
    middle, bottom = top + 60, top + 120
    upper = [(674, top), (738, top), (768, top + 15), (768, top + 45),
             (738, middle), (700, middle)]
    lower = [(738, middle), (768, top + 75), (768, top + 105),
             (738, bottom), (674, bottom)]
    return min(letters_mp(x, y, top), stroke(x, y, upper), stroke(x, y, lower))


def label_mp4(x, y, top=708):
    slant = [(748, top), (662, top + 82), (780, top + 82)]
    upright = [(748, top), (748, top + 120)]
    return min(letters_mp(x, y, top), stroke(x, y, slant), stroke(x, y, upright))


def symbol(x, y):
    return min(download_mark(x, y), label_mp3(x, y), label_mp4(x, y))


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
    # 배경은 위의 밝은 빨강에서 아래의 진한 자주색으로
    color = mix((255, 94, 87), (142, 36, 122), (y - 100) / 824)
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
