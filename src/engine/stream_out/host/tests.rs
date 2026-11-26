use std::net::{Ipv4Addr, TcpListener};

use super::*;

/// How many fresh ports to try: another test running beside this one may take a port the
/// moment it is let go.
const PORT_TRIES: usize = 5;

#[test]
fn a_held_port_is_not_free() {
    let held = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    assert!(!is_port_free(held.local_addr().unwrap()));
}

#[test]
fn a_port_let_go_is_free_again() {
    let freed = (0..PORT_TRIES).any(|_| {
        let held = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = held.local_addr().unwrap();
        drop(held);
        is_port_free(address)
    });
    assert!(freed, "none of {PORT_TRIES} ports let go was free");
}
