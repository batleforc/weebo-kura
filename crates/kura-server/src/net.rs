//! The client address (RFC 0001 §4.1): forwarding headers are believed only
//! from a peer in `trustedProxies`; otherwise the peer address is the client.
//! And the outbound client for webhooks (RFC 0003 §6.5), which connects only
//! to public addresses or to ranges the operator allowed.

use std::net::IpAddr;

use axum::http::HeaderMap;
use ipnet::IpNet;

/// Walks `X-Forwarded-For` from the right, skipping trusted proxies, and
/// returns the first address no trusted proxy vouches beyond.
pub fn client_addr(peer: IpAddr, headers: &HeaderMap, trusted: &[IpNet]) -> IpAddr {
    let is_trusted = |ip: &IpAddr| trusted.iter().any(|n| n.contains(ip));
    if !is_trusted(&peer) {
        return peer;
    }
    let hops: Vec<IpAddr> = headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(str::trim)
        .map(|s| s.parse::<IpAddr>())
        .collect::<Result<_, _>>()
        .unwrap_or_default();
    let mut client = peer;
    for hop in hops.iter().rev() {
        client = *hop;
        if !is_trusted(hop) {
            break;
        }
    }
    client
}

/// [`client_addr`] for a check that grants access (a scoped token's CIDR
/// list, §6.2): behind a trusted proxy, the client must be named by a
/// well-formed `X-Forwarded-For` and be no trusted proxy itself. Otherwise
/// `None`, refused: a malformed or missing header never makes the proxy's
/// own address the client (review 7 M1).
pub fn client_addr_strict(peer: IpAddr, headers: &HeaderMap, trusted: &[IpNet]) -> Option<IpAddr> {
    let is_trusted = |ip: &IpAddr| trusted.iter().any(|n| n.contains(ip));
    if !is_trusted(&peer) {
        return Some(peer);
    }
    let hops: Vec<IpAddr> = headers
        .get_all("x-forwarded-for")
        .iter()
        .map(|v| v.to_str().ok())
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .flat_map(|v| v.split(','))
        .map(|s| s.trim().parse::<IpAddr>().ok())
        .collect::<Option<_>>()?;
    hops.iter().rev().find(|h| !is_trusted(h)).copied()
}

/// Whether a webhook may connect to `addr` (RFC 0003 §6.5, T1): a range of
/// `allowedCidrs` opens it; otherwise every private or special address is
/// refused, an IPv4-mapped IPv6 address by its IPv4 form.
pub fn webhook_allowed(addr: IpAddr, allowed: &[IpNet]) -> bool {
    let v4 = match addr {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4),
        IpAddr::V4(_) => None,
    };
    let candidates = [Some(addr), v4];
    if candidates
        .iter()
        .flatten()
        .any(|a| allowed.iter().any(|n| n.contains(a)))
    {
        return true;
    }
    !kura_core::webhook::refused_by_default(addr)
}

/// The resolver of the webhook client: it drops every address the guard
/// refuses, at every connection, so a name that pointed outside at creation
/// and now points inside reaches nothing (DNS rebinding, T1).
#[derive(Clone)]
struct GuardedResolver {
    allowed: std::sync::Arc<Vec<IpNet>>,
    lookup: Lookup,
}

/// Where the guarded resolver gets a name's addresses: the system's
/// resolver in the server, a scripted one in the rebinding test.
pub type Lookup = std::sync::Arc<
    dyn Fn(
            String,
        ) -> std::pin::Pin<
            Box<dyn Future<Output = std::io::Result<Vec<std::net::SocketAddr>>> + Send>,
        > + Send
        + Sync,
>;

impl reqwest::dns::Resolve for GuardedResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let allowed = self.allowed.clone();
        let answer = (self.lookup)(name.as_str().to_owned());
        Box::pin(async move {
            let found: Vec<std::net::SocketAddr> = answer
                .await?
                .into_iter()
                .filter(|a| webhook_allowed(a.ip(), &allowed))
                .collect();
            if found.is_empty() {
                return Err(Box::new(AddressRefused) as Box<dyn std::error::Error + Send + Sync>);
            }
            let addrs: reqwest::dns::Addrs = Box::new(found.into_iter());
            Ok(addrs)
        })
    }
}

/// No address of the name may be reached.
#[derive(Debug, thiserror::Error)]
#[error("address_refused")]
pub struct AddressRefused;

/// The webhook client: its own, not Authentik's or Vault's. No redirect, no
/// proxy, the guarded resolver, 3 s to connect and 5 s in all; `ca` adds
/// trust roots to this client only (§6.5).
pub fn webhook_client(
    allowed: Vec<IpNet>,
    ca: &[reqwest::Certificate],
) -> Result<reqwest::Client, reqwest::Error> {
    let system: Lookup = std::sync::Arc::new(|host: String| {
        Box::pin(async move { Ok(tokio::net::lookup_host((host.as_str(), 0)).await?.collect()) })
    });
    webhook_client_resolving(allowed, ca, system)
}

/// [`webhook_client`] with the names answered by `lookup`; every answer
/// still goes through the guard.
pub fn webhook_client_resolving(
    allowed: Vec<IpNet>,
    ca: &[reqwest::Certificate],
    lookup: Lookup,
) -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .https_only(true)
        .dns_resolver(GuardedResolver {
            allowed: std::sync::Arc::new(allowed),
            lookup,
        })
        .connect_timeout(std::time::Duration::from_secs(3))
        .timeout(std::time::Duration::from_secs(5))
        .user_agent(concat!("kura-server/", env!("CARGO_PKG_VERSION")))
        .tls_certs_merge(ca.iter().cloned())
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(xff: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", xff.parse().unwrap());
        h
    }

    #[test]
    fn forwarding_headers_count_only_from_trusted_proxies() {
        let traefik: Vec<IpNet> = vec!["10.42.0.0/16".parse().unwrap()];
        let ip = |s: &str| s.parse::<IpAddr>().unwrap();
        // Untrusted peer: its header is a lie it may tell.
        assert_eq!(
            client_addr(ip("203.0.113.9"), &headers("1.2.3.4"), &traefik),
            ip("203.0.113.9")
        );
        // Trusted peer: the right-most untrusted hop is the client.
        assert_eq!(
            client_addr(ip("10.42.1.5"), &headers("1.2.3.4, 198.51.100.7"), &traefik),
            ip("198.51.100.7")
        );
        assert_eq!(
            client_addr(
                ip("10.42.1.5"),
                &headers("198.51.100.7, 10.42.9.9"),
                &traefik
            ),
            ip("198.51.100.7")
        );
        // No trusted proxies configured: headers are ignored.
        assert_eq!(
            client_addr(ip("10.42.1.5"), &headers("1.2.3.4"), &[]),
            ip("10.42.1.5")
        );
        // A malformed header is ignored, not half-read.
        assert_eq!(
            client_addr(ip("10.42.1.5"), &headers("1.2.3.4, junk"), &traefik),
            ip("10.42.1.5")
        );
    }

    #[test]
    fn the_strict_form_never_takes_the_proxy_for_the_client() {
        let proxy: IpAddr = "10.42.0.5".parse().unwrap();
        let trusted = ["10.42.0.0/16".parse::<IpNet>().unwrap()];
        let strict = |xff: Option<&str>| {
            let h = xff.map(headers).unwrap_or_default();
            client_addr_strict(proxy, &h, &trusted)
        };
        assert_eq!(
            strict(Some("203.0.113.7")),
            Some("203.0.113.7".parse().unwrap())
        );
        assert_eq!(
            strict(Some("203.0.113.7, 10.42.0.9")),
            Some("203.0.113.7".parse().unwrap())
        );
        for bad in [
            None,
            Some("junk"),
            Some("203.0.113.7:443"),
            Some("10.42.0.9"),
            Some("1.2.3.4, junk"),
        ] {
            assert_eq!(strict(bad), None, "{bad:?}");
        }
        let direct: IpAddr = "198.51.100.1".parse().unwrap();
        assert_eq!(
            client_addr_strict(direct, &headers("junk"), &trusted),
            Some(direct)
        );
    }

    #[test]
    fn webhooks_reach_public_addresses_and_allowed_ranges_only() {
        let ip = |s: &str| s.parse::<IpAddr>().unwrap();
        let hub: Vec<IpNet> = vec!["10.42.7.15/32".parse().unwrap()];
        assert!(webhook_allowed(ip("1.1.1.1"), &[]));
        assert!(!webhook_allowed(ip("10.42.7.15"), &[]));
        assert!(webhook_allowed(ip("10.42.7.15"), &hub));
        assert!(
            webhook_allowed(ip("::ffff:10.42.7.15"), &hub),
            "mapped form of an allowed address"
        );
        assert!(!webhook_allowed(ip("10.42.7.16"), &hub));
        assert!(!webhook_allowed(ip("169.254.169.254"), &hub));
        assert!(!webhook_allowed(ip("::ffff:169.254.169.254"), &[]));
    }
}
