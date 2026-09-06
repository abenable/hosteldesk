# Install

**Windows (10+)** — download an installer from [Releases](https://github.com/abenable/hosteldesk/releases), or run:

```powershell
powershell -c "irm https://raw.githubusercontent.com/abenable/hosteldesk/main/scripts/install.ps1|iex"
```

Both a per-user setup `.exe` and a machine-wide MSI are published; the MSI supports silent install with `msiexec /i HostelDesk_<version>_x64_en-US.msi /qn`.

**Linux (Ubuntu 22.04+)** — download the `.deb` from [Releases](https://github.com/abenable/hosteldesk/releases) and install it with `sudo apt install ./HostelDesk_<version>_amd64.deb`. Runtime dependencies (WebKitGTK and friends) are declared by the package.
