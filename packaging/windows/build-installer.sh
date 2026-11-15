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

# On Windows, makensis wants a native path for the output file.
command -v cygpath >/dev/null && outfile="$(cygpath -w "$outfile")"

# -NOCD keeps the repo root as the base for relative paths (makensis otherwise
# changes into the script's folder).
"$makensis" -NOCD \
  -DVERSION="$version" \
  -DOUTFILE="$outfile" \
  -DBINARY=target/release/hosteldesk.exe \
  packaging/windows/hosteldesk.nsi
