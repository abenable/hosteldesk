#!/usr/bin/env bash
# Registers a .desktop entry for the dev build so the taskbar/dock shows the
# HostelDesk icon instead of a generic one. Linux resolves that icon by
# matching the running window's app-id (set in crates/app/src/lib.rs) against
# an installed .desktop file's StartupWMClass -- there is no way to do this
# from inside the binary itself, so it's a one-time step per machine.
#
# Run once after cloning: ./scripts/install-dev-desktop.sh
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bin_path="$repo_root/target/debug/app"
icon_path="$repo_root/crates/app/icons/icon.png"
desktop_dir="$HOME/.local/share/applications"
desktop_file="$desktop_dir/com.hosteldesk.app.desktop"

mkdir -p "$desktop_dir"
cat >"$desktop_file" <<EOF
[Desktop Entry]
Type=Application
Name=HostelDesk (dev)
Comment=HostelDesk development build
Exec=$bin_path
Icon=$icon_path
Terminal=false
Categories=Utility;
StartupWMClass=com.hosteldesk.app
EOF

command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$desktop_dir" || true

echo "Installed $desktop_file"
echo "Build the app once (cargo build -p app or cargo tauri dev), then relaunch it -- the taskbar/dock icon should show HostelDesk."
