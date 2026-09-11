# Benchmarking MyVPN

This document is the protocol, not a results table. A VPN benchmark needs two machines, a real
network path and Administrator rights, so numbers produced on one laptop would not generalise.
Publishing the method means anyone can produce numbers that are actually comparable.

Record the following with every run, or the numbers mean nothing:

- Host and client CPU, RAM and OS build
- Physical link on both ends (Ethernet / Wi-Fi generation, ISP uplink and downlink)
- Whether the session is **direct** (hole-punched) or **relayed** — check the connection type in the
  app before timing anything
- MyVPN version and `iroh` version

---

## 1. Throughput and latency

Baseline first, always. A tunnel measured without a direct-path baseline tells you nothing.

```bash
# On the host
iperf3 -s

# On the client, DIRECT (no tunnel) — the baseline
iperf3 -c <host-lan-ip> -t 30 -P 4

# On the client, THROUGH the tunnel
iperf3 -c <host-tunnel-ip> -t 30 -P 4

# Reverse direction (download rather than upload)
iperf3 -c <host-tunnel-ip> -t 30 -P 4 -R

# UDP, to separate congestion-control effects from raw forwarding cost
iperf3 -c <host-tunnel-ip> -u -b 200M -t 30
```

Report, for LAN and cross-internet separately:

| metric | direct | tunnelled | overhead |
|--------|--------|-----------|----------|
| TCP throughput up (Mbps) | | | |
| TCP throughput down (Mbps) | | | |
| UDP throughput (Mbps) @ loss % | | | |
| RTT p50 / p95 (ms, `ping -n 100`) | | | |
| Host CPU % during transfer | | | |
| Client CPU % during transfer | | | |

Take the median of three runs. Note whether either machine was thermally throttling.

---

## 2. MTU and fragmentation

This is where real VPNs quietly lose throughput. Encapsulation shrinks the usable payload; if the
tunnel MTU is wrong, every large packet fragments or is silently dropped by path-MTU discovery.

Find the largest payload that traverses the tunnel without fragmenting:

```bash
# Windows: -f sets don't-fragment, -l sets payload size. Binary-search the size.
ping -f -l 1400 <host-tunnel-ip>
ping -f -l 1372 <host-tunnel-ip>
```

Report the largest successful payload, the MTU MyVPN configured on the adapter, and the throughput
delta between the configured MTU and the empirically optimal one. A mismatch of a few dozen bytes is
routinely worth tens of percent.

---

## 3. NAT traversal success rate

This determines whether a session runs peer-to-peer or falls back through a relay, which is the
single biggest factor in both latency and privacy posture. It is also the number almost nobody
publishes.

Classify each endpoint's NAT, then attempt connections across every combination:

| host NAT | client NAT | attempts | direct | relayed | failed | median time to connect |
|----------|-----------|---------:|-------:|--------:|-------:|-----------------------:|
| full-cone | full-cone | | | | | |
| full-cone | restricted | | | | | |
| restricted | port-restricted | | | | | |
| port-restricted | port-restricted | | | | | |
| symmetric | full-cone | | | | | |
| symmetric | symmetric | | | | | |

Symmetric-to-symmetric is expected to fall back to a relay; it is the case hole-punching cannot
generally solve. Run at least 20 attempts per cell — hole-punching is probabilistic, and a single
success proves nothing.

---

## 4. Kill-switch verification

The kill-switch claim is binary and must be demonstrated, not asserted.

1. Start a continuous transfer through the tunnel.
2. Begin a packet capture on the **physical** adapter, filtered to the destination:
   `tshark -i <physical-adapter> -f "host <destination-ip>"`
3. Force the tunnel down (kill the host process, or disable the host's network adapter).
4. Keep capturing for 30 seconds.

**Pass condition: zero packets to the destination appear on the physical adapter.** Any packet at all
is a leak, regardless of how briefly the window was open. Repeat 10 times; leaks are often a race
that only appears occasionally.

Repeat the same procedure for DNS specifically, capturing UDP/53 and TCP/853 on the physical adapter
while resolving names during a tunnel drop.

---

## 5. Reconnection behaviour

| scenario | expected | measure |
|----------|----------|---------|
| Host restarts | client reconnects automatically | time to re-establish |
| Client changes network (Wi-Fi → Ethernet) | tunnel migrates or re-establishes | time, and whether the kill-switch held during the gap |
| Host IP changes (DHCP renew) | discovery re-resolves the pairing code | time to re-establish |
| Sustained 24 h session | no leak, no memory growth | RSS at start vs end, packet capture spot-checks |

---

## Reporting

Publish the full table including the cases that failed. A benchmark that only lists the favourable
configurations is marketing, not measurement.
