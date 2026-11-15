# Install

**Windows (10+)** — download an installer from [Releases](https://github.com/abenable/hosteldesk/releases), or run:

```powershell
powershell -c "irm https://raw.githubusercontent.com/abenable/hosteldesk/main/scripts/install.ps1|iex"
```

The published installer is a per-user NSIS setup (`HostelDesk_<version>_x64-setup.exe`). It installs silently with `/S`.

**Linux (Ubuntu 22.04+)** — download the `.deb` from [Releases](https://github.com/abenable/hosteldesk/releases) and install it with `sudo apt install ./HostelDesk_<version>_amd64.deb`. The package depends on GTK 3 and the usual X11/Wayland libraries, not a web view.
