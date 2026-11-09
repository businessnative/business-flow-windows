#!/usr/bin/env bash
# Replace the placeholder identifier com.example.businessflow.
# Usage: scripts/set-bundle-id.sh com.yourorg.businessflow
set -euo pipefail
if [[ $# -ne 1 ]]; then
  echo "Használat: scripts/set-bundle-id.sh com.sajatszervezet.businessflow" >&2
  exit 1
fi
NEW="$1"
if [[ ! "$NEW" =~ ^[a-zA-Z0-9]+(\.[a-zA-Z0-9_-]+)+$ ]]; then
  echo "Az azonosító pontokkal tagolt legyen, például com.ceg.businessflow" >&2
  exit 1
fi
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export NEW
python3 - <<'PY' "$ROOT" "$NEW"
import pathlib, sys
root, new = pathlib.Path(sys.argv[1]), sys.argv[2]
old = "com.example.businessflow"
files = [
    root / "src-tauri" / "tauri.conf.json",
    root / "src-tauri" / "src" / "identity.rs",
    root / "README.md",
    root / "SECURITY.md",
]
for path in files:
    text = path.read_text(encoding="utf-8")
    if old not in text:
        raise SystemExit(f"nem találom a helyőrzőt: {path}")
    path.write_text(text.replace(old, new), encoding="utf-8")
    print(f"frissítve: {path}")
PY
echo "Kész. A már elmentett kulcsok a régi azonosító alatt maradnak a kulcstartóban; mentsd őket újra az alkalmazásban."
