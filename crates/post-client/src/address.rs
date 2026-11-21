use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use post_proto::DEFAULT_PORT;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServerAddress {
    host: Host,
    port: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Host {
    Localhost,
    Ip(IpAddr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressError {
    Empty,
    Invalid,
    InvalidPort,
    NotLocal,
}

impl ServerAddress {
    pub fn parse(text: &str) -> Result<Self, AddressError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(AddressError::Empty);
        }
        if text.contains('/') {
            return Err(AddressError::Invalid);
        }

        let (host, port) = match text.parse() {
            Ok(ip) => (Some(Host::Ip(ip)), None),
            Err(_) => split(text)?,
        };
        let port = match port {
            None => DEFAULT_PORT,
            Some(port) => port
                .parse()
                .ok()
                .filter(|port| *port != 0)
                .ok_or(AddressError::InvalidPort)?,
        };

        match host {
            Some(Host::Ip(ip)) if !ip.is_loopback() => Err(AddressError::NotLocal),
            Some(host) => Ok(Self { host, port }),
            None => Err(AddressError::NotLocal),
        }
    }

    #[must_use]
    pub fn candidates(&self) -> Vec<SocketAddr> {
        match self.host {
            Host::Ip(ip) => vec![SocketAddr::new(ip, self.port)],
            Host::Localhost => vec![
                SocketAddr::from((Ipv4Addr::LOCALHOST, self.port)),
                SocketAddr::from((Ipv6Addr::LOCALHOST, self.port)),
            ],
        }
    }
}

fn split(text: &str) -> Result<(Option<Host>, Option<&str>), AddressError> {
    if let Some(rest) = text.strip_prefix('[') {
        let (ip, after) = rest.split_once(']').ok_or(AddressError::Invalid)?;
        let ip: Ipv6Addr = ip.parse().map_err(|_| AddressError::Invalid)?;
        let port = match after.strip_prefix(':') {
            Some(port) => Some(port),
            None if after.is_empty() => None,
            None => return Err(AddressError::Invalid),
        };
        return Ok((Some(Host::Ip(IpAddr::V6(ip))), port));
    }

    let (host, port) = match text.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (text, None),
    };
    Ok((host_from(host)?, port))
}

fn host_from(text: &str) -> Result<Option<Host>, AddressError> {
    if text.eq_ignore_ascii_case("localhost") {
        return Ok(Some(Host::Localhost));
    }
    if let Ok(ip) = text.parse::<Ipv4Addr>() {
        return Ok(Some(Host::Ip(IpAddr::V4(ip))));
    }
    let is_host_name = !text.is_empty()
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'.');
    if is_host_name {
        Ok(None)
    } else {
        Err(AddressError::Invalid)
    }
}

impl fmt::Display for ServerAddress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.host {
            Host::Ip(ip) => SocketAddr::new(ip, self.port).fmt(formatter),
            Host::Localhost => write!(formatter, "localhost:{}", self.port),
        }
    }
}

impl fmt::Display for AddressError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "the address is empty",
            Self::Invalid => "the address must have the form host or host:port",
            Self::InvalidPort => "the port must be a number from 1 to 65535",
            Self::NotLocal => "this version connects only to this computer",
        })
    }
}

impl std::error::Error for AddressError {}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use super::{AddressError, ServerAddress};

    fn sockets(list: &[&str]) -> Vec<SocketAddr> {
        list.iter().map(|text| text.parse().expect(text)).collect()
    }

    #[test]
    fn reads_localhost_with_and_without_a_port() {
        for (text, shown, port) in [
            ("localhost", "localhost:7678", 7678),
            ("localhost:7678", "localhost:7678", 7678),
            ("localhost:9000", "localhost:9000", 9000),
            ("LocalHost:1", "localhost:1", 1),
            ("  localhost:65535\n", "localhost:65535", 65535),
        ] {
            let address = ServerAddress::parse(text).expect(text);
            assert_eq!(address.to_string(), shown);
            assert_eq!(
                address.candidates(),
                sockets(&[&format!("127.0.0.1:{port}"), &format!("[::1]:{port}")])
            );
        }
    }

    #[test]
    fn reads_a_loopback_address_with_and_without_a_port() {
        for (text, shown) in [
            ("127.0.0.1", "127.0.0.1:7678"),
            ("127.0.0.1:9000", "127.0.0.1:9000"),
            ("127.1.2.3:80", "127.1.2.3:80"),
            ("::1", "[::1]:7678"),
            ("[::1]", "[::1]:7678"),
            ("[::1]:9000", "[::1]:9000"),
        ] {
            let address = ServerAddress::parse(text).expect(text);
            assert_eq!(address.to_string(), shown);
            assert_eq!(address.candidates(), sockets(&[shown]));
        }
    }

    #[test]
    fn refuses_an_empty_address() {
        for text in ["", "   ", "\t\n"] {
            assert_eq!(
                ServerAddress::parse(text),
                Err(AddressError::Empty),
                "{text:?}"
            );
        }
    }

    #[test]
    fn refuses_an_address_that_is_not_this_computer() {
        for text in [
            "chat.example.org",
            "chat.example.org:443",
            "example",
            "localhost.example.org",
            "192.168.1.20",
            "192.168.1.20:7678",
            "10.0.0.1:7678",
            "0.0.0.0",
            "8.8.8.8:53",
            "::",
            "[2001:db8::1]:7678",
            "2001:db8::1",
            "[::ffff:192.168.1.20]:7678",
        ] {
            assert_eq!(
                ServerAddress::parse(text),
                Err(AddressError::NotLocal),
                "{text}"
            );
        }
    }

    #[test]
    fn refuses_a_port_that_is_not_a_number_from_1_to_65535() {
        for text in [
            "localhost:0",
            "localhost:65536",
            "localhost:port",
            "localhost:",
            "localhost:-1",
            "localhost:80:90",
            "127.0.0.1:0",
            "127.0.0.1:65536",
            "[::1]:0",
            "[::1]:port",
            "chat.example.org:port",
        ] {
            assert_eq!(
                ServerAddress::parse(text),
                Err(AddressError::InvalidPort),
                "{text}"
            );
        }
    }

    #[test]
    fn refuses_text_that_is_not_an_address() {
        for text in [
            "local host",
            "ws://localhost:7678",
            "http://localhost",
            "localhost/",
            "localhost:7678/chat",
            "[::1",
            "::1]",
            "[localhost]",
            "caf\u{e9}.example",
            ":7678",
            "#lobby",
        ] {
            assert_eq!(
                ServerAddress::parse(text),
                Err(AddressError::Invalid),
                "{text}"
            );
        }
    }

    #[test]
    fn two_forms_of_one_address_are_equal() {
        assert_eq!(
            ServerAddress::parse("LOCALHOST"),
            ServerAddress::parse("localhost:7678")
        );
        assert_ne!(
            ServerAddress::parse("localhost"),
            ServerAddress::parse("127.0.0.1")
        );
    }

    #[test]
    fn describes_each_error() {
        assert_eq!(AddressError::Empty.to_string(), "the address is empty");
        assert_eq!(
            AddressError::Invalid.to_string(),
            "the address must have the form host or host:port"
        );
        assert_eq!(
            AddressError::InvalidPort.to_string(),
            "the port must be a number from 1 to 65535"
        );
        assert_eq!(
            AddressError::NotLocal.to_string(),
            "this version connects only to this computer"
        );
    }
}
