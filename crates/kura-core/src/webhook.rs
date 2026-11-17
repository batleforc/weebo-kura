//! A webhook target, as a tenant admin gives it (RFC 0003 §4.3, §6.5): the
//! parsing and the address rule, with no network, so the fuzz target and
//! the server share them. Whether a name resolves to an allowed address is
//! the server's resolver's question, asked at every attempt.

use std::net::IpAddr;

/// The longest URL accepted.
pub const MAX_URL: usize = 2048;

/// One refusal for every case (§4.5): the form is not a probe of the network.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "this address cannot be used: a webhook must be an https address that is public, or one the platform operator allowed"
)]
pub struct TargetError;

/// A webhook URL that passed every check that needs no network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    url: url::Url,
}

impl Target {
    /// `https`, no user-info, a host, at most 2,048 bytes. A host that is an
    /// IP literal is checked by `literal_allowed` here, since no resolver
    /// will see it.
    pub fn parse(
        text: &str,
        literal_allowed: impl Fn(IpAddr) -> bool,
    ) -> Result<Self, TargetError> {
        if text.len() > MAX_URL {
            return Err(TargetError);
        }
        let url = url::Url::parse(text).map_err(|_| TargetError)?;
        if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
            return Err(TargetError);
        }
        let ok = match url.host() {
            Some(url::Host::Domain(d)) => !d.is_empty(),
            Some(url::Host::Ipv4(ip)) => literal_allowed(IpAddr::V4(ip)),
            Some(url::Host::Ipv6(ip)) => literal_allowed(IpAddr::V6(ip)),
            None => false,
        };
        if !ok || url.as_str().len() > MAX_URL {
            return Err(TargetError);
        }
        Ok(Self { url })
    }

    pub fn as_str(&self) -> &str {
        self.url.as_str()
    }

    /// What is ever shown again of the URL: its host (§4.5).
    pub fn host(&self) -> String {
        self.url.host_str().unwrap_or_default().to_owned()
    }

    /// The host when it is an IP literal, which the resolver never sees.
    pub fn literal(&self) -> Option<IpAddr> {
        match self.url.host()? {
            url::Host::Ipv4(ip) => Some(IpAddr::V4(ip)),
            url::Host::Ipv6(ip) => Some(IpAddr::V6(ip)),
            url::Host::Domain(_) => None,
        }
    }
}

/// Addresses no webhook reaches unless the operator allowed a range
/// covering them (§6.5): loopback, private, shared, link-local (cloud
/// metadata), unspecified, multicast, broadcast, documentation, unique local,
/// and an IPv4-mapped or IPv4-compatible IPv6 address whose IPv4 is refused.
pub fn refused_by_default(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_multicast()
                || v4.is_broadcast()
                || v4.is_documentation()
                || a == 0
                || (a == 100 && (64..128).contains(&b))
                || (a == 192 && b == 0 && v4.octets()[2] == 0)
                || a >= 240
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6
                .to_ipv4_mapped()
                .or_else(|| v6.to_ipv4().filter(|_| v6.segments()[..6] == [0; 6]))
            {
                return refused_by_default(IpAddr::V4(v4));
            }
            let first = v6.segments()[0];
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80
                || first == 0x2001 && v6.segments()[1] == 0x0db8
                || first == 0x0064 && v6.segments()[1] == 0xff9b
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn private_and_special_addresses_are_refused() {
        for refused in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "224.0.0.1",
            "255.255.255.255",
            "192.0.2.1",
            "240.0.0.1",
            "::1",
            "::",
            "fc00::1",
            "fd12::1",
            "fe80::1",
            "ff02::1",
            "::ffff:127.0.0.1",
            "::ffff:169.254.169.254",
            "::127.0.0.1",
            "2001:db8::1",
            "64:ff9b::a9fe:a9fe",
        ] {
            assert!(refused_by_default(ip(refused)), "{refused}");
        }
        for public in [
            "1.1.1.1",
            "93.184.216.34",
            "100.128.0.1",
            "2606:4700::1111",
            "::ffff:1.1.1.1",
        ] {
            assert!(!refused_by_default(ip(public)), "{public}");
        }
    }

    #[test]
    fn a_target_is_https_without_credentials() {
        let allow = |ip: IpAddr| !refused_by_default(ip);
        let ok = Target::parse("https://Hub.Example.org:8443/hooks/kura?x=1", allow).unwrap();
        assert_eq!(ok.host(), "hub.example.org");
        assert_eq!(ok.literal(), None);
        assert_eq!(
            Target::parse("https://1.1.1.1/h", allow).unwrap().literal(),
            Some(ip("1.1.1.1"))
        );
        for bad in [
            "http://hub.example.org/",
            "https://user:pw@hub.example.org/",
            "https://user@hub.example.org/",
            "https://10.0.0.5/hook",
            "https://[::1]/",
            "https://0x7f000001/",
            "https://2130706433/",
            "file:///etc/passwd",
            "https://",
            "not a url",
        ] {
            assert_eq!(Target::parse(bad, allow), Err(TargetError), "{bad}");
        }
        assert!(
            Target::parse(&format!("https://h.example/{}", "a".repeat(MAX_URL)), allow).is_err()
        );
        // An operator's range opens a literal.
        assert!(Target::parse("https://127.0.0.1:8443/hook", |_| true).is_ok());
    }
}
