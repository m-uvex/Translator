#!/usr/bin/env bash
# Entfernt die Linux-Installation - Gegenstueck zu Uninstall.ps1.
set -euo pipefail

DIR="${UEBERSETZER_DIR:-$HOME/.local/opt/uebersetzer}"
DESKTOP="$HOME/.local/share/applications/uebersetzer.desktop"
ICON="$HOME/.local/share/icons/hicolor/512x512/apps/uebersetzer.png"
CONF="${XDG_CONFIG_HOME:-$HOME/.config}/Übersetzer"

say() { printf '  %s\n' "$*"; }
ask() {
  local a
  read -r -p "$1 [J/n] " a || true
  [ -z "${a:-}" ] && return 0
  case "${a,,}" in j | ja | y | yes) return 0 ;; *) return 1 ;; esac
}

# Nur die eigene Installation beenden, nicht jedes Electron auf dem Rechner.
pkill -f -- "$DIR/node_modules/electron/dist/electron" 2>/dev/null || true
pkill -f -- "$DIR/nllb-venv/bin/python3" 2>/dev/null || true
say "Laufende Instanzen aus $DIR beendet"

rm -f -- "$DESKTOP" "$ICON"
say "Startmenue-Eintrag und Icon entfernt"

if [ -d "$DIR" ]; then
  if ask "Programmordner $DIR loeschen?"; then
    rm -rf -- "$DIR"
    say "Programmordner geloescht"
  else
    say "Programmordner bleibt liegen"
  fi
fi

if [ -d "$CONF" ]; then
  say "Daten: Ablage, Woerterbuch und Einstellungen in $CONF"
  if ask "Auch diese Daten loeschen?"; then
    rm -rf -- "$CONF"
    say "Daten geloescht"
  else
    say "Daten bleiben erhalten"
  fi
fi

say "Fertig"
