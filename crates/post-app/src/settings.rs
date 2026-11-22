use toml_edit::{DocumentMut, Item, Value};

use crate::i18n::Language;

pub const TEMPLATE: &str = "\
# The settings of this Post profile.
# Post reads this file again when you change it.

# The language of the interface: \"system\", \"en\", or \"es-419\".
language = \"system\"

# The clock of the timeline: 24 or 12 hours.
clock = 24

# Show the seconds of each time in the timeline.
seconds = false

# The size of the text in percent, from 80 to 200.
text-size = 100

# Show the protocol console with the F12 key. This setting is for developers.
console = false
";

const LANGUAGE: &str = "language";
const CLOCK: &str = "clock";
const SECONDS: &str = "seconds";
const TEXT_SIZE: &str = "text-size";
const CONSOLE: &str = "console";
const SYSTEM: &str = "system";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub language: Option<Language>,
    pub clock: Clock,
    pub seconds: bool,
    pub text_size: u16,
    pub console: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Clock {
    #[default]
    Hours24,
    Hours12,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    Syntax,
    Value(&'static str),
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: None,
            clock: Clock::Hours24,
            seconds: false,
            text_size: Self::DEFAULT_TEXT_SIZE,
            console: false,
        }
    }
}

impl Settings {
    pub const MIN_TEXT_SIZE: u16 = 80;
    pub const MAX_TEXT_SIZE: u16 = 200;
    pub const DEFAULT_TEXT_SIZE: u16 = 100;
    pub const TEXT_SIZE_STEP: u16 = 10;

    #[must_use]
    pub fn read(text: &str) -> (Self, Option<Problem>) {
        let Ok(document) = text.parse::<DocumentMut>() else {
            return (Self::default(), Some(Problem::Syntax));
        };
        let mut settings = Self::default();
        let mut problem = None;
        let mut read = |key: &'static str, apply: &mut dyn FnMut(&Item) -> bool| {
            if let Some(item) = document.get(key)
                && !apply(item)
                && problem.is_none()
            {
                problem = Some(Problem::Value(key));
            }
        };
        read(LANGUAGE, &mut |item| match item.as_str() {
            Some(SYSTEM) => true,
            Some(tag) => Language::from_tag(tag)
                .map(|language| settings.language = Some(language))
                .is_some(),
            None => false,
        });
        read(CLOCK, &mut |item| match item.as_integer() {
            Some(24) => true,
            Some(12) => {
                settings.clock = Clock::Hours12;
                true
            }
            _ => false,
        });
        read(SECONDS, &mut |item| {
            item.as_bool()
                .map(|seconds| settings.seconds = seconds)
                .is_some()
        });
        read(TEXT_SIZE, &mut |item| {
            item.as_integer()
                .and_then(|size| u16::try_from(size).ok())
                .filter(|size| (Self::MIN_TEXT_SIZE..=Self::MAX_TEXT_SIZE).contains(size))
                .map(|size| settings.text_size = size)
                .is_some()
        });
        read(CONSOLE, &mut |item| {
            item.as_bool()
                .map(|console| settings.console = console)
                .is_some()
        });
        (settings, problem)
    }

    #[must_use]
    pub fn write_into(&self, text: &str) -> String {
        let mut document = text
            .parse::<DocumentMut>()
            .unwrap_or_else(|_| TEMPLATE.parse().expect("the template is valid TOML"));
        let language = self.language.map_or(SYSTEM, Language::tag);
        put(&mut document, LANGUAGE, language.into());
        let clock = match self.clock {
            Clock::Hours24 => 24,
            Clock::Hours12 => 12,
        };
        put(&mut document, CLOCK, i64::from(clock).into());
        put(&mut document, SECONDS, self.seconds.into());
        put(&mut document, TEXT_SIZE, i64::from(self.text_size).into());
        put(&mut document, CONSOLE, self.console.into());
        document.to_string()
    }

    #[must_use]
    pub fn scale(&self) -> f32 {
        f32::from(self.text_size) / 100.0
    }

    #[must_use]
    pub fn with_larger_text(self) -> Self {
        let text_size = (self.text_size + Self::TEXT_SIZE_STEP).min(Self::MAX_TEXT_SIZE);
        Self { text_size, ..self }
    }

    #[must_use]
    pub fn with_smaller_text(self) -> Self {
        let text_size = self
            .text_size
            .saturating_sub(Self::TEXT_SIZE_STEP)
            .max(Self::MIN_TEXT_SIZE);
        Self { text_size, ..self }
    }
}

fn put(document: &mut DocumentMut, key: &str, new: Value) {
    match document.get_mut(key).and_then(Item::as_value_mut) {
        Some(old) => {
            let decor = old.decor().clone();
            *old = new;
            *old.decor_mut() = decor;
        }
        None => document[key] = Item::Value(new),
    }
}

#[cfg(test)]
mod tests {
    use super::{Clock, Problem, Settings, TEMPLATE};
    use crate::i18n::Language;

    fn changed() -> Settings {
        Settings {
            language: Some(Language::Spanish),
            clock: Clock::Hours12,
            seconds: true,
            text_size: 150,
            console: true,
        }
    }

    #[test]
    fn the_template_gives_the_default_settings() {
        let (settings, problem) = Settings::read(TEMPLATE);

        assert_eq!(settings, Settings::default());
        assert_eq!(problem, None);
        assert_eq!(settings.language, None);
        assert_eq!(settings.clock, Clock::Hours24);
        assert!(!settings.seconds);
        assert_eq!(settings.text_size, 100);
        assert!(!settings.console);
    }

    #[test]
    fn an_empty_file_gives_the_default_settings() {
        assert_eq!(Settings::read(""), (Settings::default(), None));
        assert_eq!(
            Settings::read("# only a comment\n"),
            (Settings::default(), None)
        );
    }

    #[test]
    fn reads_each_setting() {
        let text =
            "language = \"es-419\"\nclock = 12\nseconds = true\ntext-size = 150\nconsole = true\n";

        assert_eq!(Settings::read(text), (changed(), None));
        assert_eq!(
            Settings::read("language = \"en\"\n").0.language,
            Some(Language::English)
        );
        assert_eq!(Settings::read("text-size = 80\n").0.text_size, 80);
        assert_eq!(Settings::read("text-size = 200\n").0.text_size, 200);
    }

    #[test]
    fn ignores_a_setting_that_it_does_not_know() {
        let text = "accent = \"blue\"\ntext-size = 120\n\n[keys]\nsend = \"enter\"\n";

        let (settings, problem) = Settings::read(text);

        assert_eq!(settings.text_size, 120);
        assert_eq!(problem, None);
    }

    #[test]
    fn uses_the_default_for_a_value_that_is_not_valid() {
        for (text, key) in [
            ("language = \"fr\"\n", "language"),
            ("language = 7\n", "language"),
            ("clock = 13\n", "clock"),
            ("clock = \"24\"\n", "clock"),
            ("seconds = \"yes\"\n", "seconds"),
            ("text-size = 79\n", "text-size"),
            ("text-size = 201\n", "text-size"),
            ("text-size = -100\n", "text-size"),
            ("text-size = 100.5\n", "text-size"),
            ("console = 1\n", "console"),
        ] {
            let (settings, problem) = Settings::read(text);

            assert_eq!(settings, Settings::default(), "{text}");
            assert_eq!(problem, Some(Problem::Value(key)), "{text}");
        }
    }

    #[test]
    fn keeps_the_valid_values_of_a_file_with_one_problem() {
        let text = "clock = 12\ntext-size = 500\nseconds = 3\nconsole = true\n";

        let (settings, problem) = Settings::read(text);

        assert_eq!(settings.clock, Clock::Hours12);
        assert_eq!(settings.text_size, 100);
        assert!(settings.console);
        assert_eq!(problem, Some(Problem::Value("seconds")));
    }

    #[test]
    fn tells_that_a_file_is_not_toml() {
        let (settings, problem) = Settings::read("clock = = 12\n");

        assert_eq!(settings, Settings::default());
        assert_eq!(problem, Some(Problem::Syntax));
    }

    #[test]
    fn writes_the_values_and_keeps_the_comments_and_the_sequence() {
        let text = changed().write_into(TEMPLATE);

        let expected = TEMPLATE
            .replace("language = \"system\"", "language = \"es-419\"")
            .replace("clock = 24", "clock = 12")
            .replace("seconds = false", "seconds = true")
            .replace("text-size = 100", "text-size = 150")
            .replace("console = false", "console = true");
        assert_eq!(text, expected);
        assert_eq!(Settings::read(&text), (changed(), None));
    }

    #[test]
    fn keeps_what_the_user_wrote_in_the_file() {
        let text =
            "# my settings\nconsole = true   # for my bot\n\naccent = \"blue\"\nclock = 12\n";

        let written = Settings::default().write_into(text);

        assert!(written.starts_with(
            "# my settings\nconsole = false   # for my bot\n\naccent = \"blue\"\nclock = 24\n"
        ));
        assert!(written.contains("language = \"system\"\n"));
        assert!(written.contains("text-size = 100\n"));
        assert!(written.contains("seconds = false\n"));
        assert_eq!(Settings::read(&written), (Settings::default(), None));
    }

    #[test]
    fn writes_the_template_when_the_file_is_not_toml() {
        let written = changed().write_into("clock = = 12\n");

        assert!(written.starts_with("# The settings of this Post profile.\n"));
        assert_eq!(Settings::read(&written), (changed(), None));
    }

    #[test]
    fn writes_a_file_that_gives_the_same_settings_again() {
        let written = Settings::default().write_into("");

        assert_eq!(Settings::read(&written), (Settings::default(), None));
        assert_eq!(Settings::default().write_into(TEMPLATE), TEMPLATE);
    }

    #[test]
    fn changes_the_text_size_in_steps_inside_the_limits() {
        let settings = Settings::default();

        assert_eq!(settings.with_larger_text().text_size, 110);
        assert_eq!(settings.with_smaller_text().text_size, 90);
        let largest = Settings {
            text_size: 195,
            ..settings
        };
        assert_eq!(largest.with_larger_text().text_size, 200);
        assert_eq!(largest.with_larger_text().with_larger_text().text_size, 200);
        let smallest = Settings {
            text_size: 85,
            ..settings
        };
        assert_eq!(smallest.with_smaller_text().text_size, 80);
        assert_eq!(
            smallest.with_smaller_text().with_smaller_text().text_size,
            80
        );
    }

    #[test]
    fn gives_the_scale_of_the_interface() {
        let settings = Settings {
            text_size: 150,
            ..Settings::default()
        };

        assert!((settings.scale() - 1.5).abs() < f32::EPSILON);
        assert!((Settings::default().scale() - 1.0).abs() < f32::EPSILON);
    }
}
