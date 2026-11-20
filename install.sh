#!/usr/bin/env bash
# Einrichtungs-Assistent der Electron-Fassung fuer Linux.
# Gegenstueck zu install.ps1: gleiche Schritte, gleiche Dienste, aber ohne
# Administratorrechte und ohne Windows-Formulare.
set -euo pipefail

HERE="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
TARGET="${UEBERSETZER_DIR:-$HOME/.local/opt/uebersetzer}"
ASSUME_YES=0
WITH_MODELS="nllb:600M"
FILES=(main.js preload.js index.html style.css renderer.js editor.js nllb_server.py package.json requirements.txt)
SCRIPTS=(install.sh installieren.sh starten.sh Uninstall.sh)

say() { printf '  %s\n' "$*"; }
step() { printf '\n== %s\n' "$*"; }
die() { printf '\nFehler: %s\n' "$*" >&2; exit 1; }
ask() {
  [ "$ASSUME_YES" = 1 ] && return 0
  local a
  read -r -p "$1 [J/n] " a || true
  [ -z "${a:-}" ] && return 0
  case "${a,,}" in j | ja | y | yes) return 0 ;; *) return 1 ;; esac
}

usage() {
  cat <<'TXT'
Aufruf: bash install.sh [Optionen]

  --dir PFAD      Zielordner (Vorgabe: ~/.local/opt/uebersetzer)
  --models LISTE  Uebersetzungsmodelle, durch Leerzeichen getrennt (Vorgabe:
                  "nllb:600M"; auch "nllb:1.3B", "hy:1.8B", "hy:7B",
                  "milmmt:1B", "milmmt:4B"; leer lassen mit --models "")
  -y, --yes       ohne Rueckfragen
  -h, --help      diese Hilfe

Das Skript kopiert die App in den Zielordner, installiert Electron und die
npm-Pakete, legt die Python-Umgebung nllb-venv an, laedt auf Wunsch Modelle,
schreibt config.json und legt einen Startmenue-Eintrag an.
TXT
}

while [ $# -gt 0 ]; do
  case "$1" in
  --dir)
    TARGET="${2:?Pfad fehlt}"
    shift 2
    ;;
  --models)
    WITH_MODELS="${2-}"
    shift 2
    ;;
  -y | --yes)
    ASSUME_YES=1
    shift
    ;;
  -h | --help)
    usage
    exit 0
    ;;
  *) die "Unbekannte Option: $1 (--help zeigt die Hilfe)" ;;
  esac
done

step "Voraussetzungen pruefen"
command -v node >/dev/null || die "Node.js fehlt. Ueber die Paketverwaltung installieren (z. B. sudo apt install nodejs npm) und erneut starten."
command -v npm >/dev/null || die "npm fehlt. Ueber die Paketverwaltung installieren (z. B. sudo apt install npm)."
NODE_MAJOR="$(node -p 'process.versions.node.split(".")[0]')"
[ "$NODE_MAJOR" -ge 18 ] || die "Node.js ist zu alt ($(node -v)); noetig ist 18 oder neuer."
PY="$(command -v python3 || true)"
[ -n "$PY" ] || die "python3 fehlt. Ueber die Paketverwaltung installieren (z. B. sudo apt install python3)."
"$PY" -c 'import venv' 2>/dev/null || die "Das Modul venv fehlt (unter Debian/Ubuntu: sudo apt install python3-venv)."
say "Node.js $(node -v), npm $(npm -v), $PY"

step "Dateien nach $TARGET kopieren"
mkdir -p "$TARGET"
if [ "$HERE" != "$TARGET" ]; then
  for f in "${FILES[@]}"; do [ -f "$HERE/$f" ] && cp -f "$HERE/$f" "$TARGET/"; done
  for s in "${SCRIPTS[@]}"; do [ -f "$HERE/$s" ] && cp -f "$HERE/$s" "$TARGET/"; done
  for d in build dict; do [ -d "$HERE/$d" ] && cp -a "$HERE/$d" "$TARGET/"; done
fi
chmod +x "$TARGET"/*.sh 2>/dev/null || true
say "Quelle: $HERE"
say "Ziel:   $TARGET"

step "Electron und Abhaengigkeiten installieren"
(cd "$TARGET" && npm install --omit=dev --no-audit --no-fund)
[ -x "$TARGET/node_modules/electron/dist/electron" ] ||
  die "Electron wurde nicht installiert. '$TARGET/node_modules/electron' pruefen und den Schritt wiederholen."

step "Python-Umgebung fuer NLLB einrichten"
say "torch und transformers belegen mehrere hundert MB, der erste Lauf dauert entsprechend."
VENV="$TARGET/nllb-venv"
[ -x "$VENV/bin/python3" ] || "$PY" -m venv "$VENV"
"$VENV/bin/python3" -m pip install --upgrade pip >/dev/null
"$VENV/bin/python3" -m pip install --progress-bar off -r "$TARGET/requirements.txt"

step "Ollama pruefen"
if curl -fsS --max-time 2 http://127.0.0.1:11434 >/dev/null 2>&1; then
  say "Ollama laeuft bereits auf 127.0.0.1:11434."
elif command -v ollama >/dev/null; then
  say "Ollama gefunden: $(command -v ollama)"
else
  say "Ollama fehlt. Die offizielle Installation braucht Administratorrechte:"
  say "  curl -fsSL https://ollama.com/install.sh | sh"
  say "Danach dieses Skript erneut starten. Die App startet Ollama selbst, sobald es im PATH liegt."
fi

step "NLLB-Modelle"
if [ -n "$WITH_MODELS" ]; then
  for id in $WITH_MODELS; do
    say "Modell $id"
    if ! "$VENV/bin/python3" -u "$TARGET/nllb_server.py" --install "$id"; then
      say "Uebersprungen: $id (Meldung oben; Modelle lassen sich spaeter in der App nachladen)"
    fi
  done
else
  say 'Keine Modelle angefordert; spaeter mit --models "nllb:600M hy:1.8B" oder in der App nachladen.'
fi

step "Autostart des NLLB-Servers eintragen"
printf '{ "nllbAutostart": true }\n' >"$TARGET/config.json"
say "$TARGET/config.json geschrieben"

step "Startmenue-Eintrag anlegen"
APPS="$HOME/.local/share/applications"
ICONS="$HOME/.local/share/icons/hicolor/512x512/apps"
mkdir -p "$APPS" "$ICONS"
[ -f "$TARGET/build/icon.png" ] && cp -f "$TARGET/build/icon.png" "$ICONS/uebersetzer.png"
cat >"$APPS/uebersetzer.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Übersetzer
Comment=Lokaler Übersetzer und Texteditor
Exec=$TARGET/starten.sh
Path=$TARGET
Icon=uebersetzer
Terminal=false
Categories=Utility;TextTools;
StartupWMClass=uebersetzer
EOF
chmod +x "$APPS/uebersetzer.desktop"
command -v update-desktop-database >/dev/null && update-desktop-database "$APPS" >/dev/null 2>&1 || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" >/dev/null 2>&1 || true
say "$APPS/uebersetzer.desktop"

step "Sprachausgabe pruefen"
if command -v spd-say >/dev/null 2>&1 || command -v espeak-ng >/dev/null 2>&1 || command -v espeak >/dev/null 2>&1; then
  say "Vorlesen: $(command -v spd-say || command -v espeak-ng || command -v espeak)"
else
  say "Vorlesen fehlt: die App spricht ueber spd-say (speech-dispatcher) oder espeak-ng."
  say "Das braucht Administratorrechte, deshalb hier nur der Hinweis:"
  say "  sudo apt install espeak-ng            (oder: sudo apt install speech-dispatcher)"
fi

step "Fertig"
say "Starten:  $TARGET/starten.sh"
say "Entfernen: $TARGET/Uninstall.sh"
say "Daten:    ${XDG_CONFIG_HOME:-$HOME/.config}/Übersetzer"

if ask "Jetzt starten?"; then
  exec "$TARGET/starten.sh"
fi
