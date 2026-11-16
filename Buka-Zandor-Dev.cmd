@echo off
rem Sekali klik: jalankan Zandor dev (Vite + tauri dev, konfigurasi tauri.dev.conf.json).
rem Jendela ini harus tetap terbuka; tutup untuk menghentikan dev server.
title Zandor Dev
cd /d "%~dp0apps\desktop"
bun run tauri:dev
if errorlevel 1 (
  echo.
  echo GAGAL menjalankan Zandor dev. Baca pesan di atas.
  pause
)
