const PROFILE: &str = "--profile";
const PROFILE_WITH_VALUE: &str = "--profile=";
const MACOS_PROCESS_NUMBER: &str = "-psn_";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Arguments {
    pub profile: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgumentError {
    Unknown(String),
    NoProfileName,
}

impl Arguments {
    pub fn parse(arguments: impl IntoIterator<Item = String>) -> Result<Self, ArgumentError> {
        let mut parsed = Self::default();
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            let name = if argument == PROFILE {
                arguments.next().ok_or(ArgumentError::NoProfileName)?
            } else if let Some(name) = argument.strip_prefix(PROFILE_WITH_VALUE) {
                name.to_owned()
            } else if argument.starts_with(MACOS_PROCESS_NUMBER) {
                continue;
            } else {
                return Err(ArgumentError::Unknown(argument));
            };
            if name.trim().is_empty() {
                return Err(ArgumentError::NoProfileName);
            }
            parsed.profile = Some(name);
        }
        Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::{ArgumentError, Arguments};

    fn parse(arguments: &[&str]) -> Result<Arguments, ArgumentError> {
        Arguments::parse(arguments.iter().map(|argument| (*argument).to_owned()))
    }

    fn profile(name: &str) -> Arguments {
        Arguments {
            profile: Some(name.to_owned()),
        }
    }

    #[test]
    fn has_no_profile_without_arguments() {
        assert_eq!(parse(&[]), Ok(Arguments::default()));
        assert_eq!(Arguments::default().profile, None);
    }

    #[test]
    fn reads_the_name_of_a_profile() {
        assert_eq!(parse(&["--profile", "work"]), Ok(profile("work")));
        assert_eq!(parse(&["--profile=work"]), Ok(profile("work")));
        assert_eq!(parse(&["--profile", "my garden"]), Ok(profile("my garden")));
        assert_eq!(parse(&["--profile", "--profile"]), Ok(profile("--profile")));
        assert_eq!(
            parse(&["--profile", "work", "--profile=home"]),
            Ok(profile("home"))
        );
    }

    #[test]
    fn refuses_a_profile_argument_without_a_name() {
        for arguments in [
            ["--profile"].as_slice(),
            &["--profile="],
            &["--profile", " "],
            &["--profile", "work", "--profile"],
        ] {
            assert_eq!(
                parse(arguments),
                Err(ArgumentError::NoProfileName),
                "{arguments:?}"
            );
        }
    }

    #[test]
    fn refuses_an_argument_that_it_does_not_know() {
        for argument in ["--help", "work", "-p", "--profiles", ""] {
            assert_eq!(
                parse(&[argument]),
                Err(ArgumentError::Unknown(argument.to_owned())),
                "{argument}"
            );
        }
        assert_eq!(
            parse(&["--profile", "work", "--fast"]),
            Err(ArgumentError::Unknown("--fast".to_owned()))
        );
    }

    #[test]
    fn ignores_the_process_number_that_macos_can_give() {
        assert_eq!(parse(&["-psn_0_12345"]), Ok(Arguments::default()));
        assert_eq!(
            parse(&["-psn_0_12345", "--profile", "work"]),
            Ok(profile("work"))
        );
    }
}
