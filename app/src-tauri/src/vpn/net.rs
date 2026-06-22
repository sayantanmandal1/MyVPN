//! Windows networking configuration helpers (routing, NAT, DNS, forwarding).
//!
//! These shell out to built-in Windows tools (`netsh` and the `NetTCPIP` /
//! `NetNat` PowerShell modules). Every mutating call has a matching revert so
//! the system is left clean when a tunnel is torn down. All of these require
//! the process to run elevated (Administrator).

use std::net::IpAddr;
use std::process::Command;
use std::str::FromStr;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

use crate::error::{Result, VpnError};

/// Reject anything that isn't a bare IP literal before it reaches a shell
/// command. All callers pass system-derived IPs; this is defense-in-depth
/// against any malformed value ever flowing into a PowerShell string.
fn ensure_ip(value: &str) -> Result<()> {
    IpAddr::from_str(value.trim())
        .map(|_| ())
        .map_err(|_| VpnError::msg(format!("refusing non-IP value: {value}")))
}

fn run(program: &str, args: &[&str]) -> Result<()> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let output = cmd
        .output()
        .map_err(|e| VpnError::msg(format!("failed to launch {program}: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(VpnError::msg(format!(
            "{program} {args:?} failed: {} {}",
            stdout.trim(),
            stderr.trim()
        )));
    }
    Ok(())
}

fn pwsh(script: &str) -> Result<()> {
    run(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-Command", script],
    )
}

fn pwsh_out(script: &str) -> Result<String> {
    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", script]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let output = cmd
        .output()
        .map_err(|e| VpnError::msg(format!("failed to launch powershell: {e}")))?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Assign a static IPv4 address to the tunnel interface.
pub fn set_interface_ipv4(alias: &str, ip: &str, prefix: u8) -> Result<()> {
    ensure_ip(ip)?;
    // Clear any stale address first, then assign (one shell round-trip).
    pwsh(&format!(
        "Remove-NetIPAddress -InterfaceAlias '{alias}' -Confirm:$false -ErrorAction SilentlyContinue; \
         New-NetIPAddress -InterfaceAlias '{alias}' -IPAddress {ip} -PrefixLength {prefix} -ErrorAction Stop | Out-Null"
    ))
}

/// Set the MTU of the tunnel interface.
pub fn set_mtu(alias: &str, mtu: u32) -> Result<()> {
    run(
        "netsh",
        &[
            "interface",
            "ipv4",
            "set",
            "subinterface",
            alias,
            &format!("mtu={mtu}"),
            "store=active",
        ],
    )
}

/// Restore automatic (DHCP) DNS on the tunnel interface.
pub fn reset_dns(alias: &str) {
    let _ = pwsh(&format!(
        "Set-DnsClientServerAddress -InterfaceAlias '{alias}' -ResetServerAddresses -ErrorAction SilentlyContinue"
    ));
}

// The Windows policy that disables "smart multi-homed name resolution" (parallel
// DNS queries across every interface), which would otherwise leak lookups to the
// local network's resolver even while the tunnel is up.
const DNS_POLICY_KEY: &str = r"HKLM:\SOFTWARE\Policies\Microsoft\Windows NT\DNSClient";
const DNS_POLICY_VAL: &str = "DisableSmartNameResolution";

/// Harden client DNS against leaks while the full tunnel is up, in one shot:
/// point the tunnel interface at the tunnel resolver, make it the preferred
/// interface, and disable Windows' parallel multi-homed name resolution so no
/// lookup escapes to the local network. Returns the prior policy value (None if
/// it was unset) so teardown can restore it exactly.
pub fn harden_client_dns(alias: &str, dns: &str) -> Option<u32> {
    if ensure_ip(dns).is_err() {
        return None;
    }
    let prior = pwsh_out(&format!(
        "(Get-ItemProperty -Path '{DNS_POLICY_KEY}' -Name '{DNS_POLICY_VAL}' -ErrorAction SilentlyContinue).{DNS_POLICY_VAL}"
    ))
    .ok()
    .and_then(|s| s.trim().parse::<u32>().ok());
    let _ = pwsh(&format!(
        "Set-DnsClientServerAddress -InterfaceAlias '{alias}' -ServerAddresses {dns} -ErrorAction SilentlyContinue; \
         Set-NetIPInterface -InterfaceAlias '{alias}' -InterfaceMetric 1 -ErrorAction SilentlyContinue; \
         if (-not (Test-Path '{DNS_POLICY_KEY}')) {{ New-Item -Path '{DNS_POLICY_KEY}' -Force | Out-Null }}; \
         Set-ItemProperty -Path '{DNS_POLICY_KEY}' -Name '{DNS_POLICY_VAL}' -Value 1 -Type DWord -ErrorAction SilentlyContinue"
    ));
    prior
}

/// Reverse [`harden_client_dns`]: reset the tunnel interface DNS and restore the
/// prior multi-homed resolution policy (deleting our value if it was unset).
pub fn restore_client_dns(alias: &str, prior: Option<u32>) {
    let restore_policy = match prior {
        Some(v) => format!(
            "Set-ItemProperty -Path '{DNS_POLICY_KEY}' -Name '{DNS_POLICY_VAL}' -Value {v} -Type DWord -ErrorAction SilentlyContinue"
        ),
        None => format!(
            "Remove-ItemProperty -Path '{DNS_POLICY_KEY}' -Name '{DNS_POLICY_VAL}' -ErrorAction SilentlyContinue"
        ),
    };
    let _ = pwsh(&format!(
        "Set-DnsClientServerAddress -InterfaceAlias '{alias}' -ResetServerAddresses -ErrorAction SilentlyContinue; {restore_policy}"
    ));
}

/// The current physical default gateway as `(next_hop_ip, interface_index)`.
///
/// Captured *before* installing tunnel routes so the tunnel's own carrier
/// traffic can be pinned to the real gateway (NAT loop-avoidance).
pub fn default_gateway() -> Option<(String, u32)> {
    let out = pwsh_out(
        "Get-NetRoute -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue | \
         Sort-Object RouteMetric | Select-Object -First 1 | \
         ForEach-Object { \"$($_.NextHop) $($_.ifIndex)\" }",
    )
    .ok()?;
    let mut parts = out.split_whitespace();
    let ip = parts.next()?.to_string();
    let idx: u32 = parts.next()?.parse().ok()?;
    if ip.is_empty() || ip == "0.0.0.0" {
        return None;
    }
    Some((ip, idx))
}

/// Install split-default routes that send all traffic into the tunnel.
///
/// Using `0.0.0.0/1` + `128.0.0.0/1` overrides the system default route
/// without deleting it, which makes teardown clean and reliable.
pub fn add_full_tunnel_routes(alias: &str, gateway: &str) -> Result<()> {
    ensure_ip(gateway)?;
    pwsh(&format!(
        "$ErrorActionPreference='Stop'; \
         foreach ($p in '0.0.0.0/1','128.0.0.0/1') {{ \
           Remove-NetRoute -DestinationPrefix $p -InterfaceAlias '{alias}' -Confirm:$false -ErrorAction SilentlyContinue; \
           New-NetRoute -DestinationPrefix $p -InterfaceAlias '{alias}' -NextHop {gateway} -RouteMetric 1 | Out-Null \
         }}"
    ))
}

/// Remove the split-default tunnel routes.
pub fn remove_full_tunnel_routes(alias: &str) {
    let _ = pwsh(&format!(
        "foreach ($p in '0.0.0.0/1','128.0.0.0/1') {{ \
           Remove-NetRoute -DestinationPrefix $p -InterfaceAlias '{alias}' -Confirm:$false -ErrorAction SilentlyContinue \
         }}"
    ));
}

/// Pin a single carrier address to the physical gateway so the tunnel's own
/// QUIC packets bypass the tunnel (prevents a routing loop).
pub fn add_bypass_route(ip: &str, gateway: &str, if_index: u32) -> Result<()> {
    ensure_ip(ip)?;
    ensure_ip(gateway)?;
    let _ = pwsh(&format!(
        "Remove-NetRoute -DestinationPrefix '{ip}/32' -Confirm:$false -ErrorAction SilentlyContinue"
    ));
    pwsh(&format!(
        "New-NetRoute -DestinationPrefix '{ip}/32' -InterfaceIndex {if_index} -NextHop {gateway} -RouteMetric 1 -ErrorAction Stop | Out-Null"
    ))
}

/// Remove a previously added carrier bypass route.
pub fn remove_bypass_route(ip: &str) {
    let _ = pwsh(&format!(
        "Remove-NetRoute -DestinationPrefix '{ip}/32' -Confirm:$false -ErrorAction SilentlyContinue"
    ));
}

/// Enable IP forwarding on an interface (host gateway side).
pub fn enable_forwarding(alias: &str) -> Result<()> {
    pwsh(&format!(
        "Set-NetIPInterface -InterfaceAlias '{alias}' -Forwarding Enabled -ErrorAction Stop"
    ))
}

/// Create a NAT so client traffic is masqueraded out of the host's connection.
pub fn create_nat(name: &str, subnet: &str) -> Result<()> {
    let _ = pwsh(&format!(
        "Remove-NetNat -Name '{name}' -Confirm:$false -ErrorAction SilentlyContinue"
    ));
    pwsh(&format!(
        "New-NetNat -Name '{name}' -InternalIPInterfaceAddressPrefix '{subnet}' -ErrorAction Stop | Out-Null"
    ))
}

/// Remove the host NAT.
pub fn remove_nat(name: &str) {
    let _ = pwsh(&format!(
        "Remove-NetNat -Name '{name}' -Confirm:$false -ErrorAction SilentlyContinue"
    ));
}

/// Block all IPv6 while the IPv4 tunnel is up by routing the IPv6 default into
/// the (IPv4-only) tunnel adapter, where it is dropped. Prevents IPv6 leaks.
/// Best-effort: failures are non-fatal. `alias` is a trusted constant.
pub fn add_ipv6_killswitch(alias: &str) {
    let _ = pwsh(&format!(
        "foreach ($p in '::/1','8000::/1') {{ \
           Remove-NetRoute -DestinationPrefix $p -InterfaceAlias '{alias}' -Confirm:$false -ErrorAction SilentlyContinue; \
           New-NetRoute -DestinationPrefix $p -InterfaceAlias '{alias}' -RouteMetric 1 -ErrorAction SilentlyContinue | Out-Null \
         }}"
    ));
}

/// Remove the IPv6 leak kill-switch routes.
pub fn remove_ipv6_killswitch(alias: &str) {
    let _ = pwsh(&format!(
        "foreach ($p in '::/1','8000::/1') {{ \
           Remove-NetRoute -DestinationPrefix $p -InterfaceAlias '{alias}' -Confirm:$false -ErrorAction SilentlyContinue \
         }}"
    ));
}
