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


def video(x, y):
    """재생 삼각형과 필름 구멍이 있는 영상 화면."""
    screen = rounded_box(x, y, 512, 304, 232, 136, 34)
    holes = min(
        rounded_box(x, y, cx, cy, 18, 18, 6)
        for cx in (320, 704)
        for cy in (222, 304, 386)
    )
    play = polygon(x, y, [(460, 230), (590, 304), (460, 378)])
    # 화면에서 구멍과 삼각형 부분을 파내기
    return max(screen, -holes, -play)


def arrow(x, y):
    """영상에서 소리를 꺼낸다는 뜻의 아래 화살표."""
    shaft = rounded_box(x, y, 512, 515, 36, 45, 6)
    head = polygon(x, y, [(406, 545), (618, 545), (512, 635)])
    return min(shaft, head)


def label(x, y):
    """"MP3" 글자."""
    top, middle, bottom = 692, 758, 824
    letters = [
        # M
        [(258, bottom), (258, top), (338, 774), (418, top), (418, bottom)],
        # P
        [(488, bottom), (488, top), (564, top), (596, 706), (604, 729),
         (596, 752), (564, 766), (488, 766)],
        # 3
        [(670, top), (734, top), (764, 708), (764, 740), (734, middle), (696, middle)],
        [(734, middle), (764, 776), (764, 808), (734, bottom), (670, bottom)],
    ]
    return min(stroke(x, y, points) for points in letters)


def symbol(x, y):
    return min(video(x, y), arrow(x, y), label(x, y))


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
