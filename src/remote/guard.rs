//! Who may talk to the web remote at all, before any password: only computers and phones on
//! this computer's own networks, only by an address (never a DNS name a web page could point
//! here, DNS rebinding), and changes only from the remote's own page (no cross-site requests).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use super::http::{Method, Request};

/// Whether `peer` is on a local network: this computer, a private range or link-local. The
/// internet is refused even if a router forwards the port.
pub fn is_local_peer(peer: IpAddr) -> bool {
    match peer {
        IpAddr::V4(ip) => is_local_v4(ip),
        IpAddr::V6(ip) => ip.to_ipv4_mapped().map_or_else(|| is_local_v6(ip), is_local_v4),
    }
}

fn is_local_v4(ip: Ipv4Addr) -> bool {
    ip.is_loopback() || ip.is_private() || ip.is_link_local()
}

fn is_local_v6(ip: Ipv6Addr) -> bool {
    let first = ip.segments()[0];
    // Unique local (fc00::/7) and link-local (fe80::/10).
    ip.is_loopback() || first & 0xfe00 == 0xfc00 || first & 0xffc0 == 0xfe80
}

/// Whether the request's `Host` names this computer the way the remote's link does: an IP
/// address, `localhost`, or a `.local` name on the network. A page on another site that made
/// its own name resolve here would send that name, and is refused.
pub fn is_allowed_host(request: &Request) -> bool {
    let Some(host) = request.header("host") else { return false };
    let name = strip_port(host);
    let name = name.strip_prefix('[').and_then(|n| n.strip_suffix(']')).unwrap_or(name);
    let name = name.trim_end_matches('.').to_ascii_lowercase();
    name.parse::<IpAddr>().is_ok() || name == "localhost" || is_mdns_name(&name)
}

fn strip_port(host: &str) -> &str {
    match host.rsplit_once(':') {
        Some((name, port)) if !name.ends_with(':') && port.bytes().all(|b| b.is_ascii_digit()) => {
            name
        }
        _ => host,
    }
}

fn is_mdns_name(name: &str) -> bool {
    name.strip_suffix(".local").is_some_and(|label| {
        !label.is_empty() && label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    })
}

/// Whether a request that changes something comes from the remote's own page: its `Origin` is
/// this server. Browsers send `Origin` with every POST, so another site's form or script is
/// refused, and a request without one isn't a browser's.
pub fn is_same_origin(request: &Request) -> bool {
    if request.method != Method::Post {
        return true;
    }
    let (Some(origin), Some(host)) = (request.header("origin"), request.header("host")) else {
        return false;
    };
    origin.strip_prefix("http://").is_some_and(|origin| origin.eq_ignore_ascii_case(host))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote::http::read_request;

    fn request(head: &str) -> Request {
        read_request(format!("{head}\r\n\r\n").as_bytes()).unwrap()
    }

    #[test]
    fn local_networks_are_let_in_and_the_internet_is_not() {
        for ip in ["127.0.0.1", "192.168.1.20", "10.1.2.3", "172.16.0.9", "169.254.3.4", "::1"] {
            assert!(is_local_peer(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["fd12::1", "fe80::1", "::ffff:192.168.0.2"] {
            assert!(is_local_peer(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["8.8.8.8", "100.64.0.1", "172.32.0.1", "2001:db8::1", "::ffff:1.1.1.1"] {
            assert!(!is_local_peer(ip.parse().unwrap()), "{ip}");
        }
    }

    #[test]
    fn hosts_must_be_addresses_or_local_names() {
        for host in ["192.168.1.5:8080", "localhost:8080", "[::1]:8080", "my-mac.local", "10.0.0.1"]
        {
            assert!(
                is_allowed_host(&request(&format!("GET / HTTP/1.1\r\nHost: {host}"))),
                "{host}"
            );
        }
        for host in ["evil.example.com:8080", "192.168.1.5.nip.io", "a.b.local", ".local"] {
            assert!(
                !is_allowed_host(&request(&format!("GET / HTTP/1.1\r\nHost: {host}"))),
                "{host}"
            );
        }
        assert!(!is_allowed_host(&request("GET / HTTP/1.1")));
    }

    #[test]
    fn posts_must_come_from_the_remote_page() {
        let own = request("POST / HTTP/1.1\r\nHost: 10.0.0.2:8080\r\nOrigin: http://10.0.0.2:8080");
        assert!(is_same_origin(&own));
        let other = request("POST / HTTP/1.1\r\nHost: 10.0.0.2:8080\r\nOrigin: http://evil.com");
        assert!(!is_same_origin(&other));
        assert!(!is_same_origin(&request("POST / HTTP/1.1\r\nHost: 10.0.0.2:8080")));
        assert!(is_same_origin(&request("GET / HTTP/1.1\r\nHost: 10.0.0.2:8080")));
    }
}
