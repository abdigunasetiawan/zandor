@echo off
rem Klik dua kali untuk membangun (bila ada perubahan) lalu menjalankan Zandor GPUI (debug).
setlocal
cd /d "%~dp0apps\desktop-gpui"
set CARGO_INCREMENTAL=0
echo Membangun Zandor...
cargo build
if errorlevel 1 (
  echo.
  echo Build gagal. Periksa pesan di atas ^(disk penuh? coba: cargo clean^).
  pause
  exit /b 1
)
echo Menjalankan Zandor...
start "" "target\debug\zandor-gpui.exe"
endlocal
