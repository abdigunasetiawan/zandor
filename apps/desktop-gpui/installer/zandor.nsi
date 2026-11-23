; Installer Zandor (GPUI) — per pengguna, tanpa hak admin (ADR-084).
; Dibangun oleh scripts/rilis.ts:  makensis /DVERSION=X.Y.Z /DSTAGE=<folder berkas> /DOUTFILE=<exe> zandor.nsi
;
; Mode:
;   (biasa)        dialog folder + pemasangan + "Jalankan Zandor"; shortcut Start Menu dan desktop.
;   /S /UPDATE     dipakai pembaruan otomatis: senyap, tidak membuat ulang shortcut desktop yang dihapus pengguna.
; Data pengguna (%APPDATA%\Zandor) tidak disentuh installer maupun uninstaller.

Unicode true
!ifndef VERSION
  !error "VERSION belum di-set (/DVERSION=X.Y.Z)"
!endif
!ifndef STAGE
  !error "STAGE belum di-set (/DSTAGE=folder)"
!endif
!ifndef OUTFILE
  !define OUTFILE "Zandor-Setup-${VERSION}.exe"
!endif

!define APP "Zandor"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Zandor"

Name "${APP}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\Zandor"
InstallDirRegKey HKCU "Software\Zandor" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma
ManifestDPIAware true
BrandingText "${APP} ${VERSION}"

!include MUI2.nsh
!include LogicLib.nsh
!include FileFunc.nsh

!define MUI_ICON "..\assets\icon.ico"
!define MUI_UNICON "..\assets\icon.ico"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\Zandor.exe"

; Ketentuan penggunaan beta: tombol Berikutnya aktif setelah kotak persetujuan dicentang. Mode senyap (/S, dipakai
; pembaruan otomatis) melewati semua halaman, jadi persetujuan hanya diminta saat pemasangan manual.
!define MUI_LICENSEPAGE_CHECKBOX
!insertmacro MUI_PAGE_LICENSE $(TermsFile)
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "Indonesian"
!insertmacro MUI_LANGUAGE "English"

LicenseLangString TermsFile ${LANG_INDONESIAN} "terms-id.txt"
LicenseLangString TermsFile ${LANG_ENGLISH} "terms-en.txt"

VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=${LANG_ENGLISH} "ProductName" "${APP}"
VIAddVersionKey /LANG=${LANG_ENGLISH} "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=${LANG_ENGLISH} "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=${LANG_ENGLISH} "FileDescription" "${APP} Setup"
VIAddVersionKey /LANG=${LANG_ENGLISH} "LegalCopyright" "Zandor"

Var IsUpdate

Function .onInit
  StrCpy $IsUpdate 0
  ${GetParameters} $R0
  ClearErrors
  ${GetOptions} $R0 "/UPDATE" $R1
  ${IfNot} ${Errors}
    StrCpy $IsUpdate 1
  ${EndIf}
FunctionEnd

Section "Zandor" SecMain
  ; Mode senyap melewati berkas yang gagal ditulis (mis. masih terkunci); tandai itu sebagai kegagalan agar
  ; penerap pembaruan mencatat kode keluar bukan nol alih-alih "berhasil" dengan exe lama.
  ClearErrors
  SetOutPath "$INSTDIR"
  File "${STAGE}\Zandor.exe"
  File "${STAGE}\opencode.exe"
  SetOutPath "$INSTDIR\resources"
  File "${STAGE}\resources\zandor-plugin.js"
  SetOutPath "$INSTDIR"
  ${If} ${Errors}
    SetErrorLevel 2
    ${If} $IsUpdate == 1
      Abort
    ${EndIf}
  ${EndIf}

  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKCU "Software\Zandor" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "${APP}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "Zandor"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\Zandor.exe"
  WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "${UNINST_KEY}" "EstimatedSize" "$0"

  CreateShortcut "$SMPROGRAMS\Zandor.lnk" "$INSTDIR\Zandor.exe"
  ${If} $IsUpdate == 0
    CreateShortcut "$DESKTOP\Zandor.lnk" "$INSTDIR\Zandor.exe"
  ${EndIf}
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\Zandor.exe"
  Delete "$INSTDIR\opencode.exe"
  Delete "$INSTDIR\resources\zandor-plugin.js"
  RMDir "$INSTDIR\resources"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\Zandor.lnk"
  Delete "$DESKTOP\Zandor.lnk"
  RMDir /r "$LOCALAPPDATA\Zandor\updates"
  DeleteRegKey HKCU "${UNINST_KEY}"
  DeleteRegKey HKCU "Software\Zandor"
SectionEnd
