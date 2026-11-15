<p align="center">
  <img src="crates/app/icons/icon.png" alt="HostelDesk" width="160" />
</p>

<h1 align="center">HostelDesk</h1>

<p align="center">
  A local-first desktop app for hostel managers — rooms, residents, payments, and PDF receipts.
</p>

<p align="center">
  <a href="https://github.com/abenable/hosteldesk/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/abenable/hosteldesk"></a>
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/license-proprietary-blue.svg"></a>
  <img alt="Platforms" src="https://img.shields.io/badge/platform-windows%20%7C%20linux-lightgrey.svg">
</p>

HostelDesk is a native Rust desktop app (egui). The ledger lives in a SQLite database on the machine and receipts are offline PDFs. A hosted server can sync that cache across a customer's computers when the network is up; the desk keeps working when it is not.

## Install

Download an installer for your platform from [Releases](https://github.com/abenable/hosteldesk/releases).

## Documentation

- [Features](docs/FEATURES.md)
- [Install](docs/INSTALL.md)
- [Development guide](docs/DEVELOPMENT.md)
- [Licensing](docs/LICENSING.md)
- [Releasing](docs/RELEASING.md)

## License

Proprietary — see [`LICENSE`](LICENSE) and [`EULA.md`](EULA.md). Contact ableabenaitwe@gmail.com for licensing, purchasing, or support questions.
