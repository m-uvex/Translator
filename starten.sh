#!/usr/bin/env bash
# Startet die App aus diesem Ordner - Gegenstueck zu Starten.vbs unter Windows.
set -euo pipefail
DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd "$DIR"

ELECTRON="$DIR/node_modules/electron/dist/electron"
if [ ! -x "$ELECTRON" ]; then
  printf 'Electron fehlt unter %s.\n' "$ELECTRON" >&2
  printf 'Einmalig einrichten: bash "%s/install.sh"\n' "$DIR" >&2
  exit 1
fi

exec "$ELECTRON" "$DIR" "$@"
