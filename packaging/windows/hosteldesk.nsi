; Per-user HostelDesk installer. Silent install is `/S`, which scripts/install.ps1 uses.
;
; Build from the repo root with -NOCD so relative paths resolve there:
;   packaging/windows/build-installer.sh <version> <outfile>
; (it runs makensis -NOCD with native paths; Windows makensis needs backslashes in File paths)
;
; Installs into the same folder, Start-menu shortcut and Uninstall registry key
; the 1.x (Tauri) installer used, so upgrading replaces it instead of leaving a
; second copy in Apps & features. The database lives in %APPDATA% and is never
; touched by install or uninstall.
;
; HostelDesk draws its window with Microsoft Edge WebView2. Windows 11 (and
; updated Windows 10) ships the runtime; when it is missing, the installer runs
; Microsoft's Evergreen bootstrapper (embedded by build-installer.sh) and refuses to
; finish if the runtime still is not there, so nobody ends up with an app that
; cannot open.
!include "MUI2.nsh"
!include "LogicLib.nsh"

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!ifndef OUTFILE
  !define OUTFILE "HostelDesk_setup.exe"
!endif
!ifndef BINARY
  !define BINARY "target/release/hosteldesk.exe"
!endif
!ifndef ICON
  !define ICON "crates/app/icons/icon.ico"
!endif
!ifndef WEBVIEW2
  !define WEBVIEW2 "target/MicrosoftEdgeWebview2Setup.exe"
!endif
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\HostelDesk"
; WebView2 Runtime's EdgeUpdate client id. This 32-bit installer reads HKLM
; through WOW6432Node automatically; HKCU holds a per-user runtime.
!define WEBVIEW2KEY "SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"

; $0 = installed WebView2 Runtime version, or "" when there is none.
!macro WebView2Version
  ReadRegStr $0 HKLM "${WEBVIEW2KEY}" "pv"
  ${If} $0 == ""
  ${OrIf} $0 == "0.0.0.0"
    ReadRegStr $0 HKCU "${WEBVIEW2KEY}" "pv"
  ${EndIf}
  ${If} $0 == "0.0.0.0"
    StrCpy $0 ""
  ${EndIf}
!macroend

Name "HostelDesk ${VERSION}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\HostelDesk"
RequestExecutionLevel user
!define MUI_ICON "${ICON}"
!define MUI_UNICON "${ICON}"

!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "-WebView2"
  !insertmacro WebView2Version
  ${If} $0 == ""
    DetailPrint "Installing the Microsoft Edge WebView2 Runtime..."
    InitPluginsDir
    File "/oname=$PLUGINSDIR\MicrosoftEdgeWebview2Setup.exe" "${WEBVIEW2}"
    ExecWait '"$PLUGINSDIR\MicrosoftEdgeWebview2Setup.exe" /silent /install' $1
    !insertmacro WebView2Version
    ${If} $0 == ""
      MessageBox MB_ICONSTOP "HostelDesk needs the Microsoft Edge WebView2 Runtime, which could not be installed (code $1). Connect to the internet and run this installer again." /SD IDOK
      Abort
    ${EndIf}
  ${EndIf}
SectionEnd

Section "HostelDesk"
  SetOutPath "$INSTDIR"
  File "${BINARY}"
  File /oname=hosteldesk.ico "${ICON}"
  CreateShortcut "$SMPROGRAMS\HostelDesk.lnk" "$INSTDIR\hosteldesk.exe" "" "$INSTDIR\hosteldesk.ico"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  WriteRegStr HKCU "${UNINSTKEY}" "DisplayName" "HostelDesk"
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTKEY}" "Publisher" "HostelDesk"
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayIcon" "$INSTDIR\hosteldesk.ico"
  WriteRegStr HKCU "${UNINSTKEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTKEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINSTKEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNINSTKEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTKEY}" "NoRepair" 1
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\hosteldesk.exe"
  Delete "$INSTDIR\hosteldesk.ico"
  Delete "$INSTDIR\uninstall.exe"
  Delete "$SMPROGRAMS\HostelDesk.lnk"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "${UNINSTKEY}"
SectionEnd
