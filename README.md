<div align="center">

# MyVPN

### Your own serverless, peer-to-peer VPN

Host your network from one device, connect from anywhere on another — **no middleware server, end-to-end encrypted.**

[![Download](https://img.shields.io/badge/Download-Windows%20Installer-2ea44f?style=for-the-badge&logo=windows)](https://github.com/sayantanmandal1/MyVPN/releases/latest/download/MyVPN-Setup-x64.exe)
[![Latest Release](https://img.shields.io/github/v/release/sayantanmandal1/MyVPN?style=for-the-badge)](https://github.com/sayantanmandal1/MyVPN/releases/latest)
[![Build](https://img.shields.io/github/actions/workflow/status/sayantanmandal1/MyVPN/release.yml?style=for-the-badge)](https://github.com/sayantanmandal1/MyVPN/actions)

</div>

---

## What is MyVPN?

MyVPN turns any two Windows machines into your own private VPN. On one device you click
**Host** and give your network a name. On the other device you **Connect** to it — and
all of your traffic now flows securely through your home network, exactly like a
commercial VPN, except it is entirely **yours** and runs with **no server in the middle**.

- **Host once, connect from anywhere.** On the same network, hosts appear by name
  automatically. Across the internet, you connect with the host's **pairing code**,
  with an optional passphrase to control who may join.
- **Full tunnel.** Browse the internet through your host's connection and reach every
  device on the host's local network.
- **Truly peer-to-peer.** Built on [`iroh`](https://www.iroh.computer/) QUIC with NAT
  hole-punching. Traffic is end-to-end encrypted (TLS 1.3) and, once connected, flows
  **directly** between your devices.
- **Zero downtime.** Enable *Start on boot* and MyVPN re-hosts your network automatically
  after every restart, living quietly in the system tray.
- **Two modes, one app.** Use your **private** peer-to-peer VPN, or switch to the separate
  **Public servers** section to route through a free, volunteer-run server in another
  country (searchable, multiple servers per country). See
  [`docs/PUBLIC_VPN.md`](docs/PUBLIC_VPN.md).

## Download

> **[⬇ Download the latest Windows installer](https://github.com/sayantanmandal1/MyVPN/releases/latest/download/MyVPN-Setup-x64.exe)**

Installers are built automatically for every release. You can always grab the newest one
from the [releases page](https://github.com/sayantanmandal1/MyVPN/releases/latest).

After installing, launch **MyVPN**. Because a VPN creates a system network adapter, Windows
will ask for Administrator permission — this is required and expected.

## How it works

```
┌──────────────┐         encrypted QUIC          ┌──────────────┐
│   Device B   │  ◀────  (direct P2P / relay) ───▶ │   Device A   │
│  "Connect"   │                                   │   "Host"     │
│              │                                   │              │
│  Wintun TUN  │   all traffic ──▶ host ──▶ 🌐     │  NAT gateway │
└──────────────┘                                   └──────────────┘
        ▲                                                  │
        └────────  internet & host LAN reachable  ◀────────┘
```

1. **Host** creates an `iroh` endpoint (an Ed25519 keypair) and advertises a friendly name.
2. **Connect** discovers the host — on the LAN automatically by name, or across the
   internet via the host's pairing code resolved through `iroh`'s public discovery — and
   opens an encrypted QUIC connection.
3. The connecting device captures **all** of its traffic through a Wintun adapter and pipes
   it over the tunnel. The host forwards it to the internet (NAT) and back. The tunnel's own
   carrier packets are pinned to the physical gateway, so routing never loops.

## A note on "no server"

On the **same network**, MyVPN uses zero infrastructure — devices find each other directly.
For **cross-internet** connections, MyVPN relies on **public, community-run infrastructure that
you do not operate**: `iroh`'s NAT-traversal relays and public discovery service, used to set up
the connection and resolve a pairing code to its current address. Once the connection is
established it flows **directly** between your devices whenever hole-punching succeeds. You run
**no server of your own** — but if you want total sovereignty you can point MyVPN at **your own
relay** in Settings; see [`docs/RELAY.md`](docs/RELAY.md). A relay only assists the connection and
never sees your (end-to-end-encrypted) traffic.

## Good to know

- **One connection at a time.** A host serves a single connected device at a time; another
  device can take over once it's free. Multi-client hosting is on the roadmap.
- **Connecting across the internet.** On the same network, hosts appear by name automatically.
  Across the internet, paste the host's pairing code. (Name + passphrase discovery via DHT is
  planned.)
- **Administrator rights.** MyVPN creates a real network adapter and system routes, so it runs
  elevated. The installer and the *Start on boot* task handle that for you.
- **Resilient by design.** If the link drops, MyVPN holds a kill-switch (your traffic is blocked,
  never leaked) and transparently reconnects to the host in the background.
- **Bring your own relay or exit.** Connection relays are automatic and multi-region, or you can
  [self-host one](docs/RELAY.md). For an exit in a specific country, run a MyVPN host on a VPS
  there and connect to it — your exit, your infrastructure, no third party in the middle.

## Build from source

```powershell
# Prerequisites: Node.js 18+, Rust (stable, MSVC), and the Tauri prerequisites for Windows.
git clone https://github.com/sayantanmandal1/MyVPN.git
cd MyVPN/app

# Fetch the Wintun driver (one-time; the installer bundles it automatically in CI).
powershell -ExecutionPolicy Bypass -File ../scripts/fetch-wintun.ps1

npm install
npm run tauri dev      # run locally (launch elevated for the tunnel)
npm run tauri build    # produce an installer in app/src-tauri/target/release/bundle
```

To cut a release, the maintainer simply tags a version:

```powershell
git tag v0.0.1
git push --tags
```

The [release workflow](.github/workflows/release.yml) downloads the Wintun driver, builds the
installer, and publishes it to a GitHub Release automatically.

## Project structure

| Path                | Description                                             |
| ------------------- | ------------------------------------------------------- |
| [`app/`](app/)      | The MyVPN desktop application (Tauri + Rust + React).   |
| [`web/`](web/)      | Marketing & showcase website (Next.js + shadcn/ui).     |
| `.github/workflows` | CI that builds installers and publishes releases.       |

## Security

MyVPN is built with security as a first-class concern: end-to-end TLS 1.3 encryption,
passphrase-authenticated peers, no secrets in logs, an on-device identity key encrypted at rest
with Windows DPAPI (tied to your user account), and a kill-switch that blocks traffic if the
tunnel drops while it reconnects. Release installers can be Authenticode-signed — see
[`docs/SIGNING.md`](docs/SIGNING.md). For more detail see [`app/`](app/).

## License

MIT

