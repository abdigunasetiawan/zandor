@echo off
rem Sekali klik: bangun Zandor dari repo ini, ganti salinan stabil, lalu buka (ADR-061).
rem Salinan lama dicadangkan ke %LOCALAPPDATA%\ZandorStable.bak. Data pengguna tidak tersentuh.
cd /d "%~dp0"
bun scripts/install-stable.ts %*
if errorlevel 1 (
  echo.
  echo GAGAL. Baca pesan di atas. Versi lama tidak diubah bila gagal sebelum tahap salin.
  pause
  exit /b 1
)
echo.
echo Selesai.
timeout /t 5 >nul
