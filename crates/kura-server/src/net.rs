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
}
