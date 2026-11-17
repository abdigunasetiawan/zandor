@echo off
rem Sekali klik: buat file instalasi Zandor BETA (NSIS .exe) untuk dibagikan. Engine opencode ikut dibundel.
rem Hasil disalin ke folder rilis-beta\ di repo ini.
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\build-beta.ps1"
if errorlevel 1 (
  echo.
  echo GAGAL membuat installer. Baca pesan di atas.
  pause
  exit /b 1
)
echo.
echo Selesai. Bagikan file .exe di folder rilis-beta ke teman-teman.
explorer "%~dp0rilis-beta"
pause
