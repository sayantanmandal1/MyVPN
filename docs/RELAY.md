# Self-hosting a relay

MyVPN connects your devices peer-to-peer. To set up that connection (and to keep
it working when both devices are behind strict NATs), it uses **relay servers**.
By default MyVPN uses the public, multi-region relays operated by
[number 0](https://n0.computer/) — you don't have to run anything. This guide is
for people who want to run their **own** relay for maximum sovereignty or
reliability.

## What a relay does — and what it does *not* do

A relay is **not** a VPN exit server. It only:

- helps two devices **discover each other's addresses** and **hole-punch** through
  NATs, and
- forwards **end-to-end-encrypted** QUIC packets **only as a fallback** when a
  direct peer-to-peer path can't be established.

A relay **never** sees your unencrypted traffic (it's TLS 1.3 end-to-end between
your devices), and it **does not change where your traffic exits** — that is
always your **host** device. Running a relay in another country does *not* make
your traffic appear to come from there. (For that, see
[Want an exit in another country?](#want-an-exit-in-another-country) below.)

## Quick relay (LAN / testing)

On the machine that will run the relay:

```bash
cargo install iroh-relay --features server
iroh-relay --dev
```

`--dev` runs a plain-HTTP relay on port **3340**. In MyVPN, open
**Settings → Connection relay** on both devices and set:

```
http://<relay-machine-ip>:3340
```

> Plain HTTP is fine for a trusted LAN. For anything over the internet, use the
> TLS setup below.

## Production relay (TLS, public)

You need a domain name pointing at the server and ports **443** (HTTPS) plus the
relay's QUIC port (shown in the relay's startup logs) open in the firewall.

Create `relay.toml`:

```toml
# Lock the relay down so only your own devices may use it.
access.shared_token = ["<REPLACE_ME_long_random_token>"]

[tls]
cert_mode = "LetsEncrypt"
hostname  = "relay.example.com"
contact   = "you@example.com"
```

Run it:

```bash
iroh-relay --config-path relay.toml
```

Then point MyVPN (**Settings → Connection relay**) at:

```
https://relay.example.com
```

If you set `access.shared_token`, append the token as a query parameter:
`https://relay.example.com?token=<your-token>`. You can also keep the token out
of the config file by setting the `IROH_RELAY_ACCESS_TOKEN` environment variable
on the server.

### Locking it down

By default a relay admits everyone. For a private relay, restrict access with one
of:

- `access.shared_token = ["token-a", "token-b"]` — only clients presenting a token.
- `access.allowlist = ["<endpoint-id>", ...]` — only specific device identities
  (each device's pairing code is its endpoint id).

## Want an exit in another country?

Because MyVPN is peer-to-peer, **your traffic always exits through the host you
connect to** — there is no list of third-party "country servers", and we
deliberately do **not** route your traffic through unknown public/free VPN
servers (they are a well-known source of logging, injection, and traffic
interception).

The secure, fully-private way to get an exit in, say, Germany is to run a MyVPN
**host on a small VPS located in Germany** that *you* control, then connect to it
from your laptop. Your exit IP is then that VPS, on infrastructure you own — with
the same end-to-end encryption and no third party in the middle. Spin up as many
regional hosts as you like; each is just another MyVPN host you pair with.
