; Windows 설치 파일 만들기 설정 (NSIS)
; scripts/windows-installer.sh에서 STAGE, OUTFILE, ICON, VERSION 값을 넘겨받음

Unicode true
!include "MUI2.nsh"

!define NAME "JustGrab Player"
!define NAME_KO "걍담아 플레이어"
; 설치 폴더 이름. 이름이 바뀌어도 같은 폴더에 덮어 설치
!define FOLDER "JustGrab"
; 이전에 쓰던 이름들. 이전 설치본을 정리할 때 사용
!define OLD_NAME "YouTube MP3"
!define OLD_NAME_KO "유투브내려받기"
!define OLD_NAME_EN "YouTube Download"
!define OLD_NAME_DAMA "DamaDama"
!define OLD_NAME_SHORT "JustGrab"
!define OLD_NAME_SHORT_KO "걍담아"
!define PUBLISHER "ITSolvers(Hyeok Lee)"
!define PUBLISHER_KO "아이티해결단(이혁)"
; 저작권자 표시
!define COPYRIGHT "ITSolvers"
!define COPYRIGHT_KO "아이티해결단"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\yt-mp3"

OutFile "${OUTFILE}"
; 관리자 권한 없이 사용자 폴더에 설치.
; yt-dlp를 나중에 업데이트할 수 있도록 쓰기 가능한 위치를 사용
RequestExecutionLevel user
InstallDir "$LOCALAPPDATA\Programs\${FOLDER}"
SetCompressor /SOLID lzma

!define MUI_ICON "${ICON}"
!define MUI_UNICON "${ICON}"
!define MUI_FINISHPAGE_RUN "$INSTDIR\yt-mp3.exe"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
; Windows의 표시 언어에 맞는 것을 자동 선택.
; 맞는 언어가 없으면 첫 번째인 영어 사용
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "Korean"

LangString APPNAME ${LANG_ENGLISH} "${NAME}"
LangString APPNAME ${LANG_KOREAN} "${NAME_KO}"
LangString COMPANY ${LANG_ENGLISH} "${PUBLISHER}"
LangString COMPANY ${LANG_KOREAN} "${PUBLISHER_KO}"
Name "$(APPNAME)"

; 설치 파일의 "속성 > 자세히"에 보이는 정보 (언어 설정 뒤에 와야 함)
VIProductVersion "${VERSION}.0"
!macro VersionInfo LANG PRODUCT DESCRIPTION COMPANY HOLDER
    VIAddVersionKey /LANG=${LANG} "ProductName" "${PRODUCT}"
    VIAddVersionKey /LANG=${LANG} "ProductVersion" "${VERSION}"
    VIAddVersionKey /LANG=${LANG} "FileVersion" "${VERSION}"
    VIAddVersionKey /LANG=${LANG} "FileDescription" "${DESCRIPTION}"
    VIAddVersionKey /LANG=${LANG} "CompanyName" "${COMPANY}"
    VIAddVersionKey /LANG=${LANG} "LegalCopyright" "${HOLDER}"
!macroend
!insertmacro VersionInfo ${LANG_ENGLISH} "${NAME}" "${NAME} Setup" "${PUBLISHER}" "${COPYRIGHT}"
!insertmacro VersionInfo ${LANG_KOREAN} "${NAME_KO}" "${NAME_KO} 설치 파일" "${PUBLISHER_KO}" "${COPYRIGHT_KO}"

; 이 설치 파일이 설치한 파일만 하나씩 삭제 (사용자가 넣은 파일은 남김)
!macro RemoveFiles DIR
    Delete "${DIR}\tools\yt-dlp.exe"
    Delete "${DIR}\tools\ffmpeg.exe"
    Delete "${DIR}\tools\ffprobe.exe"
    Delete "${DIR}\tools\deno.exe"
    Delete "${DIR}\licenses\ffmpeg-LICENSE.txt"
    Delete "${DIR}\licenses\ffmpeg-README.txt"
    Delete "${DIR}\licenses\THIRD-PARTY-NOTICES.txt"
    Delete "${DIR}\soundfont\GeneralUser-GS.sf2"
    Delete "${DIR}\soundfont\LICENSE.txt"
    RMDir "${DIR}\soundfont"
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
    File "${STAGE}/licenses/THIRD-PARTY-NOTICES.txt"

    SetOutPath "$INSTDIR\soundfont"
    File "${STAGE}/soundfont/GeneralUser-GS.sf2"
    File "${STAGE}/soundfont/LICENSE.txt"

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
    Delete "$SMPROGRAMS\${OLD_NAME_DAMA}.lnk"
    Delete "$DESKTOP\${OLD_NAME_DAMA}.lnk"
    Delete "$SMPROGRAMS\${OLD_NAME_SHORT}.lnk"
    Delete "$DESKTOP\${OLD_NAME_SHORT}.lnk"
    Delete "$SMPROGRAMS\${OLD_NAME_SHORT_KO}.lnk"
    Delete "$DESKTOP\${OLD_NAME_SHORT_KO}.lnk"
    ; 이전 version이 설치했던 폴더 정리
    !insertmacro RemoveFiles "$LOCALAPPDATA\Programs\${OLD_NAME}"
    !insertmacro RemoveFiles "$LOCALAPPDATA\Programs\${OLD_NAME_DAMA}"

    ; 표시 언어를 바꾼 뒤 다시 설치해도 바로 가기가 두 개가 되지 않게 지우기
    Delete "$SMPROGRAMS\${NAME}.lnk"
    Delete "$DESKTOP\${NAME}.lnk"
    Delete "$SMPROGRAMS\${NAME_KO}.lnk"
    Delete "$DESKTOP\${NAME_KO}.lnk"

    CreateShortcut "$SMPROGRAMS\$(APPNAME).lnk" "$INSTDIR\yt-mp3.exe" "" "$INSTDIR\icon.ico"
    CreateShortcut "$DESKTOP\$(APPNAME).lnk" "$INSTDIR\yt-mp3.exe" "" "$INSTDIR\icon.ico"

    ; Windows의 "설치된 앱" 목록에 등록
    WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "$(APPNAME)"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "$(COMPANY)"
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
    Delete "$SMPROGRAMS\${NAME_KO}.lnk"
    Delete "$DESKTOP\${NAME_KO}.lnk"
    DeleteRegKey HKCU "${UNINSTALL_KEY}"
SectionEnd
