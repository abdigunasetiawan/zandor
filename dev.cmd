@echo off
rem Skrip tunggal development Zandor GPUI: auto rebuild + restart saat file disimpan, plus monitor CPU/RAM.
rem   dev.cmd         mode jeda (restart setelah Enter agar state yang sedang dipakai tidak hilang)
rem   dev.cmd -Auto   restart otomatis tiap build sukses
rem   dev.cmd -NoMonitor  tanpa jendela monitor
rem Hentikan dengan Ctrl+C; jendela monitor ikut ditutup.
title Zandor Dev
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\dev.ps1" %*
