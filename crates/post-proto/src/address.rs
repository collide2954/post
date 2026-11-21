use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

const LONGEST_NAME: usize = 253;
const LONGEST_LABEL: usize = 63;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Address {
    host: Host,
    port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Host {
    Name(String),
    Ip(IpAddr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressError {
    Empty,
    Invalid,
    InvalidPort,
}

impl Address {
    #[must_use]
    pub fn localhost(port: u16) -> Self {
        Self {
            host: Host::Name("localhost".to_owned()),
            port,
        }
    }

    #[must_use]
    pub fn from_ip(ip: IpAddr, port: u16) -> Self {
        Self {
            host: Host::Ip(ip),
            port,
        }
    }

    pub fn parse(text: &str, default_port: u16) -> Result<Self, AddressError> {
        if text.is_empty() {
            return Err(AddressError::Empty);
        }
        if !text.bytes().all(is_address_character) {
            return Err(AddressError::Invalid);
        }
        let (host, port) = match text.parse() {
            Ok(ip) => (Host::Ip(ip), None),
            Err(_) => split(text)?,
        };
        if matches!(host, Host::Ip(ip) if ip.is_unspecified()) {
            return Err(AddressError::Invalid);
        }
        let port = match port {
            None => default_port,
            Some(port) => port_from(port)?,
        };
        Ok(Self { host, port })
    }

    #[must_use]
    pub fn host(&self) -> &Host {
        &self.host
    }

    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    #[must_use]
    pub fn is_local(&self) -> bool {
        match &self.host {
            Host::Name(name) => name == "localhost",
            Host::Ip(ip) => ip.is_loopback(),
        }
    }
}

fn is_address_character(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b':' | b'[' | b']')
}

fn split(text: &str) -> Result<(Host, Option<&str>), AddressError> {
    if let Some(rest) = text.strip_prefix('[') {
        let (ip, after) = rest.split_once(']').ok_or(AddressError::Invalid)?;
        let ip: Ipv6Addr = ip.parse().map_err(|_| AddressError::Invalid)?;
        let port = match after.strip_prefix(':') {
            Some(port) => Some(port),
            None if after.is_empty() => None,
            None => return Err(AddressError::Invalid),
        };
        return Ok((Host::Ip(IpAddr::V6(ip)), port));
    }

    let (host, port) = match text.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (text, None),
    };
    Ok((host_from(host)?, port))
}

fn host_from(text: &str) -> Result<Host, AddressError> {
    if let Ok(ip) = text.parse::<Ipv4Addr>() {
        return Ok(Host::Ip(IpAddr::V4(ip)));
    }
    if is_host_name(text) {
        Ok(Host::Name(text.to_ascii_lowercase()))
    } else {
        Err(AddressError::Invalid)
    }
}

fn is_host_name(text: &str) -> bool {
    let last_is_a_number = text
        .rsplit('.')
        .next()
        .is_some_and(|label| label.bytes().all(|byte| byte.is_ascii_digit()));
    text.len() <= LONGEST_NAME && text.split('.').all(is_label) && !last_is_a_number
}

fn is_label(label: &str) -> bool {
    (1..=LONGEST_LABEL).contains(&label.len())
        && label
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        && !label.starts_with('-')
        && !label.ends_with('-')
}

fn port_from(text: &str) -> Result<u16, AddressError> {
    let is_number = text.bytes().all(|byte| byte.is_ascii_digit()) && !text.starts_with('0');
    text.parse()
        .ok()
        .filter(|_| is_number)
        .ok_or(AddressError::InvalidPort)
}

impl fmt::Display for Host {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name(name) => formatter.write_str(name),
            Self::Ip(IpAddr::V4(ip)) => ip.fmt(formatter),
            Self::Ip(IpAddr::V6(ip)) => write!(formatter, "[{ip}]"),
        }
    }
}

impl fmt::Display for Address {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.host, self.port)
    }
}

impl fmt::Display for AddressError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "the address is empty",
            Self::Invalid => "the address must have the form host or host:port",
            Self::InvalidPort => "the port must be a number from 1 to 65535",
        })
    }
}

impl std::error::Error for AddressError {}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use super::{Address, AddressError, Host};

    fn parse(text: &str) -> Result<Address, AddressError> {
        Address::parse(text, 443)
    }

    #[test]
    fn reads_a_host_name_with_and_without_a_port() {
        for (text, shown) in [
            ("chat.example.org", "chat.example.org:443"),
            ("chat.example.org:443", "chat.example.org:443"),
            ("chat.example.org:7678", "chat.example.org:7678"),
            ("Chat.Example.ORG:1", "chat.example.org:1"),
            ("localhost", "localhost:443"),
            ("localhost:65535", "localhost:65535"),
            ("my-server", "my-server:443"),
            ("a.b-c.d0", "a.b-c.d0:443"),
            ("xn--caf-dma.example", "xn--caf-dma.example:443"),
            ("1a.example", "1a.example:443"),
        ] {
            let address = parse(text).expect(text);
            assert_eq!(address.to_string(), shown);
            assert!(matches!(address.host(), Host::Name(_)), "{text}");
        }
    }

    #[test]
    fn reads_a_network_address_with_and_without_a_port() {
        for (text, shown) in [
            ("127.0.0.1", "127.0.0.1:443"),
            ("192.168.1.20:7678", "192.168.1.20:7678"),
            ("::1", "[::1]:443"),
            ("[::1]", "[::1]:443"),
            ("[::1]:9000", "[::1]:9000"),
            ("[2001:DB8:0:0::1]:7678", "[2001:db8::1]:7678"),
            ("2001:db8::1", "[2001:db8::1]:443"),
        ] {
            let address = parse(text).expect(text);
            assert_eq!(address.to_string(), shown);
            assert!(matches!(address.host(), Host::Ip(_)), "{text}");
        }
    }

    #[test]
    fn uses_the_default_port_that_the_caller_gives() {
        let address = Address::parse("localhost", 7678).expect("valid");

        assert_eq!(address.port(), 7678);
        assert_eq!(address.to_string(), "localhost:7678");
    }

    #[test]
    fn gives_the_parts_of_an_address() {
        let address = parse("Chat.Example.org:7000").expect("valid");
        assert_eq!(address.host(), &Host::Name("chat.example.org".to_owned()));
        assert_eq!(address.port(), 7000);

        let ip: IpAddr = "192.168.1.20".parse().expect("valid");
        let address = parse("192.168.1.20").expect("valid");
        assert_eq!(address.host(), &Host::Ip(ip));
        assert_eq!(Address::from_ip(ip, 443), address);
        assert_eq!(Address::localhost(443), parse("LOCALHOST").expect("valid"));
    }

    #[test]
    fn knows_an_address_of_the_local_computer() {
        for text in [
            "localhost",
            "LocalHost:9000",
            "127.0.0.1",
            "127.1.2.3:80",
            "::1",
        ] {
            assert!(parse(text).expect(text).is_local(), "{text}");
        }
        for text in [
            "chat.example.org",
            "localhost.example.org",
            "my.localhost",
            "192.168.1.20",
            "[2001:db8::1]:7678",
            "[::ffff:127.0.0.1]",
        ] {
            assert!(!parse(text).expect(text).is_local(), "{text}");
        }
    }

    #[test]
    fn refuses_an_empty_address() {
        assert_eq!(parse(""), Err(AddressError::Empty));
    }

    #[test]
    fn refuses_a_port_that_is_not_a_number_from_1_to_65535() {
        for text in [
            "localhost:0",
            "localhost:65536",
            "localhost:port",
            "localhost:",
            "localhost:-1",
            "localhost:080",
            "localhost:80:90",
            "127.0.0.1:0",
            "127.0.0.1:65536",
            "[::1]:0",
            "[::1]:port",
            "chat.example.org:port",
        ] {
            assert_eq!(parse(text), Err(AddressError::InvalidPort), "{text}");
        }
    }

    #[test]
    fn refuses_text_that_is_not_an_address() {
        let long_label = format!("{}.example", "a".repeat(64));
        let long_name = format!("{}.example", "a.".repeat(124));
        for text in [
            "local host",
            " localhost",
            "localhost\n",
            "ws://localhost:7678",
            "https://chat.example.org",
            "localhost/",
            "localhost:7678/chat",
            "[::1",
            "::1]",
            "[localhost]",
            "[::1]x",
            "caf\u{e9}.example",
            ":7678",
            "#lobby",
            "chat..example.org",
            ".example.org",
            "example.org.",
            "-chat.example.org",
            "chat-.example.org",
            "chat_server.example.org",
            "1.2.3",
            "example.123",
            "256.1.1.1",
            "0.0.0.0",
            "::",
            "[::]:7678",
            &long_label,
            &long_name,
        ] {
            assert_eq!(parse(text), Err(AddressError::Invalid), "{text}");
        }
    }

    #[test]
    fn accepts_the_longest_label_and_the_longest_name() {
        let label = format!("{}.example", "a".repeat(63));
        assert!(parse(&label).is_ok());

        let name = format!("{}example.org", "a.".repeat(121));
        assert_eq!(name.len(), 253);
        assert!(parse(&name).is_ok());
    }

    #[test]
    fn two_forms_of_one_address_are_equal() {
        assert_eq!(parse("LOCALHOST"), parse("localhost:443"));
        assert_eq!(parse("::1"), parse("[0:0:0:0:0:0:0:1]:443"));
        assert_ne!(parse("localhost"), parse("127.0.0.1"));
        assert_ne!(parse("localhost"), parse("localhost:444"));
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
    }
}
