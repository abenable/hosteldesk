; Per-user HostelDesk installer. Silent install is `/S`, which scripts/install.ps1 uses.
!include "MUI2.nsh"

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!ifndef OUTFILE
  !define OUTFILE "HostelDesk_setup.exe"
!endif
!ifndef BINARY
  !define BINARY "hosteldesk.exe"
!endif

Name "HostelDesk ${VERSION}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\HostelDesk"
RequestExecutionLevel user

!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "HostelDesk"
  SetOutPath "$INSTDIR"
  File "${BINARY}"
  CreateShortcut "$SMPROGRAMS\HostelDesk.lnk" "$INSTDIR\hosteldesk.exe"
  WriteUninstaller "$INSTDIR\uninstall.exe"
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\hosteldesk.exe"
  Delete "$INSTDIR\uninstall.exe"
  Delete "$SMPROGRAMS\HostelDesk.lnk"
  RMDir "$INSTDIR"
SectionEnd
