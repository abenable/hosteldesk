#!/usr/bin/env bash
# Build the Windows NSIS installer from the repo root.
#
#   packaging/windows/build-installer.sh <version> <outfile>
#
# Expects target/release/hosteldesk.exe. Used by the release workflow, and by
# CI on a Windows runner (with a placeholder exe) so packaging breaks show up
# on pull requests, not on release day.
set -euo pipefail

version="$1"
outfile="$2"

# Chocolatey installs NSIS without putting it on PATH for later steps.
makensis="$(command -v makensis || true)"
if [[ -z "$makensis" ]]; then
  for candidate in "/c/Program Files (x86)/NSIS/makensis.exe" "/c/Program Files/NSIS/makensis.exe"; do
    [[ -x "$candidate" ]] && makensis="$candidate" && break
  done
fi
[[ -n "$makensis" ]] || { echo "makensis not found; install NSIS first" >&2; exit 1; }

# Windows makensis splits File paths only on backslashes, so give it native
# paths there; forward slashes elsewhere.
native() {
  if command -v cygpath >/dev/null; then cygpath -w "$1"; else printf '%s' "$1"; fi
}

# -NOCD keeps the repo root as the base for relative paths (makensis otherwise
# changes into the script's folder).
"$makensis" -NOCD \
  -DVERSION="$version" \
  -DOUTFILE="$(native "$outfile")" \
  -DBINARY="$(native target/release/hosteldesk.exe)" \
  -DICON="$(native crates/app/icons/icon.ico)" \
  packaging/windows/hosteldesk.nsi
