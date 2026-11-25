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
import textwrap
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


def summary(found):
    """license 종류별로 library 이름만 모은 요약."""
    groups = {}
    for package in found:
        license_name = package["license"] or "see license text"
        groups.setdefault(license_name, []).append(
            f'{package["name"]} {package["version"]}'
        )
    # 많이 쓰는 license부터
    ordered = sorted(groups.items(), key=lambda item: (-len(item[1]), item[0]))
    return "\n\n".join(
        f"{license_name} ({len(names)})\n"
        + textwrap.fill(
            ", ".join(names), width=78, initial_indent="  ",
            subsequent_indent="  ", break_long_words=False, break_on_hyphens=False,
        )
        for license_name, names in ordered
    )


def rust_sections(target):
    """Rust library 부분을 (제목, 내용) 목록으로 만들기."""
    found = packages(target)
    listing = []
    # 같은 내용의 license 전문은 한 번만 싣고, 사용하는 library 이름을 모아 두기
    texts = {}
    for package in found:
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

    included = "".join(
        f"\n--- {', '.join(users)} ---\n\n{text}\n" for text, users in texts.items()
    )
    standard = "".join(
        f"\n--- {file.stem} ---\n\n{read(file)}\n"
        for file in sorted(STANDARD.glob("*.txt"))
    )
    return [
        ("Rust libraries by license / license별 Rust library", "\n" + summary(found)),
        ("Rust library details / Rust library 상세 정보", "\n" + "\n\n".join(listing)),
        ("License texts included with the Rust libraries / library에 포함된 license 전문",
         included),
        ("Standard license texts / 표준 license 전문\n"
         "(for libraries that do not ship their own license file)", standard),
    ]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("output")
    parser.add_argument("--target")
    parser.add_argument("--section", nargs="+", action="append", default=[])
    args = parser.parse_args()

    document = (
        "JustGrab Player (걍담아 플레이어) - THIRD-PARTY NOTICES / 오픈소스 라이선스\n\n"
        "This program includes or uses the open source software listed below.\n"
        "Each component remains under its own license.\n\n"
        "이 프로그램은 아래의 오픈소스 software를 포함하거나 사용합니다.\n"
        "각 구성요소에는 해당 license가 적용됩니다."
    )
    sections = [
        (heading, "".join(f"\n{read(file)}\n" for file in files))
        for heading, *files in args.section
    ]
    sections += rust_sections(args.target or host_target())

    # 목차: 본문 제목과 같은 번호를 붙여서 검색으로 찾아갈 수 있게 하기
    document += title("CONTENTS / 목차")
    for number, (heading, _) in enumerate(sections, 1):
        document += f"\n[{number}] {heading.splitlines()[0]}"
    document += (
        "\n\nTo jump to a section, search for its number, e.g. [1]."
        "\n찾기 기능에서 [1]처럼 번호를 검색하면 해당 부분으로 이동합니다."
    )
    for number, (heading, body) in enumerate(sections, 1):
        document += title(f"[{number}] {heading}") + body

    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    # BOM을 붙여서 메모장과 TextEdit이 한글을 UTF-8로 읽게 하기
    output.write_text(document + "\n", encoding="utf-8-sig")
    print(f"완료: {output}")


if __name__ == "__main__":
    main()
