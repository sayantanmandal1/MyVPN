# Public VPN mode

Alongside the private peer-to-peer VPN, MyVPN includes a separate **Public VPN**
mode (sidebar → **Public VPN → Free servers**) for connecting through free,
volunteer-run public servers in other countries — handy for changing your
apparent region or casual browsing.

The two modes are deliberately independent: different transport, separate UI,
and **mutually exclusive at runtime** (both need the default route, so only one
can be connected at a time — the app enforces this).

## Trust & safety

> Public servers are operated by **volunteers**, aggregated from open sources.
> They are great for geo-unblocking and casual use, but whoever runs a server
> can technically observe traffic that isn't already encrypted (HTTPS). **Avoid
> sensitive logins** over a public server. For full privacy, use the **Private
> VPN** with a host you control. The app shows this notice in the Public tab.

## How it works

- **Server lists** are aggregated by a multi-source layer
  ([`sources.rs`](../app/src-tauri/src/public_vpn/sources.rs)). The implemented
  source is **VPN Gate** (an academic project at the University of Tsukuba),
  which publishes hundreds of servers across ~90 countries, each with an
  embedded, credential-free OpenVPN config. New sources can be added by
  implementing another fetcher and appending it in `fetch_all`.
- **Connecting** launches the bundled **OpenVPN** client against the chosen
  server's config and watches its localhost *management interface* for live
  status; disconnecting asks OpenVPN to exit cleanly so it removes its own
  routes/DNS.
- Server configs are kept **server-side** (keyed by id) and never sent to the
  UI; the UI only ever sees country/host/latency metadata.

## The bundled OpenVPN client

The release installer bundles the OpenVPN community client so the feature is
turnkey. CI stages it with
[`scripts/fetch-openvpn.ps1`](../scripts/fetch-openvpn.ps1) (which installs the
official OpenVPN build via Chocolatey and copies `openvpn.exe` + its DLLs +
`wintun.dll` into `app/src-tauri/openvpn/`, bundled via the `openvpn/*` Tauri
resource). At runtime the app prefers the bundled client and falls back to a
system-wide OpenVPN install.

> **Licensing.** OpenVPN is GPLv2. Bundling it alongside this MIT app is
> permitted as an aggregate; the corresponding source is available from
> <https://openvpn.net/community-downloads/>.

If you build locally without running the fetch script, the `openvpn/` folder
contains only a placeholder, so the Public mode will look for a system OpenVPN
install and otherwise show a clear "OpenVPN client not found" message.

## Want a guaranteed-private exit in a specific country?

Use the **Private VPN** instead: run a MyVPN host on a small VPS in that country
(infrastructure you control) and connect to it. See
[`docs/RELAY.md`](RELAY.md#want-an-exit-in-another-country).

## Troubleshooting

- **"OpenVPN client not found"** — the bundled client wasn't staged and no
  system OpenVPN is installed. Install OpenVPN, or build with the fetch script.
- **Empty server list** — the source fetch failed (no internet, or a corporate
  proxy blocking it). Use **Refresh** to retry.
- **Connects then drops** — some volunteer servers are flaky or rate-limited;
  pick another server (there are usually several per country).
