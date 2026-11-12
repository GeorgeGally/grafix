#!/usr/bin/env bash
# Serves the production build at http://localhost:4173/ (run build-web.sh first).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/apps/editor"
exec npm run preview -- --host 127.0.0.1
