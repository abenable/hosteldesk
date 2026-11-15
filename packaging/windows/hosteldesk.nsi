; Per-user HostelDesk installer. Silent install is `/S`, which scripts/install.ps1 uses.
;
; Build from the repo root with -NOCD so relative paths resolve there:
;   makensis -NOCD -DVERSION=2.0.0 -DOUTFILE=out.exe -DBINARY=target/release/hosteldesk.exe packaging/windows/hosteldesk.nsi
;
; Installs into the same folder, Start-menu shortcut and Uninstall registry key
; the 1.x (Tauri) installer used, so upgrading replaces it instead of leaving a
; second copy in Apps & features. The database lives in %APPDATA% and is never
; touched by install or uninstall.
!include "MUI2.nsh"

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!ifndef OUTFILE
  !define OUTFILE "HostelDesk_setup.exe"
!endif
!ifndef BINARY
  !define BINARY "target/release/hosteldesk.exe"
!endif
!define ICON "crates/app/icons/icon.ico"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\HostelDesk"

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
