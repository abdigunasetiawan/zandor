@echo off
rem Sekali klik: buka Zandor stabil (prod) hasil Perbarui-Zandor.cmd.
set "EXE=%LOCALAPPDATA%\ZandorStable\zandor-desktop.exe"
if not exist "%EXE%" (
  echo Zandor stabil belum terpasang di %LOCALAPPDATA%\ZandorStable.
  echo Jalankan Perbarui-Zandor.cmd dulu.
  pause
  exit /b 1
)
start "" "%EXE%"
