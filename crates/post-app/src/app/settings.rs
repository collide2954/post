use super::Effect;
use crate::i18n::Language;
use crate::settings::{self, Clock, Settings};

#[derive(Debug, Clone)]
pub enum Message {
    LanguageSelected(Option<Language>),
    ClockSelected(Clock),
    SecondsToggled(bool),
    Larger,
    Smaller,
    DefaultSize,
    ConsoleToggled(bool),
    ConsoleFlipped,
    FileChanged(String),
    NotSaved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    File(settings::Problem),
    NotSaved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Store {
    values: Settings,
    problem: Option<Problem>,
}

impl Store {
    #[must_use]
    pub fn values(&self) -> &Settings {
        &self.values
    }

    #[must_use]
    pub fn problem(&self) -> Option<Problem> {
        self.problem
    }

    pub fn update(&mut self, message: Message) -> Effect {
        let wanted = match message {
            Message::FileChanged(text) => {
                self.read(&text);
                return Effect::None;
            }
            Message::NotSaved => {
                self.problem = Some(Problem::NotSaved);
                return Effect::None;
            }
            Message::LanguageSelected(language) => Settings {
                language,
                ..self.values
            },
            Message::ClockSelected(clock) => Settings {
                clock,
                ..self.values
            },
            Message::SecondsToggled(seconds) => Settings {
                seconds,
                ..self.values
            },
            Message::Larger => self.values.with_larger_text(),
            Message::Smaller => self.values.with_smaller_text(),
            Message::DefaultSize => Settings {
                text_size: Settings::DEFAULT_TEXT_SIZE,
                ..self.values
            },
            Message::ConsoleToggled(console) => Settings {
                console,
                ..self.values
            },
            Message::ConsoleFlipped => Settings {
                console: !self.values.console,
                ..self.values
            },
        };
        if wanted == self.values {
            return Effect::None;
        }
        self.values = wanted;
        self.problem = None;
        Effect::SaveSettings(wanted)
    }

    fn read(&mut self, text: &str) {
        let (values, problem) = Settings::read(text);
        if problem != Some(settings::Problem::Syntax) {
            self.values = values;
        }
        self.problem = problem.map(Problem::File);
    }
}

#[cfg(test)]
mod tests {
    use super::{Message, Problem, Store};
    use crate::app::Effect;
    use crate::i18n::Language;
    use crate::settings::{self, Clock, Settings};

    fn changed(store: &mut Store, message: Message) -> Settings {
        match store.update(message) {
            Effect::SaveSettings(settings) => {
                assert_eq!(&settings, store.values());
                settings
            }
            other => panic!("the store does not ask for a save: {other:?}"),
        }
    }

    #[test]
    fn starts_with_the_default_settings_and_no_problem() {
        let store = Store::default();

        assert_eq!(store.values(), &Settings::default());
        assert_eq!(store.problem(), None);
    }

    #[test]
    fn changes_each_setting_and_asks_for_a_save() {
        let mut store = Store::default();

        let language = Message::LanguageSelected(Some(Language::Spanish));
        assert_eq!(
            changed(&mut store, language).language,
            Some(Language::Spanish)
        );
        let clock = Message::ClockSelected(Clock::Hours12);
        assert_eq!(changed(&mut store, clock).clock, Clock::Hours12);
        assert!(changed(&mut store, Message::SecondsToggled(true)).seconds);
        assert_eq!(changed(&mut store, Message::Larger).text_size, 110);
        assert_eq!(changed(&mut store, Message::Larger).text_size, 120);
        assert_eq!(changed(&mut store, Message::Smaller).text_size, 110);
        assert_eq!(changed(&mut store, Message::DefaultSize).text_size, 100);
        assert!(changed(&mut store, Message::ConsoleToggled(true)).console);

        assert_eq!(
            store.values(),
            &Settings {
                language: Some(Language::Spanish),
                clock: Clock::Hours12,
                seconds: true,
                text_size: 100,
                console: true,
            }
        );
        let system = Message::LanguageSelected(None);
        assert_eq!(changed(&mut store, system).language, None);
        assert!(!changed(&mut store, Message::ConsoleFlipped).console);
        assert!(changed(&mut store, Message::ConsoleFlipped).console);
    }

    #[test]
    fn does_not_ask_for_a_save_when_no_value_changes() {
        let mut store = Store::default();

        for message in [
            Message::LanguageSelected(None),
            Message::ClockSelected(Clock::Hours24),
            Message::SecondsToggled(false),
            Message::DefaultSize,
            Message::ConsoleToggled(false),
        ] {
            assert_eq!(store.update(message), Effect::None);
        }
        for _ in 0..2 {
            changed(&mut store, Message::Smaller);
        }
        assert_eq!(store.values().text_size, 80);
        assert_eq!(store.update(Message::Smaller), Effect::None);
    }

    #[test]
    fn takes_the_settings_from_the_text_of_the_file() {
        let mut store = Store::default();

        let text = "clock = 12\ntext-size = 150\n".to_owned();
        assert_eq!(store.update(Message::FileChanged(text)), Effect::None);

        assert_eq!(store.values().clock, Clock::Hours12);
        assert_eq!(store.values().text_size, 150);
        assert_eq!(store.problem(), None);

        store.update(Message::FileChanged(String::new()));
        assert_eq!(store.values(), &Settings::default());
    }

    #[test]
    fn keeps_its_settings_when_the_file_is_not_toml() {
        let mut store = Store::default();
        store.update(Message::FileChanged("clock = 12\n".to_owned()));

        store.update(Message::FileChanged("clock = = 24\n".to_owned()));

        assert_eq!(store.values().clock, Clock::Hours12);
        assert_eq!(
            store.problem(),
            Some(Problem::File(settings::Problem::Syntax))
        );

        store.update(Message::FileChanged("clock = 24\n".to_owned()));
        assert_eq!(store.values().clock, Clock::Hours24);
        assert_eq!(store.problem(), None);
    }

    #[test]
    fn uses_the_default_for_a_value_that_is_not_valid() {
        let mut store = Store::default();
        store.update(Message::FileChanged("text-size = 150\n".to_owned()));

        let text = "text-size = 500\nclock = 12\n".to_owned();
        store.update(Message::FileChanged(text));

        assert_eq!(store.values().text_size, 100);
        assert_eq!(store.values().clock, Clock::Hours12);
        assert_eq!(
            store.problem(),
            Some(Problem::File(settings::Problem::Value("text-size")))
        );
    }

    #[test]
    fn remembers_that_the_file_is_not_saved_until_the_next_change() {
        let mut store = Store::default();
        changed(&mut store, Message::Larger);

        assert_eq!(store.update(Message::NotSaved), Effect::None);
        assert_eq!(store.problem(), Some(Problem::NotSaved));
        assert_eq!(store.values().text_size, 110);

        changed(&mut store, Message::Larger);
        assert_eq!(store.problem(), None);
    }
}
