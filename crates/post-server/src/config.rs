use std::fmt;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};

use post_proto::{Address, DEFAULT_PORT};
use serde::Deserialize;

pub const FILE_NAME: &str = "post-server.toml";
pub const TLS_PORT: u16 = 443;
const DEVELOPMENT_DATA: &str = "post-server-data";
const DATA: &str = "data";
const DATABASE: &str = "post.db";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub addresses: Vec<Address>,
    pub listen: Listen,
    pub data: PathBuf,
    pub tls: Tls,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Listen {
    Loopback(u16),
    All(u16),
    Given(Vec<SocketAddr>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tls {
    Off,
    Own,
    Files { certificate: PathBuf, key: PathBuf },
    Proxy,
}

#[derive(Debug)]
pub enum ConfigError {
    Read(std::io::Error),
    Syntax(toml::de::Error),
    NoAddress,
    Address(String),
    Listen(String),
    FilesWithoutMode,
    MissingFile,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    address: Texts,
    listen: Option<Texts>,
    data: Option<PathBuf>,
    #[serde(default)]
    tls: TlsSection,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Texts {
    One(String),
    Many(Vec<String>),
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct TlsSection {
    mode: Option<Mode>,
    certificate: Option<PathBuf>,
    key: Option<PathBuf>,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Mode {
    Own,
    Files,
    Proxy,
}

impl Config {
    #[must_use]
    pub fn development(port: u16) -> Self {
        Self {
            addresses: Vec::new(),
            listen: Listen::Loopback(port),
            data: PathBuf::from(DEVELOPMENT_DATA),
            tls: Tls::Off,
        }
    }

    #[must_use]
    pub fn public_addresses(&self, port: u16) -> Vec<Address> {
        if self.addresses.is_empty() {
            vec![
                Address::localhost(port),
                Address::from_ip(Ipv4Addr::LOCALHOST.into(), port),
                Address::from_ip(Ipv6Addr::LOCALHOST.into(), port),
            ]
        } else {
            self.addresses.clone()
        }
    }

    #[must_use]
    pub fn link(&self, port: u16) -> String {
        let address = &self.public_addresses(port)[0];
        match self.tls {
            Tls::Off => format!("http://{address}"),
            _ if address.port() == TLS_PORT => format!("https://{}", address.host()),
            _ => format!("https://{address}"),
        }
    }

    #[must_use]
    pub fn port(&self) -> u16 {
        match &self.listen {
            Listen::Loopback(port) | Listen::All(port) => *port,
            Listen::Given(addresses) => addresses.first().map_or(TLS_PORT, SocketAddr::port),
        }
    }

    #[must_use]
    pub fn database(&self) -> PathBuf {
        self.data.join(DATABASE)
    }

    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(ConfigError::Read)?;
        let directory = path.parent().unwrap_or_else(|| Path::new(""));
        Self::parse(&text, directory)
    }

    pub fn parse(text: &str, directory: &Path) -> Result<Self, ConfigError> {
        let file: File = toml::from_str(text).map_err(ConfigError::Syntax)?;

        let addresses = file
            .address
            .into_vec()
            .iter()
            .map(|text| {
                Address::parse(text, TLS_PORT).map_err(|_| ConfigError::Address(text.clone()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if addresses.is_empty() {
            return Err(ConfigError::NoAddress);
        }

        let tls = file.tls.into_tls(directory)?;
        let listen = match file.listen {
            Some(texts) => Listen::Given(
                texts
                    .into_vec()
                    .iter()
                    .map(|text| text.parse().map_err(|_| ConfigError::Listen(text.clone())))
                    .collect::<Result<_, _>>()?,
            ),
            None if tls == Tls::Proxy => Listen::Loopback(DEFAULT_PORT),
            None => Listen::All(TLS_PORT),
        };
        let data = directory.join(file.data.unwrap_or_else(|| PathBuf::from(DATA)));

        Ok(Self {
            addresses,
            listen,
            data,
            tls,
        })
    }
}

impl Texts {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::One(text) => vec![text],
            Self::Many(texts) => texts,
        }
    }
}

impl TlsSection {
    fn into_tls(self, directory: &Path) -> Result<Tls, ConfigError> {
        let has_files = self.certificate.is_some() || self.key.is_some();
        match self
            .mode
            .unwrap_or(if has_files { Mode::Files } else { Mode::Own })
        {
            Mode::Files => match (self.certificate, self.key) {
                (Some(certificate), Some(key)) => Ok(Tls::Files {
                    certificate: directory.join(certificate),
                    key: directory.join(key),
                }),
                _ => Err(ConfigError::MissingFile),
            },
            _ if has_files => Err(ConfigError::FilesWithoutMode),
            Mode::Own => Ok(Tls::Own),
            Mode::Proxy => Ok(Tls::Proxy),
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(formatter, "cannot read the configuration file: {error}"),
            Self::Syntax(error) => {
                write!(formatter, "the configuration file is not valid: {error}")
            }
            Self::NoAddress => formatter.write_str("the configuration file must give an address"),
            Self::Address(text) => write!(
                formatter,
                "the address \"{text}\" is not valid: use a host name with an optional port"
            ),
            Self::Listen(text) => write!(
                formatter,
                "the listen address \"{text}\" is not valid: use a network address with a port"
            ),
            Self::FilesWithoutMode => formatter
                .write_str("the certificate files are permitted only with the TLS mode \"files\""),
            Self::MissingFile => formatter
                .write_str("the TLS mode \"files\" must have a certificate file and a key file"),
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::path::{Path, PathBuf};

    use post_proto::Address;

    use super::{Config, ConfigError, Listen, Tls};

    fn directory() -> PathBuf {
        Path::new("etc").join("post")
    }

    fn parse(text: &str) -> Result<Config, ConfigError> {
        Config::parse(text, &directory())
    }

    fn address(text: &str) -> Address {
        Address::parse(text, 443).expect(text)
    }

    fn sockets(list: &[&str]) -> Vec<SocketAddr> {
        list.iter().map(|text| text.parse().expect(text)).collect()
    }

    #[test]
    fn gives_the_development_mode_without_a_file() {
        let config = Config::development(7678);

        assert_eq!(
            config,
            Config {
                addresses: Vec::new(),
                listen: Listen::Loopback(7678),
                data: PathBuf::from("post-server-data"),
                tls: Tls::Off,
            }
        );
    }

    #[test]
    fn answers_to_the_names_of_the_local_computer_in_development_mode() {
        let config = Config::development(0);

        let addresses: Vec<String> = config
            .public_addresses(9000)
            .iter()
            .map(ToString::to_string)
            .collect();

        assert_eq!(
            addresses,
            ["localhost:9000", "127.0.0.1:9000", "[::1]:9000"]
        );
        assert_eq!(config.link(9000), "http://localhost:9000");
        assert_eq!(config.port(), 0);
        assert_eq!(
            config.database(),
            Path::new("post-server-data").join("post.db")
        );
    }

    #[test]
    fn answers_to_the_addresses_of_the_file() {
        let config =
            parse("address = [\"chat.example.org\", \"192.168.1.20:7678\"]\n").expect("valid");

        assert_eq!(config.public_addresses(9000), config.addresses);
        assert_eq!(config.link(9000), "https://chat.example.org");
        assert_eq!(config.port(), 443);
        assert_eq!(config.database(), directory().join("data").join("post.db"));
    }

    #[test]
    fn puts_a_port_that_is_not_443_into_the_link() {
        let config = parse("address = \"chat.example.org:7678\"\nlisten = \"0.0.0.0:7000\"\n")
            .expect("valid");

        assert_eq!(config.link(1), "https://chat.example.org:7678");
        assert_eq!(config.port(), 7000);
    }

    #[test]
    fn reads_a_file_with_only_an_address() {
        let config = parse("address = \"chat.example.org\"\n").expect("valid");

        assert_eq!(
            config,
            Config {
                addresses: vec![address("chat.example.org:443")],
                listen: Listen::All(443),
                data: directory().join("data"),
                tls: Tls::Own,
            }
        );
    }

    #[test]
    fn reads_a_file_with_each_setting() {
        let config = parse(
            r#"
            address = ["Chat.Example.org", "192.168.1.20:7678"]
            listen = ["0.0.0.0:8443", "[::]:8443"]
            data = "store"

            [tls]
            mode = "files"
            certificate = "fullchain.pem"
            key = "private/key.pem"
            "#,
        )
        .expect("valid");

        assert_eq!(
            config,
            Config {
                addresses: vec![
                    address("chat.example.org:443"),
                    address("192.168.1.20:7678")
                ],
                listen: Listen::Given(sockets(&["0.0.0.0:8443", "[::]:8443"])),
                data: directory().join("store"),
                tls: Tls::Files {
                    certificate: directory().join("fullchain.pem"),
                    key: directory().join("private").join("key.pem"),
                },
            }
        );
    }

    #[test]
    fn accepts_one_listen_address_as_a_text() {
        let config =
            parse("address = \"a.example\"\nlisten = \"127.0.0.1:9000\"\n").expect("valid");

        assert_eq!(config.listen, Listen::Given(sockets(&["127.0.0.1:9000"])));
    }

    #[test]
    fn selects_certificate_files_when_the_file_gives_them() {
        let config =
            parse("address = \"a.example\"\n[tls]\ncertificate = \"c.pem\"\nkey = \"k.pem\"\n")
                .expect("valid");

        assert_eq!(
            config.tls,
            Tls::Files {
                certificate: directory().join("c.pem"),
                key: directory().join("k.pem"),
            }
        );
    }

    #[test]
    fn listens_on_the_local_computer_behind_a_proxy() {
        let config = parse("address = \"a.example\"\n[tls]\nmode = \"proxy\"\n").expect("valid");

        assert_eq!(config.tls, Tls::Proxy);
        assert_eq!(config.listen, Listen::Loopback(7678));
    }

    #[test]
    fn makes_its_own_certificate_by_default_and_on_demand() {
        for text in [
            "address = \"a.example\"\n",
            "address = \"a.example\"\n[tls]\n",
            "address = \"a.example\"\n[tls]\nmode = \"own\"\n",
        ] {
            assert_eq!(parse(text).expect(text).tls, Tls::Own);
        }
    }

    #[test]
    fn keeps_a_full_path_as_it_is() {
        let full = if cfg!(windows) {
            "C:\\post\\data"
        } else {
            "/var/lib/post"
        };
        let text = format!("address = \"a.example\"\ndata = '{full}'\n");

        assert_eq!(parse(&text).expect("valid").data, PathBuf::from(full));
    }

    #[test]
    fn refuses_a_file_that_is_not_valid_toml() {
        for text in [
            "address = ",
            "address = 7",
            "address = \"a.example\"\nport = 7678\n",
            "address = \"a.example\"\n[tls]\nmode = \"none\"\n",
            "address = \"a.example\"\n[tls]\nfile = \"c.pem\"\n",
            "",
        ] {
            assert!(matches!(parse(text), Err(ConfigError::Syntax(_))), "{text}");
        }
    }

    #[test]
    fn refuses_a_file_without_an_address() {
        assert!(matches!(
            parse("address = []\n"),
            Err(ConfigError::NoAddress)
        ));
    }

    #[test]
    fn refuses_an_address_that_is_not_valid() {
        for bad in ["https://a.example", "a.example:0", ""] {
            let text = format!("address = [\"a.example\", \"{bad}\"]\n");
            match parse(&text) {
                Err(ConfigError::Address(found)) => assert_eq!(found, bad),
                other => panic!("{bad}: {other:?}"),
            }
        }
    }

    #[test]
    fn refuses_a_listen_address_that_is_not_valid() {
        for bad in ["localhost:443", "0.0.0.0", "443"] {
            let text = format!("address = \"a.example\"\nlisten = [\"{bad}\"]\n");
            match parse(&text) {
                Err(ConfigError::Listen(found)) => assert_eq!(found, bad),
                other => panic!("{bad}: {other:?}"),
            }
        }
    }

    #[test]
    fn refuses_certificate_settings_that_do_not_agree() {
        for text in [
            "address = \"a.example\"\n[tls]\nmode = \"files\"\n",
            "address = \"a.example\"\n[tls]\nmode = \"files\"\nkey = \"k.pem\"\n",
            "address = \"a.example\"\n[tls]\ncertificate = \"c.pem\"\n",
        ] {
            assert!(
                matches!(parse(text), Err(ConfigError::MissingFile)),
                "{text}"
            );
        }
        for mode in ["own", "proxy"] {
            let text = format!(
                "address = \"a.example\"\n[tls]\nmode = \"{mode}\"\ncertificate = \"c.pem\"\nkey = \"k.pem\"\n"
            );
            assert!(
                matches!(parse(&text), Err(ConfigError::FilesWithoutMode)),
                "{mode}"
            );
        }
    }

    #[test]
    fn reads_a_file_from_the_disk() {
        let directory =
            std::env::temp_dir().join(format!("post-config-test-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("a temporary directory");
        let path = directory.join("post-server.toml");
        std::fs::write(&path, "address = \"a.example\"\n").expect("a file");

        let config = Config::load(&path);
        let missing = Config::load(&directory.join("missing.toml"));
        std::fs::remove_dir_all(&directory).expect("a removed directory");

        assert_eq!(config.expect("valid").data, directory.join("data"));
        assert!(matches!(missing, Err(ConfigError::Read(_))));
    }

    #[test]
    fn describes_each_error() {
        let read = Config::load(Path::new("post-missing-directory").join("x.toml").as_path());
        let syntax = parse("address = ");
        let errors = [
            (read, "cannot read the configuration file: "),
            (syntax, "the configuration file is not valid: "),
            (
                Err(ConfigError::NoAddress),
                "the configuration file must give an address",
            ),
            (
                Err(ConfigError::Address("x y".to_owned())),
                "the address \"x y\" is not valid: use a host name with an optional port",
            ),
            (
                Err(ConfigError::Listen("x".to_owned())),
                "the listen address \"x\" is not valid: use a network address with a port",
            ),
            (
                Err(ConfigError::FilesWithoutMode),
                "the certificate files are permitted only with the TLS mode \"files\"",
            ),
            (
                Err(ConfigError::MissingFile),
                "the TLS mode \"files\" must have a certificate file and a key file",
            ),
        ];
        for (result, start) in errors {
            let text = result.expect_err("an error").to_string();
            assert!(text.starts_with(start), "{text}");
        }
    }
}
