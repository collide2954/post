use std::fmt;

pub const MAX_TEXT_CHARACTERS: usize = 4000;
pub const MAX_TOPIC_CHARACTERS: usize = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextError {
    Empty,
    TooLong,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopicError {
    ControlCharacter,
    TooLong,
}

pub fn check_topic(topic: &str) -> Result<(), TopicError> {
    if topic.chars().any(char::is_control) {
        return Err(TopicError::ControlCharacter);
    }
    if topic.chars().count() > MAX_TOPIC_CHARACTERS {
        return Err(TopicError::TooLong);
    }
    Ok(())
}

pub fn check_text(text: &str) -> Result<(), TextError> {
    if text.trim().is_empty() {
        return Err(TextError::Empty);
    }
    if text.chars().count() > MAX_TEXT_CHARACTERS {
        return Err(TextError::TooLong);
    }
    Ok(())
}

impl fmt::Display for TextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("a message must contain text"),
            Self::TooLong => write!(
                formatter,
                "a message can have a maximum of {MAX_TEXT_CHARACTERS} characters"
            ),
        }
    }
}

impl std::error::Error for TextError {}

impl fmt::Display for TopicError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ControlCharacter => formatter.write_str("a topic must be one line of text"),
            Self::TooLong => write!(
                formatter,
                "a topic can have a maximum of {MAX_TOPIC_CHARACTERS} characters"
            ),
        }
    }
}

impl std::error::Error for TopicError {}

#[cfg(test)]
mod tests {
    use super::{
        MAX_TEXT_CHARACTERS, MAX_TOPIC_CHARACTERS, TextError, TopicError, check_text, check_topic,
    };

    #[test]
    fn accepts_a_topic_of_one_line_and_an_empty_topic() {
        assert_eq!(check_topic(""), Ok(()));
        assert_eq!(check_topic("Plans for the week"), Ok(()));
        assert_eq!(check_topic(&"\u{e9}".repeat(MAX_TOPIC_CHARACTERS)), Ok(()));
        assert_eq!(MAX_TOPIC_CHARACTERS, 300);
    }

    #[test]
    fn refuses_a_topic_that_is_too_long_or_has_more_than_one_line() {
        assert_eq!(
            check_topic(&"x".repeat(MAX_TOPIC_CHARACTERS + 1)),
            Err(TopicError::TooLong)
        );
        for invalid in ["two\nlines", "a\ttab", "bell\u{7}", "end\r"] {
            assert_eq!(
                check_topic(invalid),
                Err(TopicError::ControlCharacter),
                "{invalid:?}"
            );
        }
        assert_eq!(
            TopicError::ControlCharacter.to_string(),
            "a topic must be one line of text"
        );
        assert_eq!(
            TopicError::TooLong.to_string(),
            "a topic can have a maximum of 300 characters"
        );
    }

    #[test]
    fn accepts_normal_text() {
        assert_eq!(check_text("Hello"), Ok(()));
        assert_eq!(check_text("  two lines\n  with spaces  "), Ok(()));
        assert_eq!(check_text("x"), Ok(()));
    }

    #[test]
    fn refuses_text_without_a_visible_character() {
        for empty in ["", " ", "\n", " \t\r\n "] {
            assert_eq!(check_text(empty), Err(TextError::Empty), "{empty:?}");
        }
    }

    #[test]
    fn counts_characters_and_not_bytes() {
        let longest = "\u{e9}".repeat(MAX_TEXT_CHARACTERS);
        assert_eq!(check_text(&longest), Ok(()));

        let too_long = "\u{e9}".repeat(MAX_TEXT_CHARACTERS + 1);
        assert_eq!(check_text(&too_long), Err(TextError::TooLong));
    }

    #[test]
    fn has_a_maximum_of_4000_characters() {
        assert_eq!(MAX_TEXT_CHARACTERS, 4000);
    }

    #[test]
    fn describes_each_error() {
        assert_eq!(TextError::Empty.to_string(), "a message must contain text");
        assert_eq!(
            TextError::TooLong.to_string(),
            "a message can have a maximum of 4000 characters"
        );
    }
}
