@echo off
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\monitor-zandor.ps1" %*
