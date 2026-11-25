; Windows 설치 파일 만들기 설정 (NSIS)
; scripts/windows-installer.sh에서 STAGE, OUTFILE, ICON, VERSION 값을 넘겨받음

Unicode true
!include "MUI2.nsh"

!define NAME "DamaDama"
; 이전에 쓰던 이름들. 이전 설치본을 정리할 때 사용
!define OLD_NAME "YouTube MP3"
!define OLD_NAME_KO "유투브내려받기"
!define OLD_NAME_EN "YouTube Download"
!define PUBLISHER "아이티해결단(이혁)"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\yt-mp3"

Name "${NAME}"
OutFile "${OUTFILE}"
; 관리자 권한 없이 사용자 폴더에 설치.
; yt-dlp를 나중에 업데이트할 수 있도록 쓰기 가능한 위치를 사용
RequestExecutionLevel user
InstallDir "$LOCALAPPDATA\Programs\${NAME}"
SetCompressor /SOLID lzma

!define MUI_ICON "${ICON}"
!define MUI_UNICON "${ICON}"
!define MUI_FINISHPAGE_RUN "$INSTDIR\yt-mp3.exe"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "Korean"

; 설치 파일의 "속성 > 자세히"에 보이는 정보 (언어 설정 뒤에 와야 함)
VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=${LANG_KOREAN} "ProductName" "${NAME}"
VIAddVersionKey /LANG=${LANG_KOREAN} "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=${LANG_KOREAN} "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=${LANG_KOREAN} "FileDescription" "${NAME} 설치 파일"
VIAddVersionKey /LANG=${LANG_KOREAN} "CompanyName" "${PUBLISHER}"
VIAddVersionKey /LANG=${LANG_KOREAN} "LegalCopyright" "${PUBLISHER}"

; 이 설치 파일이 설치한 파일만 하나씩 삭제 (사용자가 넣은 파일은 남김)
!macro RemoveFiles DIR
    Delete "${DIR}\tools\yt-dlp.exe"
    Delete "${DIR}\tools\ffmpeg.exe"
    Delete "${DIR}\tools\ffprobe.exe"
    Delete "${DIR}\tools\deno.exe"
    Delete "${DIR}\licenses\ffmpeg-LICENSE.txt"
    Delete "${DIR}\licenses\ffmpeg-README.txt"
    Delete "${DIR}\yt-mp3.exe"
    Delete "${DIR}\icon.ico"
    Delete "${DIR}\uninstall.exe"
    RMDir "${DIR}\tools"
    RMDir "${DIR}\licenses"
    RMDir "${DIR}"
!macroend

Section "Install"
    SetOutPath "$INSTDIR\tools"
    File "${STAGE}/tools/yt-dlp.exe"
    File "${STAGE}/tools/ffmpeg.exe"
    File "${STAGE}/tools/ffprobe.exe"
    File "${STAGE}/tools/deno.exe"

    SetOutPath "$INSTDIR\licenses"
    File "${STAGE}/licenses/ffmpeg-LICENSE.txt"
    File "${STAGE}/licenses/ffmpeg-README.txt"

    ; 바로 가기의 작업 폴더가 되므로 마지막에 설정
    SetOutPath "$INSTDIR"
    File "${STAGE}/yt-mp3.exe"
    File "/oname=icon.ico" "${ICON}"
    WriteUninstaller "$INSTDIR\uninstall.exe"

    ; 이전 이름의 바로 가기 지우기
    Delete "$SMPROGRAMS\${OLD_NAME}.lnk"
    Delete "$DESKTOP\${OLD_NAME}.lnk"
    Delete "$SMPROGRAMS\${OLD_NAME_KO}.lnk"
    Delete "$DESKTOP\${OLD_NAME_KO}.lnk"
    Delete "$SMPROGRAMS\${OLD_NAME_EN}.lnk"
    Delete "$DESKTOP\${OLD_NAME_EN}.lnk"
    ; 이전 version이 설치했던 폴더 정리
    !insertmacro RemoveFiles "$LOCALAPPDATA\Programs\${OLD_NAME}"

    CreateShortcut "$SMPROGRAMS\${NAME}.lnk" "$INSTDIR\yt-mp3.exe" "" "$INSTDIR\icon.ico"
    CreateShortcut "$DESKTOP\${NAME}.lnk" "$INSTDIR\yt-mp3.exe" "" "$INSTDIR\icon.ico"

    ; Windows의 "설치된 앱" 목록에 등록
    WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "${NAME}"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "${PUBLISHER}"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\icon.ico"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
    WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
    WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
SectionEnd

Section "Uninstall"
    !insertmacro RemoveFiles "$INSTDIR"

    Delete "$SMPROGRAMS\${NAME}.lnk"
    Delete "$DESKTOP\${NAME}.lnk"
    DeleteRegKey HKCU "${UNINSTALL_KEY}"
SectionEnd
