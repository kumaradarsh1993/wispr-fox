//! DNS-over-HTTPS resolution for the Supabase backend.
//!
//! Since 24 Feb 2026 several Indian ISPs (Jio, Airtel, ACT Fibernet) answer
//! DNS queries for `*.supabase.co` with a sinkhole address under a blocking
//! order. The system resolver happily returns that address, the TCP connect
//! succeeds, and the TLS handshake is then dropped — surfacing to the user as
//! `tls handshake eof` on sign-in. The browser half of Google sign-in still
//! works because browsers use their own encrypted DNS.
//!
//! Fix: for Supabase hosts only, resolve through Cloudflare / Google DoH
//! (contacted by IP, so the lookup itself needs no DNS), and fall back to the
//! system resolver only if every DoH endpoint fails. TLS still verifies the
//! real `*.supabase.co` certificate, so a wrong answer can never be trusted.
//! Every other host keeps using the system resolver unchanged.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};

/// JSON DoH endpoints, addressed by IP. Both certificates carry IP SANs.
const DOH_ENDPOINTS: [&str; 3] = [
    "https://1.1.1.1/dns-query",
    "https://1.0.0.1/dns-query",
    "https://8.8.8.8/resolve",
];

/// Hosts routed through DoH. Suffix match, so project subdomains are covered.
const DOH_SUFFIXES: [&str; 1] = ["supabase.co"];

const MIN_TTL: u64 = 60;
const MAX_TTL: u64 = 3600;

pub fn wants_doh(host: &str) -> bool {
    let h = host.trim_end_matches('.').to_ascii_lowercase();
    DOH_SUFFIXES
        .iter()
        .any(|s| h == *s || h.ends_with(&format!(".{s}")))
}

/// Parse a `application/dns-json` answer into IPv4/IPv6 addresses plus the
/// smallest TTL seen. CNAME records are skipped; the resolver follows them.
pub fn parse_doh_json(body: &str) -> Option<(Vec<IpAddr>, u64)> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    if v.get("Status").and_then(|s| s.as_u64()) != Some(0) {
        return None;
    }
    let mut ips = Vec::new();
    let mut ttl = MAX_TTL;
    for a in v.get("Answer")?.as_array()? {
        let t = a.get("type").and_then(|t| t.as_u64()).unwrap_or(0);
        if t != 1 && t != 28 {
            continue;
        }
        if let Some(ip) = a
            .get("data")
            .and_then(|d| d.as_str())
            .and_then(|d| d.parse::<IpAddr>().ok())
        {
            ips.push(ip);
            if let Some(x) = a.get("TTL").and_then(|x| x.as_u64()) {
                ttl = ttl.min(x);
            }
        }
    }
    if ips.is_empty() {
        None
    } else {
        Some((ips, ttl.clamp(MIN_TTL, MAX_TTL)))
    }
}

#[derive(Default)]
pub struct DohResolver {
    cache: Mutex<HashMap<String, (Vec<IpAddr>, Instant)>>,
}

fn doh_client() -> reqwest::Client {
    static C: OnceLock<reqwest::Client> = OnceLock::new();
    C.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .connect_timeout(Duration::from_secs(3))
            .build()
            .expect("reqwest client construction is infallible with default config")
    })
    .clone()
}

async fn doh_lookup(host: &str) -> Option<(Vec<IpAddr>, u64)> {
    let client = doh_client();
    for ep in DOH_ENDPOINTS {
        // A first, AAAA only if A is empty — the sinkhole problem is the
        // same for both, and most networks here are IPv4.
        for rtype in ["A", "AAAA"] {
            let resp = client
                .get(ep)
                .query(&[("name", host), ("type", rtype)])
                .header("accept", "application/dns-json")
                .send()
                .await;
            let Ok(resp) = resp else { break };
            let Ok(body) = resp.text().await else { break };
            if let Some(hit) = parse_doh_json(&body) {
                return Some(hit);
            }
        }
    }
    None
}

impl Resolve for DohResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_string();
        let cached = {
            let c = self.cache.lock();
            c.get(&host)
                .filter(|(_, exp)| *exp > Instant::now())
                .map(|(ips, _)| ips.clone())
        };
        Box::pin(async move {
            let ips = if let Some(ips) = cached {
                Some(ips)
            } else if wants_doh(&host) {
                match doh_lookup(&host).await {
                    Some((ips, ttl)) => {
                        shared().cache.lock().insert(
                            host.clone(),
                            (ips.clone(), Instant::now() + Duration::from_secs(ttl)),
                        );
                        Some(ips)
                    }
                    None => {
                        tracing::warn!("DoH lookup failed for {host}; falling back to system DNS");
                        None
                    }
                }
            } else {
                None
            };
            let addrs: Vec<SocketAddr> = match ips {
                Some(ips) => ips.into_iter().map(|ip| SocketAddr::new(ip, 0)).collect(),
                None => {
                    let h = host.clone();
                    tokio::task::spawn_blocking(move || {
                        (h.as_str(), 0u16).to_socket_addrs().map(|i| i.collect::<Vec<_>>())
                    })
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
                }
            };
            Ok(Box::new(addrs.into_iter()) as Addrs)
        })
    }
}

/// Single shared resolver, so every Supabase client shares one cache. The
/// `'static` resolve future writes to the cache through this, not `self`.
pub fn shared() -> Arc<DohResolver> {
    static R: OnceLock<Arc<DohResolver>> = OnceLock::new();
    R.get_or_init(|| Arc::new(DohResolver::default())).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_only_supabase_hosts() {
        assert!(wants_doh("hvaljemiwuhnohrndyyh.supabase.co"));
        assert!(wants_doh("SUPABASE.CO."));
        assert!(!wants_doh("api.groq.com"));
        assert!(!wants_doh("notsupabase.co"));
    }

    #[test]
    fn parses_cloudflare_answer_skipping_cname() {
        let body = r#"{"Status":0,"Answer":[
            {"name":"x.supabase.co","type":5,"TTL":300,"data":"x.cdn."},
            {"name":"x.cdn","type":1,"TTL":120,"data":"104.18.38.10"},
            {"name":"x.cdn","type":1,"TTL":30,"data":"172.64.149.246"}]}"#;
        let (ips, ttl) = parse_doh_json(body).unwrap();
        assert_eq!(ips.len(), 2);
        assert_eq!(ttl, MIN_TTL); // 30 clamped up
    }

    #[test]
    fn rejects_nxdomain_and_empty() {
        assert!(parse_doh_json(r#"{"Status":3}"#).is_none());
        assert!(parse_doh_json(r#"{"Status":0,"Answer":[]}"#).is_none());
        assert!(parse_doh_json("garbage").is_none());
    }

    /// Network test: `cargo test --lib sync::dns -- --ignored`.
    #[test]
    #[ignore]
    fn live_supabase_through_doh() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let (ips, _) = doh_lookup("hvaljemiwuhnohrndyyh.supabase.co").await.expect("doh");
            assert!(!ips.is_empty());
            let c = reqwest::Client::builder().dns_resolver(shared()).build().unwrap();
            let r = c
                .get(format!("{}/auth/v1/health", super::super::config::SUPABASE_URL))
                .send()
                .await
                .expect("request through DoH resolver");
            assert!(r.status().as_u16() < 500);
        });
    }
}
