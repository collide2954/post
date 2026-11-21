use std::fmt;

pub const MAX_TEXT_CHARACTERS: usize = 4000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextError {
    Empty,
    TooLong,
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

#[cfg(test)]
mod tests {
    use super::{MAX_TEXT_CHARACTERS, TextError, check_text};

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
