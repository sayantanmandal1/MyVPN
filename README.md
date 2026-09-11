# MyVPN

### Your own serverless, peer-to-peer VPN

Host your network from one device, connect from anywhere on another — no middleware server,
end-to-end encrypted.

[Download](https://github.com/sayantanmandal1/MyVPN/releases/latest/download/MyVPN-Setup-x64.exe) ·
[Latest release](https://github.com/sayantanmandal1/MyVPN/releases/latest) ·
[Build](https://github.com/sayantanmandal1/MyVPN/actions)

---

## What is MyVPN?

MyVPN turns any two Windows machines into your own private VPN. On one device you click **Host** and
give your network a name. On the other you **Connect** to it — and all of your traffic flows securely
through your home network, exactly like a commercial VPN, except it is entirely yours and runs with
no server in the middle.

- **Host once, connect from anywhere.** On the same network, hosts appear by name automatically.
  Across the internet you connect with the host's pairing code, with an optional passphrase to
  control who may join.
- **Full tunnel.** Browse the internet through your host's connection and reach every device on the
  host's local network.
- **Truly peer-to-peer.** Built on [iroh](https://www.iroh.computer/) QUIC with NAT hole-punching.
  Traffic is end-to-end encrypted (TLS 1.3) and, once connected, flows directly between your devices.
- **Zero downtime.** Enable *Start on boot* and MyVPN re-hosts your network automatically after every
  restart, living quietly in the system tray.
- **Two modes, one app.** Use your private peer-to-peer VPN, or switch to the separate *Public
  servers* section to route through a free, volunteer-run server in another country.

---

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

1. The host creates an `iroh` endpoint (an Ed25519 keypair) and advertises a friendly name.
2. Connect discovers the host — on the LAN automatically by name, or across the internet via the
   host's pairing code resolved through `iroh`'s public discovery — and opens an encrypted QUIC
   connection.
3. The connecting device captures all of its traffic through a Wintun adapter and pipes it over the
   tunnel. The host forwards it to the internet (NAT) and back. The tunnel's own carrier packets are
   pinned to the physical gateway, so routing never loops.

---

## Security

A VPN is only as good as the controls nobody bothered to test. These are the ones this codebase
relies on, and each is now covered by unit tests (`cargo test --lib`, 15 passing):

| control | why it matters | test |
|---------|----------------|------|
| **Command-injection guard** (`vpn::net::ensure_ip`) | Every route, DNS and NAT change shells out to `netsh` / PowerShell with an interpolated address. A value that is not a bare IP literal would execute as a command **with Administrator rights**. | Rejects 8 injection payloads (`;`, `\|`, `&`, `$( )`, backtick, quote-escape) and 9 malformed values; accepts IPv4, IPv6 and whitespace-padded literals. |
| **Constant-time proof comparison** (`constant_time_eq`) | The host compares the client's passphrase proof. A short-circuiting `==` leaks how many bytes matched, turning a 256-bit secret into a byte-at-a-time guessing game. | Matches ordinary equality on content, and returns `false` on length mismatch without early exit. |
| **Deterministic identity derivation** (`derive_secret`) | Both peers must independently derive the *same* Ed25519 identity from name + passphrase, or they can never connect. | Deterministic across calls; name normalisation makes `"Home Network"`, `"home network"` and `"  HOME NETWORK  "` dial the same host. |
| **Domain separation** across the name/proof boundary | The derivation hashes `name ‖ 0x00 ‖ proof`. Without that separator, `("ab","c")` and `("a","bc")` would hash identically — two different networks collapsing onto one identity, so a client could silently dial the wrong host. | Asserts the two cases produce different identities. |
| **Identity key at rest** | The on-device key is encrypted with Windows DPAPI, tied to the user account. | — |
| **Kill-switch + IPv6 leak block** | If the link drops, traffic is blocked rather than falling back to the clear. IPv6 default routes are pushed into the IPv4-only tunnel adapter, where they are dropped. | — (see limitations) |
| **DNS leak hardening** | Disables Windows' "smart multi-homed name resolution", which otherwise queries the local resolver in parallel even while the tunnel is up. The prior policy value is captured and restored exactly on teardown. | — |

Release installers can be Authenticode-signed — see [`docs/SIGNING.md`](docs/SIGNING.md).

---

## Benchmarking

No throughput numbers are published here, because a VPN benchmark needs two machines, a real network
path and Administrator rights — none of which can be faked into a README honestly. What the
repository provides instead is the exact methodology, so the numbers can be reproduced rather than
taken on trust. See [`docs/BENCHMARK.md`](docs/BENCHMARK.md) for the full protocol:

- **Throughput / latency** — `iperf3` through the tunnel versus direct, on LAN and cross-internet,
  reporting Mbps, added RTT, CPU% and packets/sec.
- **MTU and fragmentation** — the path where real VPNs quietly lose half their throughput.
- **NAT traversal success rate** — hole-punch outcome by NAT type (full-cone, restricted,
  port-restricted, symmetric), which decides whether a session runs direct or falls back to a relay.
- **Kill-switch verification** — drop the tunnel mid-transfer and assert with a packet capture that
  zero packets escaped to the physical interface.

This is stated as a gap rather than filled with numbers from a single machine that would not
generalise.

---

## A note on "no server"

On the same network, MyVPN uses zero infrastructure — devices find each other directly. For
cross-internet connections it relies on public, community-run infrastructure you do not operate:
`iroh`'s NAT-traversal relays and public discovery service, used to set up the connection and resolve
a pairing code to its current address. Once established, traffic flows directly between your devices
whenever hole-punching succeeds.

You run no server of your own — but if you want total sovereignty you can point MyVPN at your own
relay in Settings; see [`docs/RELAY.md`](docs/RELAY.md). A relay only assists the connection and
never sees your end-to-end-encrypted traffic.

---

## Good to know

- **One connection at a time.** A host serves a single connected device; another can take over once
  it is free. Multi-client hosting is the main missing feature.
- **Administrator rights.** MyVPN creates a real network adapter and system routes, so it runs
  elevated. The installer and the *Start on boot* task handle that.
- **Resilient by design.** If the link drops, MyVPN holds a kill-switch and transparently reconnects
  in the background.
- **Bring your own exit.** For an exit in a specific country, run a MyVPN host on a VPS there and
  connect to it — your exit, your infrastructure, no third party in the middle.

## Known limitations

Stated here rather than discovered in review:

- **Windows only.** The tunnel is built on Wintun and the network configuration shells out to
  `netsh` / PowerShell. Linux (`tun`) and macOS (`utun`) support would need a platform abstraction
  that does not exist yet.
- **Single client per host.** Per-peer TUN address allocation and a routing table are required for
  multi-client hosting.
- **No published performance numbers.** See *Benchmarking* above.
- **The kill-switch and DNS hardening are not covered by automated tests.** They mutate global
  machine state, so verifying them properly needs an integration harness with a packet capture, not a
  unit test. The pure logic around them is tested; the effects are not.
- **Cryptography is delegated to `iroh`/`rustls`.** The handshake, the proof comparison and the key
  derivation in this repository are tested, but the transport encryption itself is trusted library
  code and has not been independently audited.
- **Installers are unsigned by default.** SmartScreen will warn. Verify the publisher, check the
  SHA-256, or build from source.

---

## Build from source

```bash
# Prerequisites: Node.js 18+, Rust (stable, MSVC toolchain), Tauri prerequisites for Windows.
git clone https://github.com/sayantanmandal1/MyVPN.git
cd MyVPN/app

# Fetch the Wintun driver (one-time; CI bundles it automatically).
powershell -ExecutionPolicy Bypass -File ../scripts/fetch-wintun.ps1

npm install
npm run tauri dev      # run locally (launch elevated for the tunnel)
npm run tauri build    # installer in app/src-tauri/target/release/bundle
```

Run the test suite:

```bash
cd app/src-tauri
cargo test --lib
```

> If `cargo` resolves to a non-rustup installation, the build may target `x86_64-pc-windows-gnu` and
> fail looking for `gcc.exe`. Ensure rustup's `~/.cargo/bin` comes first on `PATH`; this project
> targets MSVC.

To cut a release, tag a version:

```bash
git tag v1.0.1 && git push --tags
```

The [release workflow](.github/workflows/release.yml) downloads the Wintun driver, builds the
installer, and publishes it to a GitHub Release automatically.

## Project structure

| path | purpose |
|------|---------|
| `app/` | The MyVPN desktop application (Tauri + Rust + React) |
| `app/src-tauri/src/vpn/` | Transport, discovery, TUN handling, Windows network configuration |
| `app/src-tauri/src/public_vpn/` | Optional public-server mode (OpenVPN config sourcing) |
| `web/` | Marketing & showcase website (Next.js + shadcn/ui) |
| `.github/workflows` | CI that builds installers and publishes releases |

## License

MIT
