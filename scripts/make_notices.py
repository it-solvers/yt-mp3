#!/usr/bin/env python3
"""설치 파일에 담을 오픈소스 license 문서(THIRD-PARTY-NOTICES.txt) 만들기.

사용법: python3 -I scripts/make_notices.py 출력파일 --target TARGET
        [--section "제목" 파일 [파일 ...]] ...
--target을 생략하면 이 컴퓨터용으로 만듦.

--section은 함께 담는 외부 도구처럼 Rust library가 아닌 항목을 추가할 때 사용.
"""
import argparse
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
STANDARD = ROOT / "licenses" / "standard"
LICENSE_PREFIXES = ("LICENSE", "LICENCE", "COPYING", "COPYRIGHT", "NOTICE", "UNLICENSE")
LINE = "=" * 78


def read(path):
    return Path(path).read_text(encoding="utf-8", errors="replace").strip()


def title(text):
    return f"\n\n{LINE}\n{text}\n{LINE}\n"


def host_target():
    """이 컴퓨터의 target 이름."""
    output = subprocess.run(
        ["rustc", "-vV"], check=True, capture_output=True, text=True
    ).stdout
    return next(
        line.removeprefix("host: ")
        for line in output.splitlines()
        if line.startswith("host: ")
    )


def packages(target):
    """target용 프로그램에 들어가는 Rust library 목록 (test 전용은 제외)."""
    output = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked",
         "--filter-platform", target],
        cwd=ROOT, check=True, capture_output=True,
    ).stdout
    data = json.loads(output)
    by_id = {p["id"]: p for p in data["packages"]}
    nodes = {n["id"]: n for n in data["resolve"]["nodes"]}
    root = data["resolve"]["root"]

    found, waiting = set(), [root]
    while waiting:
        current = waiting.pop()
        if current in found:
            continue
        found.add(current)
        waiting += [
            dep["pkg"] for dep in nodes[current]["deps"]
            if any(kind["kind"] != "dev" for kind in dep["dep_kinds"])
        ]
    found.discard(root)
    return sorted((by_id[i] for i in found), key=lambda p: (p["name"], p["version"]))


def rust_section(target):
    listing = []
    # 같은 내용의 license 전문은 한 번만 싣고, 사용하는 library 이름을 모아 두기
    texts = {}
    for package in packages(target):
        name = f'{package["name"]} {package["version"]}'
        folder = Path(package["manifest_path"]).parent
        authors = ", ".join(package["authors"]) or "(see source)"
        listing.append(
            f'{name}\n  License: {package["license"]}\n  Authors: {authors}\n'
            f'  Source: https://crates.io/crates/{package["name"]}/{package["version"]}'
        )
        # 최상위 폴더의 license 파일과, 글꼴 폴더의 license 문서
        files = [
            f for f in folder.iterdir()
            if f.is_file() and f.name.upper().startswith(LICENSE_PREFIXES)
        ]
        if package["name"] == "epaint_default_fonts":
            files += (folder / "fonts").glob("*.txt")
        for file in sorted(files):
            texts.setdefault(read(file), []).append(f"{name} ({file.name})")

    result = title("Rust libraries") + "\n" + "\n\n".join(listing)
    result += title("License texts included with the Rust libraries")
    for text, users in texts.items():
        result += f"\n--- {', '.join(users)} ---\n\n{text}\n"
    result += title(
        "Standard license texts\n"
        "(for libraries that do not ship their own license file)"
    )
    for file in sorted(STANDARD.glob("*.txt")):
        result += f"\n--- {file.stem} ---\n\n{read(file)}\n"
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("output")
    parser.add_argument("--target")
    parser.add_argument("--section", nargs="+", action="append", default=[])
    args = parser.parse_args()

    document = (
        "JustGrab (걍담아) - THIRD-PARTY NOTICES / 오픈소스 라이선스\n\n"
        "This program includes or uses the open source software listed below.\n"
        "Each component remains under its own license.\n\n"
        "이 프로그램은 아래의 오픈소스 software를 포함하거나 사용합니다.\n"
        "각 구성요소에는 해당 license가 적용됩니다."
    )
    for heading, *files in args.section:
        document += title(heading)
        for file in files:
            document += f"\n{read(file)}\n"
    document += rust_section(args.target or host_target())

    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    # BOM을 붙여서 메모장과 TextEdit이 한글을 UTF-8로 읽게 하기
    output.write_text(document + "\n", encoding="utf-8-sig")
    print(f"완료: {output}")


if __name__ == "__main__":
    main()
