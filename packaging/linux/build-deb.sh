#!/usr/bin/env bash
# Assemble a .deb from the release binary. Run from the repo root after
# `npm --prefix frontend ci && npm --prefix frontend run build` and
# `cargo build --release -p app --features custom-protocol` (without the
# feature the binary loads http://localhost:5173 instead of its embedded UI).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)"
stage="$(mktemp -d)"
out="${1:-$root/HostelDesk_${version}_amd64.deb}"

mkdir -p \
  "$stage/DEBIAN" \
  "$stage/usr/bin" \
  "$stage/usr/share/applications" \
  "$stage/usr/share/icons/hicolor/128x128/apps"

cp "$root/target/release/hosteldesk" "$stage/usr/bin/hosteldesk"
chmod 755 "$stage/usr/bin/hosteldesk"
cp "$root/packaging/linux/com.hosteldesk.app.desktop" "$stage/usr/share/applications/"
cp "$root/crates/app/icons/128x128.png" \
  "$stage/usr/share/icons/hicolor/128x128/apps/com.hosteldesk.app.png"

cat >"$stage/DEBIAN/control" <<EOF
Package: hosteldesk
Version: ${version}
Section: utils
Priority: optional
Architecture: amd64
Maintainer: Able Abenaitwe <ableabenaitwe@gmail.com>
Depends: libwebkit2gtk-4.1-0, libgtk-3-0
Description: Hostel desk for rooms, residents, payments, and receipts
 Local-first desktop app. Each computer keeps a SQLite cache and can sync
 with the hosted ledger when a network is available.
EOF

dpkg-deb --root-owner-group --build "$stage" "$out"
echo "$out"
