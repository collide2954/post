use std::borrow::Cow;
use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Part(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartError;

impl Part {
    pub const MIN_LENGTH: usize = 2;
    pub const MAX_LENGTH: usize = 20;

    pub fn parse(text: &str) -> Result<Self, PartError> {
        let has_length = (Self::MIN_LENGTH..=Self::MAX_LENGTH).contains(&text.len());
        let has_letters = text
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'-');
        let has_ends = !text.starts_with('-') && !text.ends_with('-');
        if has_length && has_letters && has_ends {
            Ok(Self(text.to_owned()))
        } else {
            Err(PartError)
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Part {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl fmt::Display for PartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "the word for a part of the protocol has {} to {} lower-case letters and hyphens",
            Part::MIN_LENGTH,
            Part::MAX_LENGTH
        )
    }
}

impl std::error::Error for PartError {}

impl JsonSchema for Part {
    fn schema_name() -> Cow<'static, str> {
        "Part".into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "The word for an optional part of the protocol: 2 to 20 lower-case letters and hyphens.",
            "type": "string",
            "pattern": "^[a-z]([a-z-]{0,18}[a-z])$"
        })
    }
}

impl Serialize for Part {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Part {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::{Part, PartError};

    #[test]
    fn accepts_a_word_of_lower_case_letters_and_hyphens() {
        for valid in [
            "sealed",
            "voice",
            "ab",
            "link-previews",
            "abcdefghijklmnopqrst",
        ] {
            let part = Part::parse(valid).expect(valid);
            assert_eq!(part.as_str(), valid);
            assert_eq!(part.to_string(), valid);
        }
    }

    #[test]
    fn refuses_a_text_that_is_not_such_a_word() {
        for invalid in [
            "",
            "a",
            "abcdefghijklmnopqrstu",
            "Sealed",
            "voice2",
            "two words",
            "-voice",
            "voice-",
            "voice_chat",
            "caf\u{e9}",
        ] {
            assert_eq!(Part::parse(invalid), Err(PartError), "{invalid:?}");
        }
    }

    #[test]
    fn sorts_as_text() {
        let mut parts = [
            Part::parse("voice").expect("valid"),
            Part::parse("sealed").expect("valid"),
        ];
        parts.sort();
        assert_eq!(parts[0].as_str(), "sealed");
    }

    #[test]
    fn is_a_json_string() {
        let part = Part::parse("sealed").expect("valid");
        let json = serde_json::to_string(&part).expect("a part becomes JSON");
        assert_eq!(json, "\"sealed\"");
        assert_eq!(serde_json::from_str::<Part>(&json).expect("valid"), part);
        assert!(serde_json::from_str::<Part>("\"Sealed\"").is_err());
    }

    #[test]
    fn describes_the_error() {
        assert_eq!(
            PartError.to_string(),
            "the word for a part of the protocol has 2 to 20 lower-case letters and hyphens"
        );
    }
}
