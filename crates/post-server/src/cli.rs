use std::path::{Path, PathBuf};

use post_proto::DEFAULT_PORT;

use crate::config::{Config, ConfigError, FILE_NAME};

pub const USAGE: &str = "usage: post-server [--config FILE | --port PORT]
       post-server owner-link [--config FILE | --port PORT]

Without an option, the server reads the file post-server.toml in this directory.
If the file does not exist, the server runs in development mode on this computer.

  --config FILE   read this configuration file
  --port PORT     run in development mode with this port
  owner-link      make a new owner link; stop the server before this command";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Run(Source),
    OwnerLink(Source),
    Help,
    Version,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Directory,
    File(PathBuf),
    Development(u16),
}

pub fn command_from(arguments: impl IntoIterator<Item = String>) -> Result<Command, &'static str> {
    let arguments: Vec<String> = arguments.into_iter().collect();
    let mut words: &[String] = &arguments;

    match words {
        [word] if word == "--help" || word == "-h" => return Ok(Command::Help),
        [word] if word == "--version" || word == "-V" => return Ok(Command::Version),
        _ => {}
    }
    let is_owner_link = words.first().is_some_and(|word| word == "owner-link");
    if is_owner_link {
        words = &words[1..];
    }
    let source = match words {
        [] => Source::Directory,
        [option, file] if option == "--config" && !file.is_empty() => {
            Source::File(PathBuf::from(file))
        }
        [option, port] if option == "--port" => {
            Source::Development(port.parse().ok().filter(|port| *port != 0).ok_or(USAGE)?)
        }
        _ => return Err(USAGE),
    };
    Ok(if is_owner_link {
        Command::OwnerLink(source)
    } else {
        Command::Run(source)
    })
}

impl Source {
    pub fn config(&self, directory: &Path) -> Result<Config, ConfigError> {
        match self {
            Self::File(path) => Config::load(path),
            Self::Development(port) => Ok(Config::development(*port)),
            Self::Directory => {
                let path = directory.join(FILE_NAME);
                if path.is_file() {
                    Config::load(&path)
                } else {
                    Ok(Config::development(DEFAULT_PORT))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use post_testing::Directory;

    use super::{Command, Source, USAGE, command_from};
    use crate::config::{Config, ConfigError, Tls};

    fn command(list: &[&str]) -> Result<Command, &'static str> {
        command_from(list.iter().map(|text| (*text).to_owned()))
    }

    #[test]
    fn runs_the_server_without_an_argument() {
        assert_eq!(command(&[]), Ok(Command::Run(Source::Directory)));
    }

    #[test]
    fn reads_the_options_of_the_server() {
        assert_eq!(
            command(&["--config", "etc/post.toml"]),
            Ok(Command::Run(Source::File(PathBuf::from("etc/post.toml"))))
        );
        for (text, port) in [("1", 1), ("9000", 9000), ("65535", 65535)] {
            assert_eq!(
                command(&["--port", text]),
                Ok(Command::Run(Source::Development(port)))
            );
        }
    }

    #[test]
    fn reads_the_owner_link_command() {
        assert_eq!(
            command(&["owner-link"]),
            Ok(Command::OwnerLink(Source::Directory))
        );
        assert_eq!(
            command(&["owner-link", "--config", "post.toml"]),
            Ok(Command::OwnerLink(Source::File(PathBuf::from("post.toml"))))
        );
        assert_eq!(
            command(&["owner-link", "--port", "9000"]),
            Ok(Command::OwnerLink(Source::Development(9000)))
        );
    }

    #[test]
    fn reads_a_question_about_the_program() {
        for word in ["--help", "-h"] {
            assert_eq!(command(&[word]), Ok(Command::Help));
        }
        for word in ["--version", "-V"] {
            assert_eq!(command(&[word]), Ok(Command::Version));
        }
    }

    #[test]
    fn refuses_arguments_that_it_does_not_know() {
        for invalid in [
            &["9000"][..],
            &["--port"],
            &["--port", "0"],
            &["--port", "65536"],
            &["--port", "port"],
            &["--port", "9000", "9001"],
            &["--config"],
            &["--config", ""],
            &["--config", "a.toml", "--port", "9000"],
            &["--help", "--version"],
            &["owner-link", "owner-link"],
            &["--port", "9000", "owner-link"],
            &["run"],
        ] {
            assert_eq!(command(invalid), Err(USAGE), "{invalid:?}");
        }
    }

    #[test]
    fn uses_development_mode_in_a_directory_without_a_file() {
        let directory = Directory::new();

        let config = Source::Directory.config(directory.path()).expect("valid");

        assert_eq!(config, Config::development(7678));
        assert_eq!(
            Source::Development(9000)
                .config(directory.path())
                .expect("valid"),
            Config::development(9000)
        );
    }

    #[test]
    fn reads_the_file_of_the_directory_or_of_the_option() {
        let directory = Directory::new();
        let text = "address = \"chat.example.org\"\n[tls]\nmode = \"proxy\"\n";
        std::fs::write(directory.join("post-server.toml"), text).expect("a file");
        std::fs::write(directory.join("other.toml"), "address = \"b.example\"\n").expect("a file");

        let found = Source::Directory.config(directory.path()).expect("valid");
        assert_eq!(found.tls, Tls::Proxy);
        assert_eq!(found.data, directory.join("data"));

        let named = Source::File(directory.join("other.toml"))
            .config(directory.path())
            .expect("valid");
        assert_eq!(named.tls, Tls::Own);

        let missing = Source::File(directory.join("missing.toml")).config(directory.path());
        assert!(matches!(missing, Err(ConfigError::Read(_))));
    }

    #[test]
    fn tells_how_to_start_the_program() {
        assert!(USAGE.starts_with("usage: post-server [--config FILE | --port PORT]\n"));
        assert!(USAGE.contains("owner-link"));
    }
}
