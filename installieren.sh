#!/usr/bin/env bash
# Doppelklick-Einstieg unter Linux - Gegenstueck zu installieren.bat.
# Ohne Terminal oeffnet es eines der ueblichen Terminals und startet install.sh.
set -euo pipefail
DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

if [ -t 0 ]; then
  exec bash "$DIR/install.sh" "$@"
fi

for t in x-terminal-emulator gnome-terminal konsole xfce4-terminal mate-terminal tilix; do
  command -v "$t" >/dev/null || continue
  case "$t" in
  gnome-terminal | mate-terminal | tilix) exec "$t" -- bash "$DIR/install.sh" "$@" ;;
  *) exec "$t" -e bash "$DIR/install.sh" "$@" ;;
  esac
done

printf 'Kein Terminal gefunden. Bitte von Hand starten:\n  bash "%s/install.sh"\n' "$DIR" >&2
exit 1
