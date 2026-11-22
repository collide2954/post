use post_client::{NameProblem, ProfileEntry, ProfileError, ProfileId};

use super::Effect;
use crate::i18n::Text;

#[derive(Debug, Clone)]
pub enum Message {
    Listed(Vec<ProfileEntry>, ProfileId),
    Failed(Problem),
    NewNameChanged(String),
    Create,
    Rename(ProfileId),
    Delete(ProfileId),
    EditChanged(String),
    Confirm,
    Cancel,
    Switch(ProfileId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    Name(NameProblem),
    InUse,
    Files,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    Rename { id: ProfileId, name: String },
    Delete { id: ProfileId, typed: String },
}

#[derive(Debug, Default)]
pub struct Profiles {
    list: Vec<ProfileEntry>,
    current: Option<ProfileId>,
    new_name: String,
    edit: Option<Edit>,
    problem: Option<Problem>,
}

impl From<&ProfileError> for Problem {
    fn from(error: &ProfileError) -> Self {
        match error {
            ProfileError::Name(problem) => Self::Name(*problem),
            ProfileError::InUse => Self::InUse,
            ProfileError::File(_) | ProfileError::NotFound | ProfileError::Damaged(_) => {
                Self::Files
            }
        }
    }
}

impl Problem {
    #[must_use]
    pub fn text(self) -> Text {
        match self {
            Self::Name(NameProblem::Empty) => Text::ProfilesProblemEmpty,
            Self::Name(NameProblem::TooLong) => Text::ProfilesProblemLong,
            Self::Name(NameProblem::Character) => Text::ProfilesProblemCharacter,
            Self::Name(NameProblem::Taken) => Text::ProfilesProblemTaken,
            Self::InUse => Text::ProfilesProblemInUse,
            Self::Files => Text::ProfilesProblemFiles,
        }
    }
}

impl Profiles {
    #[must_use]
    pub fn list(&self) -> &[ProfileEntry] {
        &self.list
    }

    #[must_use]
    pub fn current(&self) -> Option<ProfileId> {
        self.current
    }

    #[must_use]
    pub fn new_name(&self) -> &str {
        &self.new_name
    }

    #[must_use]
    pub fn edit(&self) -> Option<&Edit> {
        self.edit.as_ref()
    }

    #[must_use]
    pub fn problem(&self) -> Option<Problem> {
        self.problem
    }

    #[must_use]
    pub fn can_delete(&self) -> bool {
        match &self.edit {
            Some(Edit::Delete { id, typed }) => self
                .find(*id)
                .is_some_and(|profile| profile.name == typed.trim()),
            Some(Edit::Rename { .. }) | None => false,
        }
    }

    pub fn update(&mut self, message: Message) -> Effect {
        match message {
            Message::Listed(list, current) => {
                let wanted = self.new_name.trim();
                if list.iter().any(|profile| profile.name == wanted) {
                    self.new_name.clear();
                }
                self.list = list;
                self.current = Some(current);
                self.edit = None;
                self.problem = None;
            }
            Message::Failed(problem) => self.problem = Some(problem),
            Message::NewNameChanged(name) => {
                self.new_name = name;
                self.problem = None;
            }
            Message::Create => return Effect::CreateProfile(self.new_name.clone()),
            Message::Rename(id) => {
                if let Some(profile) = self.find(id) {
                    let name = profile.name.clone();
                    return self.start(Edit::Rename { id, name });
                }
            }
            Message::Delete(id) => {
                if Some(id) != self.current && self.find(id).is_some() {
                    let typed = String::new();
                    return self.start(Edit::Delete { id, typed });
                }
            }
            Message::EditChanged(text) => {
                match &mut self.edit {
                    Some(Edit::Rename { name, .. }) => *name = text,
                    Some(Edit::Delete { typed, .. }) => *typed = text,
                    None => {}
                }
                self.problem = None;
            }
            Message::Confirm => match &self.edit {
                Some(Edit::Rename { id, name }) => {
                    return Effect::RenameProfile(*id, name.clone());
                }
                Some(Edit::Delete { id, .. }) if self.can_delete() => {
                    return Effect::DeleteProfile(*id);
                }
                Some(Edit::Delete { .. }) | None => {}
            },
            Message::Cancel => {
                self.cancel();
            }
            Message::Switch(id) => {
                if Some(id) != self.current && self.find(id).is_some() {
                    return Effect::SwitchProfile(id);
                }
            }
        }
        Effect::None
    }

    pub fn cancel(&mut self) -> bool {
        self.problem = None;
        self.edit.take().is_some()
    }

    fn start(&mut self, edit: Edit) -> Effect {
        self.edit = Some(edit);
        self.problem = None;
        Effect::FocusPanel
    }

    fn find(&self, id: ProfileId) -> Option<&ProfileEntry> {
        self.list.iter().find(|profile| profile.id == id)
    }
}

#[cfg(test)]
mod tests {
    use post_client::{NameProblem, ProfileEntry, ProfileError, ProfileId};

    use super::{Edit, Message, Problem, Profiles};
    use crate::app::Effect;
    use crate::i18n::Text;

    fn id(number: u32) -> ProfileId {
        ProfileId::from_number(number)
    }

    fn entry(number: u32, name: &str) -> ProfileEntry {
        ProfileEntry {
            id: id(number),
            name: name.to_owned(),
        }
    }

    fn listed() -> Profiles {
        let mut profiles = Profiles::default();
        let list = vec![entry(1, "main"), entry(2, "work")];
        assert_eq!(profiles.update(Message::Listed(list, id(1))), Effect::None);
        profiles
    }

    #[test]
    fn starts_with_no_profiles() {
        let profiles = Profiles::default();

        assert_eq!(profiles.list(), []);
        assert_eq!(profiles.current(), None);
        assert_eq!(profiles.new_name(), "");
        assert_eq!(profiles.edit(), None);
        assert_eq!(profiles.problem(), None);
        assert!(!profiles.can_delete());
    }

    #[test]
    fn takes_the_list_and_the_profile_of_this_window() {
        let profiles = listed();

        assert_eq!(profiles.list(), [entry(1, "main"), entry(2, "work")]);
        assert_eq!(profiles.current(), Some(id(1)));
    }

    #[test]
    fn asks_for_a_new_profile_with_the_name_in_the_field() {
        let mut profiles = listed();
        profiles.update(Message::NewNameChanged(" garden ".to_owned()));
        assert_eq!(profiles.new_name(), " garden ");

        assert_eq!(
            profiles.update(Message::Create),
            Effect::CreateProfile(" garden ".to_owned())
        );
        assert_eq!(profiles.new_name(), " garden ");

        let list = vec![entry(1, "main"), entry(2, "work"), entry(3, "garden")];
        profiles.update(Message::Listed(list, id(1)));
        assert_eq!(profiles.new_name(), "");
        assert_eq!(profiles.list().len(), 3);
    }

    #[test]
    fn keeps_the_name_in_the_field_when_the_profile_is_not_made() {
        let mut profiles = listed();
        profiles.update(Message::NewNameChanged("Work".to_owned()));
        profiles.update(Message::Create);

        let taken = Problem::Name(NameProblem::Taken);
        assert_eq!(profiles.update(Message::Failed(taken)), Effect::None);

        assert_eq!(profiles.problem(), Some(taken));
        assert_eq!(profiles.new_name(), "Work");

        profiles.update(Message::NewNameChanged("Works".to_owned()));
        assert_eq!(profiles.problem(), None);
    }

    #[test]
    fn changes_the_name_of_a_profile() {
        let mut profiles = listed();

        assert_eq!(profiles.update(Message::Rename(id(2))), Effect::FocusPanel);
        assert_eq!(
            profiles.edit(),
            Some(&Edit::Rename {
                id: id(2),
                name: "work".to_owned(),
            })
        );
        profiles.update(Message::EditChanged("office".to_owned()));

        assert_eq!(
            profiles.update(Message::Confirm),
            Effect::RenameProfile(id(2), "office".to_owned())
        );
        assert!(profiles.edit().is_some());

        let list = vec![entry(1, "main"), entry(2, "office")];
        profiles.update(Message::Listed(list, id(1)));
        assert_eq!(profiles.edit(), None);
        assert_eq!(profiles.update(Message::Rename(id(1))), Effect::FocusPanel);
    }

    #[test]
    fn deletes_a_profile_only_after_the_user_types_its_name() {
        let mut profiles = listed();

        assert_eq!(profiles.update(Message::Delete(id(2))), Effect::FocusPanel);
        assert_eq!(
            profiles.edit(),
            Some(&Edit::Delete {
                id: id(2),
                typed: String::new(),
            })
        );
        assert!(!profiles.can_delete());
        assert_eq!(profiles.update(Message::Confirm), Effect::None);

        for wrong in ["wor", "Work", "main", "work2"] {
            profiles.update(Message::EditChanged(wrong.to_owned()));
            assert!(!profiles.can_delete(), "{wrong}");
            assert_eq!(profiles.update(Message::Confirm), Effect::None, "{wrong}");
        }

        profiles.update(Message::EditChanged(" work ".to_owned()));
        assert!(profiles.can_delete());
        assert_eq!(
            profiles.update(Message::Confirm),
            Effect::DeleteProfile(id(2))
        );
    }

    #[test]
    fn does_not_delete_the_profile_of_this_window() {
        let mut profiles = listed();

        assert_eq!(profiles.update(Message::Delete(id(1))), Effect::None);

        assert_eq!(profiles.edit(), None);
    }

    #[test]
    fn stops_a_change_when_the_user_cancels() {
        let mut profiles = listed();
        profiles.update(Message::Rename(id(2)));
        profiles.update(Message::Failed(Problem::Name(NameProblem::Empty)));

        assert_eq!(profiles.update(Message::Cancel), Effect::None);

        assert_eq!(profiles.edit(), None);
        assert_eq!(profiles.problem(), None);
        assert!(!profiles.cancel());
        profiles.update(Message::Delete(id(2)));
        assert!(profiles.cancel());
    }

    #[test]
    fn shows_a_problem_until_the_user_changes_the_text() {
        let mut profiles = listed();
        profiles.update(Message::Rename(id(2)));

        profiles.update(Message::Failed(Problem::Name(NameProblem::TooLong)));
        assert_eq!(
            profiles.problem(),
            Some(Problem::Name(NameProblem::TooLong))
        );

        profiles.update(Message::EditChanged("w".to_owned()));
        assert_eq!(profiles.problem(), None);
    }

    #[test]
    fn opens_a_different_profile_in_this_window() {
        let mut profiles = listed();

        assert_eq!(
            profiles.update(Message::Switch(id(2))),
            Effect::SwitchProfile(id(2))
        );
        assert_eq!(profiles.update(Message::Switch(id(1))), Effect::None);
        assert_eq!(profiles.update(Message::Switch(id(9))), Effect::None);
    }

    #[test]
    fn ignores_a_change_of_a_profile_that_is_not_in_the_list() {
        let mut profiles = listed();

        assert_eq!(profiles.update(Message::Rename(id(9))), Effect::None);
        assert_eq!(profiles.update(Message::Delete(id(9))), Effect::None);
        assert_eq!(profiles.update(Message::Confirm), Effect::None);
        assert_eq!(
            profiles.update(Message::EditChanged("x".to_owned())),
            Effect::None
        );
        assert_eq!(profiles.edit(), None);
    }

    #[test]
    fn each_problem_has_a_text() {
        let problems = [
            (
                Problem::Name(NameProblem::Empty),
                Text::ProfilesProblemEmpty,
            ),
            (
                Problem::Name(NameProblem::TooLong),
                Text::ProfilesProblemLong,
            ),
            (
                Problem::Name(NameProblem::Character),
                Text::ProfilesProblemCharacter,
            ),
            (
                Problem::Name(NameProblem::Taken),
                Text::ProfilesProblemTaken,
            ),
            (Problem::InUse, Text::ProfilesProblemInUse),
            (Problem::Files, Text::ProfilesProblemFiles),
        ];
        for (problem, text) in problems {
            assert_eq!(problem.text(), text);
        }
    }

    #[test]
    fn makes_a_problem_from_an_error_of_the_profiles() {
        let file = ProfileError::File(std::io::Error::other("no access"));
        let errors = [
            (
                ProfileError::Name(NameProblem::Character),
                Problem::Name(NameProblem::Character),
            ),
            (ProfileError::InUse, Problem::InUse),
            (file, Problem::Files),
            (ProfileError::NotFound, Problem::Files),
            (ProfileError::Damaged("seed"), Problem::Files),
        ];
        for (error, problem) in errors {
            assert_eq!(Problem::from(&error), problem);
        }
    }
}
