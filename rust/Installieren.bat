@echo off
rem Startet den Einrichtungs-Assistenten der Rust-Fassung (umgeht die PowerShell-Skriptsperre nur fuer diese Datei)
powershell -NoProfile -ExecutionPolicy Bypass -STA -WindowStyle Hidden -File "%~dp0install.ps1"
if errorlevel 1 (
  echo.
  echo Die Einrichtung wurde mit einem Fehler beendet.
  echo Bitte die Meldung im PowerShell-Fenster oben notieren.
  pause
)
