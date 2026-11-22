use post_client::ProfileError;

use crate::i18n::{Strings, Text};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartProblem {
    NoHome,
    InUse,
    Damaged(&'static str),
    Files(String),
}

impl StartProblem {
    #[must_use]
    pub fn describe(&self, strings: &Strings) -> String {
        match self {
            Self::NoHome => strings.text(Text::StartNoHome),
            Self::InUse => strings.text(Text::StartProfileInUse),
            Self::Damaged(file) => strings.with(Text::StartProfileDamaged, "file", file),
            Self::Files(reason) => strings.with(Text::StartProfileFiles, "reason", reason),
        }
    }
}

impl From<ProfileError> for StartProblem {
    fn from(error: ProfileError) -> Self {
        match error {
            ProfileError::InUse => Self::InUse,
            ProfileError::Damaged(file) => Self::Damaged(file),
            other => Self::Files(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use post_client::{NameProblem, ProfileError};

    use super::StartProblem;
    use crate::i18n::{Language, Strings};

    #[test]
    fn makes_a_start_problem_from_an_error_of_the_profile() {
        let file = std::io::Error::other("no access");
        for (error, problem) in [
            (ProfileError::InUse, StartProblem::InUse),
            (ProfileError::Damaged("seed"), StartProblem::Damaged("seed")),
            (
                ProfileError::File(file),
                StartProblem::Files("a file of the profile has a problem: no access".to_owned()),
            ),
            (
                ProfileError::NotFound,
                StartProblem::Files("the profile does not exist".to_owned()),
            ),
            (
                ProfileError::Name(NameProblem::Empty),
                StartProblem::Files("a profile must have a name".to_owned()),
            ),
        ] {
            assert_eq!(StartProblem::from(error), problem);
        }
    }

    #[test]
    fn describes_each_start_problem_in_the_language_of_the_user() {
        let english = Strings::new(Language::English);
        let spanish = Strings::new(Language::Spanish);
        let files = StartProblem::Files("no access".to_owned());

        assert_eq!(
            StartProblem::NoHome.describe(&english),
            "Post cannot find a directory for its data. Set the environment variable POST_HOME to a directory."
        );
        assert_eq!(
            StartProblem::InUse.describe(&english),
            "This profile is open in a different window of Post. Close that window, then start Post again."
        );
        assert_eq!(
            StartProblem::Damaged("seed").describe(&english),
            "The seed file of this profile is not valid. Post cannot open the profile."
        );
        assert_eq!(
            files.describe(&english),
            "Post cannot use the files of this profile. no access"
        );
        assert_eq!(
            StartProblem::InUse.describe(&spanish),
            "Este perfil está abierto en otra ventana de Post. Cierra esa ventana y vuelve a iniciar Post."
        );
        assert_eq!(
            StartProblem::Damaged("seed").describe(&spanish),
            "El archivo seed de este perfil no es válido. Post no puede abrir el perfil."
        );
        assert_eq!(
            files.describe(&spanish),
            "Post no puede usar los archivos de este perfil. no access"
        );
        assert_ne!(
            StartProblem::NoHome.describe(&spanish),
            StartProblem::NoHome.describe(&english)
        );
    }
}
