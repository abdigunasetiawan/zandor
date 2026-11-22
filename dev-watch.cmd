@echo off
rem Mode pengembangan: bangun ulang dan jalankan ulang Zandor GPUI otomatis saat file disimpan.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\dev-watch.ps1"
