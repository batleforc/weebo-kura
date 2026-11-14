//! The client address (RFC 0001 §4.1): forwarding headers are believed only
//! from a peer in `trustedProxies`; otherwise the peer address is the client.

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
}
