//! Public VPN server-list providers.
//!
//! Each *source* yields a list of free public VPN servers together with the
//! OpenVPN config needed to connect to each one. [`fetch_all`] merges every
//! source so adding a new provider never touches the manager or the UI.
//!
//! The first implemented source is **VPN Gate** (an academic project at the
//! University of Tsukuba) which publishes hundreds of volunteer-run servers
//! across ~90 countries, each with an embedded, credential-free OpenVPN config.

use std::time::Duration;

use base64::Engine;

use crate::state::PublicServer;

/// A fetched server plus its decoded OpenVPN config text.
pub struct FetchedServer {
    pub server: PublicServer,
    pub ovpn: String,
}

/// Fetch and merge servers from every configured source. Returns a descriptive
/// error when nothing could be loaded, so the UI can explain *why* the list is
/// empty instead of silently showing nothing.
pub async fn fetch_all() -> anyhow::Result<Vec<FetchedServer>> {
    let mut out = Vec::new();
    let mut errors = Vec::new();

    // Sources are independent; add more here as they are implemented.
    match fetch_vpngate().await {
        Ok(mut v) => {
            tracing::info!("vpngate: {} servers", v.len());
            out.append(&mut v);
        }
        Err(e) => {
            tracing::warn!("vpngate source failed: {e}");
            errors.push(format!("VPN Gate: {e}"));
        }
    }

    if out.is_empty() {
        anyhow::bail!(
            "Could not load public servers. {}",
            if errors.is_empty() {
                "Please try again shortly.".to_string()
            } else {
                errors.join("; ")
            }
        );
    }
    Ok(out)
}

/// Server-list sources, tried in order. Our own GitHub-hosted mirror comes
/// first: VPN Gate's website blocks automated clients (a WAF returns 403 on
/// most networks), but GitHub raw content is never blocked and is refreshed
/// hourly by `.github/workflows/servers.yml`. The direct VPN Gate API is kept
/// as a fallback for the rare networks where it is reachable.
const VPNGATE_SOURCES: &[&str] = &[
    "https://raw.githubusercontent.com/sayantanmandal1/MyVPN/vpn-data/servers.csv",
    "https://www.vpngate.net/api/iphone/",
    "http://www.vpngate.net/api/iphone/",
];

fn http_client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        // A browser-like UA avoids naive WAF/bot blocks; the network can still
        // block VPN Gate entirely (many ISPs/firewalls do), which surfaces as a
        // clear error in the UI.
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
             (KHTML, like Gecko) MyVPN/1.0",
        )
        .timeout(Duration::from_secs(45))
        .build()
}

async fn fetch_vpngate() -> anyhow::Result<Vec<FetchedServer>> {
    let client = http_client()?;
    let mut last_err: Option<anyhow::Error> = None;
    for url in VPNGATE_SOURCES {
        match client.get(*url).send().await.and_then(|r| r.error_for_status()) {
            Ok(resp) => match resp.text().await {
                Ok(body) => {
                    let servers = parse_vpngate(&body);
                    if !servers.is_empty() {
                        return Ok(servers);
                    }
                    last_err = Some(anyhow::anyhow!("server list was empty"));
                }
                Err(e) => last_err = Some(e.into()),
            },
            Err(e) => last_err = Some(e.into()),
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("no reachable endpoint")))
}

/// Parse the VPN Gate CSV API response into servers.
///
/// The payload is a CSV with a `*vpn_servers` preamble line, a `#`-prefixed
/// header, data rows, and a trailing `*` line. The structured fields we use are
/// the first eight columns (all numeric/short, never containing commas); the
/// base64 OpenVPN config is always the **last** column (base64 also never
/// contains commas), so free-text columns in between cannot misalign parsing.
pub fn parse_vpngate(body: &str) -> Vec<FetchedServer> {
    let mut out = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('*') || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split(',').collect();
        if cols.len() < 15 {
            continue;
        }

        let hostname = cols[0].trim();
        let ip = cols[1].trim();
        let score = cols[2].trim().parse::<u64>().ok();
        let ping = cols[3].trim().parse::<u32>().ok().filter(|p| *p > 0);
        let speed_bps = cols[4].trim().parse::<f64>().ok();
        let country = cols[5].trim();
        let country_code = cols[6].trim();
        let sessions = cols[7].trim().parse::<u32>().ok();
        let ovpn_b64 = cols[cols.len() - 1].trim();

        if hostname.is_empty()
            || ip.is_empty()
            || country.is_empty()
            || ovpn_b64.is_empty()
        {
            continue;
        }

        let ovpn = match base64::engine::general_purpose::STANDARD.decode(ovpn_b64) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(s) if s.contains("remote ") => s,
                _ => continue,
            },
            Err(_) => continue,
        };

        // VPN Gate reports speed in bits/second; show one-decimal megabits.
        let speed_mbps = speed_bps
            .filter(|b| *b > 0.0)
            .map(|b| (b / 1_000_000.0 * 10.0).round() / 10.0);

        out.push(FetchedServer {
            server: PublicServer {
                id: format!("vpngate:{hostname}:{ip}"),
                source: "vpngate".to_string(),
                country: country.to_string(),
                country_code: country_code.to_uppercase(),
                hostname: hostname.to_string(),
                ip: ip.to_string(),
                ping_ms: ping,
                speed_mbps,
                sessions,
                score,
            },
            ovpn,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rows_and_ignores_preamble() {
        // A base64 config that decodes to text containing "remote ".
        let cfg = base64::engine::general_purpose::STANDARD.encode("client\nremote 1.2.3.4 443\n");
        let body = format!(
            "*vpn_servers\n#HostName,IP,Score,Ping,Speed,CountryLong,CountryShort,NumVpnSessions,Uptime,TotalUsers,TotalTraffic,LogType,Operator,Message,OpenVPN_ConfigData_Base64\n\
             host1,1.2.3.4,100,42,12000000,Japan,JP,7,1,1,1,2,op,msg,{cfg}\n*\n"
        );
        let servers = parse_vpngate(&body);
        assert_eq!(servers.len(), 1);
        let s = &servers[0].server;
        assert_eq!(s.country, "Japan");
        assert_eq!(s.country_code, "JP");
        assert_eq!(s.ping_ms, Some(42));
        assert_eq!(s.sessions, Some(7));
        assert_eq!(s.speed_mbps, Some(12.0));
        assert!(servers[0].ovpn.contains("remote "));
    }

    #[test]
    fn tolerates_commas_in_free_text_columns() {
        let cfg = base64::engine::general_purpose::STANDARD.encode("client\nremote 5.6.7.8 1194\n");
        // The Message column contains commas; the config must still be the last field.
        let body = format!(
            "host2,5.6.7.8,50,10,8000000,United States,US,3,1,1,1,2,op,\"hello, world, x\",{cfg}\n"
        );
        let servers = parse_vpngate(&body);
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].server.country_code, "US");
        assert!(servers[0].ovpn.contains("remote "));
    }
}
