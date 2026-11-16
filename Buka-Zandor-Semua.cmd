@echo off
rem Sekali klik: buka Zandor prod (stabil) dan Zandor dev sekaligus.
cd /d "%~dp0"
call "%~dp0Buka-Zandor-Prod.cmd"
start "Zandor Dev" cmd /k "%~dp0Buka-Zandor-Dev.cmd"
