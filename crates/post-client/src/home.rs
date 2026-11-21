use std::ffi::OsString;
use std::path::{Path, PathBuf};

const VARIABLE: &str = "POST_HOME";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Home {
    pub settings: PathBuf,
    pub data: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum System {
    Windows,
    Mac,
    Other,
}

impl System {
    #[must_use]
    pub fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::Mac
        } else {
            Self::Other
        }
    }
}

impl Home {
    #[must_use]
    pub fn at(directory: &Path) -> Self {
        Self {
            settings: directory.to_owned(),
            data: directory.to_owned(),
        }
    }

    #[must_use]
    pub fn of_this_user() -> Option<Self> {
        Self::find(System::current(), |name| std::env::var_os(name))
    }

    pub fn find(system: System, variable: impl Fn(&str) -> Option<OsString>) -> Option<Self> {
        let directory = |name: &str| {
            variable(name)
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
        };
        if let Some(home) = directory(VARIABLE) {
            return Some(Self::at(&home));
        }
        match system {
            System::Windows => Some(Self::at(&directory("APPDATA")?.join("Post"))),
            System::Mac => {
                let support = directory("HOME")?
                    .join("Library")
                    .join("Application Support");
                Some(Self::at(&support.join("Post")))
            }
            System::Other => {
                let home = directory("HOME");
                let settings =
                    directory("XDG_CONFIG_HOME").or_else(|| Some(home.clone()?.join(".config")))?;
                let data = directory("XDG_DATA_HOME")
                    .or_else(|| Some(home?.join(".local").join("share")))?;
                Some(Self {
                    settings: settings.join("post"),
                    data: data.join("post"),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    use super::{Home, System};

    fn root(name: &str) -> PathBuf {
        if cfg!(windows) {
            Path::new("C:\\").join(name)
        } else {
            Path::new("/").join(name)
        }
    }

    fn find(system: System, variables: &[(&str, PathBuf)]) -> Option<Home> {
        Home::find(system, |name| {
            variables
                .iter()
                .find(|(known, _)| *known == name)
                .map(|(_, value)| OsString::from(value))
        })
    }

    #[test]
    fn uses_one_directory_when_the_caller_gives_it() {
        let home = Home::at(Path::new("portable"));

        assert_eq!(home.settings, Path::new("portable"));
        assert_eq!(home.data, Path::new("portable"));
    }

    #[test]
    fn obeys_the_variable_of_post_on_each_system() {
        for system in [System::Windows, System::Mac, System::Other] {
            let variables = [
                ("POST_HOME", root("portable")),
                ("APPDATA", root("roaming")),
                ("HOME", root("home")),
            ];
            assert_eq!(
                find(system, &variables),
                Some(Home::at(&root("portable"))),
                "{system:?}"
            );
        }
    }

    #[test]
    fn uses_the_application_data_directory_on_windows() {
        let home = find(System::Windows, &[("APPDATA", root("roaming"))]);

        assert_eq!(home, Some(Home::at(&root("roaming").join("Post"))));
        assert_eq!(find(System::Windows, &[("HOME", root("home"))]), None);
    }

    #[test]
    fn uses_the_application_support_directory_on_macos() {
        let home = find(System::Mac, &[("HOME", root("home"))]);

        let expected = root("home")
            .join("Library")
            .join("Application Support")
            .join("Post");
        assert_eq!(home, Some(Home::at(&expected)));
        assert_eq!(find(System::Mac, &[]), None);
    }

    #[test]
    fn uses_the_xdg_directories_on_other_systems() {
        let home = find(System::Other, &[("HOME", root("home"))]).expect("a home");
        assert_eq!(home.settings, root("home").join(".config").join("post"));
        assert_eq!(
            home.data,
            root("home").join(".local").join("share").join("post")
        );

        let variables = [
            ("HOME", root("home")),
            ("XDG_CONFIG_HOME", root("settings")),
            ("XDG_DATA_HOME", root("data")),
        ];
        let home = find(System::Other, &variables).expect("a home");
        assert_eq!(home.settings, root("settings").join("post"));
        assert_eq!(home.data, root("data").join("post"));

        let without_home = [
            ("XDG_CONFIG_HOME", root("settings")),
            ("XDG_DATA_HOME", root("data")),
        ];
        assert!(find(System::Other, &without_home).is_some());
        assert_eq!(find(System::Other, &without_home[..1]), None);
        assert_eq!(find(System::Other, &[]), None);
    }

    #[test]
    fn ignores_a_directory_that_is_not_a_full_path() {
        let variables = [
            ("POST_HOME", PathBuf::from("relative")),
            ("XDG_CONFIG_HOME", PathBuf::from("relative")),
            ("HOME", root("home")),
        ];

        let home = find(System::Other, &variables).expect("a home");

        assert_eq!(home.settings, root("home").join(".config").join("post"));
    }

    #[test]
    fn knows_the_system_of_this_computer() {
        let expected = if cfg!(windows) {
            System::Windows
        } else if cfg!(target_os = "macos") {
            System::Mac
        } else {
            System::Other
        };

        assert_eq!(System::current(), expected);
    }
}
