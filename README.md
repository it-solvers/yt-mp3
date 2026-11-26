# 모아모아 플레이어 (JustGrab Player)

링크의 소리나 영상을 저장하고, 재생목록으로 듣는 데스크톱 앱입니다.
Mac, Windows, Ubuntu에서 사용합니다. 화면은 시스템 언어에 따라 한글 또는 영어로 보이며, 앱에서 바꿀 수 있습니다.

## 주요 기능

- **저장**: 영상 주소를 붙여넣어 소리(MP3) 또는 영상(MP4)으로 저장
- **재생**: 재생, 일시정지, 이전·다음 곡, 반복(전체, 한 곡), 섞기, 위치 이동, 음량 조절
- **재생목록**: 파일과 폴더를 끌어다 놓아 추가. 파일은 원래 위치에 두고 경로만 기억
- **편집**: 여러 곡 선택, 끌어서 순서 바꾸기, 목록에서 빼기, 파일 삭제(휴지통)
- **화면 색상**: 미리 만든 색 조합 선택 또는 직접 고르기

### 지원 파일

| 종류 | 확장자 | 재생 방법 |
|---|---|---|
| 소리 | mp3, wav, flac, ogg(Vorbis), m4a(AAC, ALAC), aac, mid, midi | 앱에서 재생 |
| 영상 | mp4, mov, m4v, mkv, webm, avi, wmv | 컴퓨터의 기본 앱으로 열기 |

Opus처럼 재생할 수 없는 소리 파일은 재생목록에 추가하지 않습니다.

## 설치

| 운영체제 | 설치 파일 | 설치 방법 |
|---|---|---|
| Mac (Apple Silicon) | `JustGrab-Player-<version>-arm64.dmg` | 앱을 Applications 폴더로 끌어다 놓기 |
| Windows (64bit) | `JustGrab-Player-Setup-<version>.exe` | 설치 파일 실행 |
| Ubuntu 22.04 이상 (Intel/AMD 64bit) | `JustGrab-Player-<version>-amd64.deb` | `sudo apt install ./JustGrab-Player-<version>-amd64.deb` |

필요한 도구(yt-dlp, ffmpeg, deno)와 MIDI 음색 파일은 설치 파일에 들어 있습니다.

### 처음 실행할 때 경고가 나오면

코드 서명과 Apple 공증을 받지 않은 앱이라 운영체제가 경고를 보여줄 수 있습니다.

- **Mac**: 한 번 실행해 본 뒤 `시스템 설정 > 개인정보 보호 및 보안`에서 **그래도 열기**
- **Windows**: **추가 정보**를 누른 뒤 **실행**

## 사용 방법

### 저장하기

1. 영상 주소를 입력란에 붙여넣습니다.
2. **소리 (MP3)** 또는 **영상 (MP4)** 을 고릅니다.
3. 저장 폴더를 확인합니다. 처음에는 사용자 폴더의 `Music`입니다.
4. **저장하기**를 누릅니다. 저작권 안내를 읽고 동의하면 저장이 시작됩니다.

저장한 파일은 재생목록 맨 아래에 `NEW` 표시와 함께 추가됩니다. 이미 있는 파일은 다시 받지 않습니다.
로그인이 필요한 영상, 비공개 영상, DRM으로 보호된 콘텐츠는 저장할 수 없습니다.

### 듣기

- 목록의 곡을 누르면 재생하고, 다시 누르면 일시정지합니다.
- 음량은 재생 영역 오른쪽 위의 **스피커**로 조절합니다. 마우스를 올리면 음량 막대가 열리고, 누르면 소리를 끄거나 켭니다.
- 재생할 수 없는 파일은 건너뛰고 다음 곡을 재생합니다.

### 재생목록 편집

연필 버튼을 누르면 편집 상태가 됩니다. 곡을 눌러 선택한 뒤 끌어서 옮기거나, **목록에서 빼기**(파일은 남김) 또는 **파일 삭제**(휴지통으로 이동)를 고릅니다.

지워졌거나 옮겨진 파일은 다음에 앱을 켤 때 목록에서 빠집니다. 연결되지 않은 외장 디스크나 network 드라이브의 파일은 남겨 둡니다.

## 저작권 안내

저작권자의 허락 없이 콘텐츠를 복제·배포·전송하는 행위는 저작권법에 위반되어 법적 책임을 질 수 있습니다.
권리를 가졌거나 이용을 허락받은 콘텐츠, 또는 법에서 허용하는 범위에서만 내려받고 각 사이트의 이용약관을 지켜 주세요.
**내려받은 콘텐츠의 이용에 대한 모든 책임은 사용자에게 있습니다.**

## 개인정보 안내

이 프로그램은 어떠한 개인정보도 수집하거나 외부로 전송하지 않습니다.
재생목록과 설정은 사용자의 컴퓨터에만 저장됩니다. 다만 저장할 때에는 해당 영상 사이트에 접속합니다.

## 문제 해결

| 증상 | 해결 방법 |
|---|---|
| 저장이 계속 실패함 | 주소와 인터넷 연결, 저장 폴더를 확인합니다. 영상 사이트가 바뀌었을 수 있으므로 새 설치 파일을 설치합니다. |
| 파일을 추가할 수 없음 | 지원하는 확장자인지, 다른 앱에서 재생되는 파일인지 확인합니다. |
| Ubuntu에서 글자가 흐림 | 화면 배율이 125%나 150%이면 흐리게 보일 수 있습니다. |

앱에는 자동 업데이트 기능이 없습니다. 새 설치 파일을 덮어 설치하면 재생목록과 설정은 유지됩니다.

## 개발

Rust(edition 2024)와 [eframe/egui](https://github.com/emilk/egui), [rodio](https://github.com/RustAudio/rodio)로 만들었습니다.

### 실행과 검사

개발 중에는 yt-dlp, ffmpeg, deno가 컴퓨터에 설치되어 있어야 합니다. (Mac: `brew install yt-dlp ffmpeg deno`)

```sh
./scripts/fetch-soundfont.sh   # MIDI 음색 파일 받기 (처음 한 번)
cargo run

cargo fmt --check
cargo clippy --all-targets
cargo test
```

### 설치 파일 만들기

모두 **Apple Silicon Mac**에서 실행하며, 공통으로 Rust와 Python 3가 필요합니다. 먼저 `Cargo.toml`의 version을 올립니다.
yt-dlp, ffmpeg, deno는 만들 때마다 새 version을 내려받아 담습니다.

| 대상 | 명령 | 준비물 | 결과 위치 |
|---|---|---|---|
| Mac | `./scripts/macos-dmg.sh` | 추가 준비물 없음 | `target/macos/` |
| Windows | `./scripts/windows-installer.sh` | `brew install mingw-w64 makensis`, `rustup target add x86_64-pc-windows-gnu` | `target/windows/` |
| Ubuntu | `./scripts/linux-deb.sh` | `brew install docker colima` | `target/linux/` |

Ubuntu는 Docker 안에서 빌드합니다.

```sh
# 처음 한 번: 빌드용 가상 컴퓨터 만들기
colima start justgrab --activate=false --vm-type vz --vz-rosetta --cpu 8 --memory 8 --disk 40

colima start justgrab --activate=false
./scripts/linux-deb.sh
colima stop justgrab
```

### 폴더 구조

| 경로 | 내용 |
|---|---|
| `src/main.rs` | 화면, 재생목록, 재생, 다운로드 |
| `src/glow.rs` | 재생 영역의 바탕과 slider |
| `src/icons.rs` | 직접 그리는 아이콘과 버튼 |
| `src/theme.rs` | 화면 색상 |
| `src/lang.rs` | 한글과 영어 문구 선택 |
| `src/midi.rs` | MIDI 재생 |
| `assets/` | 앱 아이콘 |
| `installer/` | Windows 설치 script, Ubuntu 빌드 환경 |
| `scripts/` | 설치 파일과 라이선스 문서를 만드는 script |
| `licenses/` | 표준 라이선스 전문 |

## 오픈소스 라이선스

설치 파일에는 yt-dlp, ffmpeg(GPL), deno(MIT), GeneralUser GS SoundFont와 여러 Rust library가 포함됩니다.
각 라이선스 전문은 앱의 **ⓘ 프로그램 정보 > 오픈소스 라이선스**에서 볼 수 있습니다.

---

© 2026 아이티해결단 (ITSolvers). All rights reserved.
