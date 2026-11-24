//! What this computer offers a stream: whether a port is free to listen on, and its own address
//! on its network, for the link viewers elsewhere open.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, UdpSocket};

/// An address outside every local network (TEST-NET-1, which nothing answers), used only to
/// ask the OS which of its addresses reaches beyond this computer. Nothing is sent to it.
const BEYOND: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)), 9);

/// Whether something could listen at `address` now: another program, or another VLC, may
/// already hold its port.
pub fn is_port_free(address: SocketAddr) -> bool {
    TcpListener::bind(address).is_ok()
}

/// This computer's address on the network it reaches others through; `None` without one.
pub fn network_address() -> Option<IpAddr> {
    let socket = UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)).ok()?;
    // A UDP connect only picks the route: no packet leaves.
    socket.connect(BEYOND).ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_unspecified() && !ip.is_loopback()).then_some(ip)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_held_port_is_not_free() {
        let held = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = held.local_addr().unwrap();
        assert!(!is_port_free(address));
        drop(held);
        assert!(is_port_free(address));
    }
}
