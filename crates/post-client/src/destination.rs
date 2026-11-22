use std::fmt;

use post_proto::{Address, AddressError, Code, DEFAULT_PORT};

pub const TLS_PORT: u16 = 443;
const INVITATION: &str = "i/";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Destination {
    address: Address,
    security: Security,
    invitation: Option<Code>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Security {
    Plain,
    Tls,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestinationError {
    Empty,
    Invalid,
    InvalidPort,
    InvalidLink,
    NotLocal,
}

impl Destination {
    pub fn parse(text: &str) -> Result<Self, DestinationError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(DestinationError::Empty);
        }
        let (scheme, rest) = if let Some(rest) = without_start(text, "https://") {
            (Some(Security::Tls), rest)
        } else if let Some(rest) = without_start(text, "http://") {
            (Some(Security::Plain), rest)
        } else {
            (None, text)
        };
        if rest.contains("://") {
            return Err(DestinationError::Invalid);
        }
        let (authority, path) = match rest.split_once('/') {
            Some((authority, path)) => (authority, path),
            None => (rest, ""),
        };

        let is_local = address(authority, TLS_PORT)?.is_local();
        let security = scheme.unwrap_or(if is_local {
            Security::Plain
        } else {
            Security::Tls
        });
        if security == Security::Plain && !is_local {
            return Err(DestinationError::NotLocal);
        }
        let default_port = match security {
            Security::Plain => DEFAULT_PORT,
            Security::Tls => TLS_PORT,
        };
        let invitation = if path.is_empty() {
            None
        } else {
            let code = path
                .strip_prefix(INVITATION)
                .ok_or(DestinationError::InvalidLink)?;
            let code = code.strip_suffix('/').unwrap_or(code);
            Some(Code::parse(code).map_err(|_| DestinationError::InvalidLink)?)
        };
        Ok(Self {
            address: address(authority, default_port)?,
            security,
            invitation,
        })
    }

    #[must_use]
    pub fn to(address: Address, tls: bool) -> Self {
        let security = if tls || !address.is_local() {
            Security::Tls
        } else {
            Security::Plain
        };
        Self {
            address,
            security,
            invitation: None,
        }
    }

    #[must_use]
    pub fn address(&self) -> &Address {
        &self.address
    }

    #[must_use]
    pub fn security(&self) -> Security {
        self.security
    }

    #[must_use]
    pub fn invitation(&self) -> Option<&Code> {
        self.invitation.as_ref()
    }

    #[must_use]
    pub fn url(&self) -> String {
        match self.security {
            Security::Plain => format!("ws://{}/", self.address),
            Security::Tls => format!("wss://{}/", self.address),
        }
    }
}

fn without_start<'a>(text: &'a str, start: &str) -> Option<&'a str> {
    let head = text.get(..start.len())?;
    head.eq_ignore_ascii_case(start)
        .then(|| &text[start.len()..])
}

fn address(authority: &str, default_port: u16) -> Result<Address, DestinationError> {
    Address::parse(authority, default_port).map_err(|error| match error {
        AddressError::InvalidPort => DestinationError::InvalidPort,
        AddressError::Empty | AddressError::Invalid => DestinationError::Invalid,
    })
}

impl fmt::Display for Destination {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let is_local = self.address.is_local();
        if self.security == Security::Tls && is_local {
            formatter.write_str("https://")?;
        }
        if self.security == Security::Tls && self.address.port() == TLS_PORT {
            self.address.host().fmt(formatter)
        } else {
            self.address.fmt(formatter)
        }
    }
}

impl fmt::Display for DestinationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "the address is empty",
            Self::Invalid => "the text is not an invite link or a server address",
            Self::InvalidPort => "the port must be a number from 1 to 65535",
            Self::InvalidLink => "the invite link is not complete",
            Self::NotLocal => "a connection without TLS is possible only to this computer",
        })
    }
}

impl std::error::Error for DestinationError {}

#[cfg(test)]
mod tests {
    use super::{Destination, DestinationError, Security};

    fn parse(text: &str) -> Destination {
        Destination::parse(text).unwrap_or_else(|error| panic!("{text}: {error}"))
    }

    #[test]
    fn reads_the_address_of_a_server_on_a_network() {
        for (text, address, shown) in [
            (
                "chat.example.org",
                "chat.example.org:443",
                "chat.example.org",
            ),
            (
                "Chat.Example.org:443",
                "chat.example.org:443",
                "chat.example.org",
            ),
            (
                "chat.example.org:7678",
                "chat.example.org:7678",
                "chat.example.org:7678",
            ),
            (
                "https://chat.example.org",
                "chat.example.org:443",
                "chat.example.org",
            ),
            (
                "HTTPS://chat.example.org/",
                "chat.example.org:443",
                "chat.example.org",
            ),
            ("192.168.1.20", "192.168.1.20:443", "192.168.1.20"),
            (
                "[2001:db8::1]:7678",
                "[2001:db8::1]:7678",
                "[2001:db8::1]:7678",
            ),
            (
                "  chat.example.org\n",
                "chat.example.org:443",
                "chat.example.org",
            ),
        ] {
            let destination = parse(text);
            assert_eq!(destination.address().to_string(), address, "{text}");
            assert_eq!(destination.security(), Security::Tls, "{text}");
            assert_eq!(destination.invitation(), None, "{text}");
            assert_eq!(destination.to_string(), shown, "{text}");
            assert_eq!(destination.url(), format!("wss://{address}/"));
        }
    }

    #[test]
    fn reads_the_address_of_a_server_on_this_computer() {
        for (text, address) in [
            ("localhost", "localhost:7678"),
            ("localhost:9000", "localhost:9000"),
            ("127.0.0.1", "127.0.0.1:7678"),
            ("::1", "[::1]:7678"),
            ("http://localhost", "localhost:7678"),
            ("http://localhost:9000/", "localhost:9000"),
        ] {
            let destination = parse(text);
            assert_eq!(destination.address().to_string(), address, "{text}");
            assert_eq!(destination.security(), Security::Plain, "{text}");
            assert_eq!(destination.to_string(), address, "{text}");
            assert_eq!(destination.url(), format!("ws://{address}/"));
        }
    }

    #[test]
    fn uses_tls_on_this_computer_when_the_user_asks_for_it() {
        for (text, address, shown) in [
            ("https://localhost", "localhost:443", "https://localhost"),
            (
                "https://localhost:8443",
                "localhost:8443",
                "https://localhost:8443",
            ),
            (
                "https://127.0.0.1:8443",
                "127.0.0.1:8443",
                "https://127.0.0.1:8443",
            ),
        ] {
            let destination = parse(text);
            assert_eq!(destination.address().to_string(), address, "{text}");
            assert_eq!(destination.security(), Security::Tls, "{text}");
            assert_eq!(destination.to_string(), shown, "{text}");
        }
    }

    #[test]
    fn reads_an_invite_link() {
        for (text, address, security) in [
            (
                "https://chat.example.org/i/7Hq2kdX9mPq4",
                "chat.example.org:443",
                Security::Tls,
            ),
            (
                "https://chat.example.org:7678/i/7Hq2kdX9mPq4/",
                "chat.example.org:7678",
                Security::Tls,
            ),
            (
                "chat.example.org/i/7Hq2kdX9mPq4",
                "chat.example.org:443",
                Security::Tls,
            ),
            (
                "http://localhost:7678/i/7Hq2kdX9mPq4",
                "localhost:7678",
                Security::Plain,
            ),
        ] {
            let destination = parse(text);
            assert_eq!(destination.address().to_string(), address, "{text}");
            assert_eq!(destination.security(), security, "{text}");
            let code = destination.invitation().expect("an invitation");
            assert_eq!(code.as_str(), "7Hq2kdX9mPq4", "{text}");
        }
    }

    #[test]
    fn shows_a_text_that_gives_the_same_destination_again() {
        for text in [
            "chat.example.org",
            "chat.example.org:7678",
            "localhost",
            "localhost:9000",
            "https://localhost",
            "https://[::1]:8443",
            "[::1]",
            "192.168.1.20:7678",
        ] {
            let destination = parse(text);
            assert_eq!(parse(&destination.to_string()), destination, "{text}");
        }

        let link = parse("https://chat.example.org/i/7Hq2kdX9mPq4");
        assert_eq!(link.to_string(), "chat.example.org");
        assert_ne!(parse(&link.to_string()), link);
    }

    #[test]
    fn refuses_a_text_that_is_not_a_destination() {
        for (text, error) in [
            ("", DestinationError::Empty),
            ("  \n", DestinationError::Empty),
            ("https://", DestinationError::Invalid),
            ("local host", DestinationError::Invalid),
            ("ws://localhost:7678", DestinationError::Invalid),
            ("#general", DestinationError::Invalid),
            ("chat.example.org:port", DestinationError::InvalidPort),
            ("localhost:0", DestinationError::InvalidPort),
            ("http://chat.example.org", DestinationError::NotLocal),
            (
                "http://192.168.1.20:7678/i/7Hq2kdX9mPq4",
                DestinationError::NotLocal,
            ),
            ("https://chat.example.org/i/", DestinationError::InvalidLink),
            (
                "https://chat.example.org/i/short",
                DestinationError::InvalidLink,
            ),
            (
                "https://chat.example.org/i/7Hq2kdX9mPq4/more",
                DestinationError::InvalidLink,
            ),
            (
                "https://chat.example.org/invite/7Hq2kdX9mPq4",
                DestinationError::InvalidLink,
            ),
            (
                "https://chat.example.org/i/7Hq2kdX9mPq4?x=1",
                DestinationError::InvalidLink,
            ),
            ("chat.example.org/general", DestinationError::InvalidLink),
        ] {
            assert_eq!(Destination::parse(text), Err(error), "{text:?}");
        }
    }

    #[test]
    fn makes_a_destination_from_an_address_that_the_profile_knows() {
        for (address, tls, security, shown) in [
            ("localhost:7678", false, Security::Plain, "localhost:7678"),
            (
                "localhost:8443",
                true,
                Security::Tls,
                "https://localhost:8443",
            ),
            (
                "chat.example.org:443",
                true,
                Security::Tls,
                "chat.example.org",
            ),
            (
                "chat.example.org:443",
                false,
                Security::Tls,
                "chat.example.org",
            ),
            (
                "192.168.1.20:7678",
                false,
                Security::Tls,
                "192.168.1.20:7678",
            ),
        ] {
            let parsed = post_proto::Address::parse(address, 443).expect(address);

            let destination = Destination::to(parsed, tls);

            assert_eq!(destination.address().to_string(), address);
            assert_eq!(destination.security(), security, "{address}");
            assert_eq!(destination.invitation(), None);
            assert_eq!(destination.to_string(), shown);
        }
    }

    #[test]
    fn describes_each_error() {
        for (error, text) in [
            (DestinationError::Empty, "the address is empty"),
            (
                DestinationError::Invalid,
                "the text is not an invite link or a server address",
            ),
            (
                DestinationError::InvalidPort,
                "the port must be a number from 1 to 65535",
            ),
            (
                DestinationError::InvalidLink,
                "the invite link is not complete",
            ),
            (
                DestinationError::NotLocal,
                "a connection without TLS is possible only to this computer",
            ),
        ] {
            assert_eq!(error.to_string(), text);
        }
    }
}
