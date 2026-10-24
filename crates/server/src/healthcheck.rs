//! `factory-server healthcheck`: the container health probe. It needs no curl in the image.
//! Exit code 0 when `/readyz` answers 200 on the local listen port.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(4);

/// The loopback address of the configured listen port.
pub fn local_target(listen: &str) -> Option<SocketAddr> {
    let port = listen.rsplit(':').next()?.parse::<u16>().ok()?;
    Some(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))
}

/// True when `GET /readyz` on `addr` answers 200.
pub fn probe(addr: SocketAddr) -> bool {
    let attempt = || -> std::io::Result<bool> {
        let mut stream = TcpStream::connect_timeout(&addr, TIMEOUT)?;
        stream.set_read_timeout(Some(TIMEOUT))?;
        stream.set_write_timeout(Some(TIMEOUT))?;
        stream
            .write_all(b"GET /readyz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")?;
        let mut head = [0u8; 16];
        let mut filled = 0;
        while filled < head.len() {
            let n = stream.read(&mut head[filled..])?;
            if n == 0 {
                break;
            }
            filled += n;
        }
        Ok(head[..filled].starts_with(b"HTTP/1.1 200"))
    };
    attempt().unwrap_or(false)
}

/// Reads `FACTORY_LISTEN` like the server does and probes it.
pub fn run_from_env() -> bool {
    let listen = std::env::var("FACTORY_LISTEN").unwrap_or_else(|_| "0.0.0.0:8080".to_owned());
    local_target(&listen).is_some_and(probe)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_the_loopback_of_the_listen_port() {
        assert_eq!(
            local_target("0.0.0.0:8080"),
            Some("127.0.0.1:8080".parse().unwrap())
        );
        assert_eq!(
            local_target("[::]:9000"),
            Some("127.0.0.1:9000".parse().unwrap())
        );
        assert_eq!(local_target("nonsense"), None);
    }

    #[test]
    fn a_closed_port_is_unhealthy() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        assert!(!probe(addr));
    }
}
