use std::borrow::Cow;
use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::string::StringSliceAdditionalSpecFns;
#[cfg(verus_keep_ghost)]
use vstd::utf8::encode_utf8;

#[cfg(verus_keep_ghost)]
use crate::name::has_name_characters;
use crate::name::has_permitted_characters;

verus! {

pub open spec fn is_channel_name(bytes: Seq<u8>) -> bool {
    has_name_characters(bytes) && 2 <= bytes.len() <= 30
}

#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChannelName(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelNameError {
    InvalidCharacter,
    TooShort,
    TooLong,
}

impl ChannelName {
    pub const MIN_LENGTH: usize = 2;

    pub const MAX_LENGTH: usize = 30;

    #[verifier::type_invariant]
    spec fn is_a_channel_name(self) -> bool {
        is_channel_name(encode_utf8(self.0@))
    }

    pub closed spec fn view(self) -> Seq<char> {
        self.0@
    }

    pub fn parse(text: &str) -> (name: Result<Self, ChannelNameError>)
        ensures
            match name {
                Ok(name) => is_channel_name(text.spec_bytes()) && name@ == text@,
                Err(ChannelNameError::InvalidCharacter) => !has_name_characters(text.spec_bytes()),
                Err(ChannelNameError::TooShort) => has_name_characters(text.spec_bytes())
                    && text.spec_bytes().len() < 2,
                Err(ChannelNameError::TooLong) => has_name_characters(text.spec_bytes())
                    && text.spec_bytes().len() > 30,
            },
    {
        let bytes = text.as_bytes();
        if !has_permitted_characters(bytes) {
            return Err(ChannelNameError::InvalidCharacter);
        }
        if bytes.len() < Self::MIN_LENGTH {
            return Err(ChannelNameError::TooShort);
        }
        if bytes.len() > Self::MAX_LENGTH {
            return Err(ChannelNameError::TooLong);
        }
        Ok(Self(text.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> (text: &str)
        ensures
            text@ == self@,
            is_channel_name(text.spec_bytes()),
    {
        proof {
            use_type_invariant(self);
        }
        self.0.as_str()
    }
}

impl Clone for ChannelName {
    fn clone(&self) -> (name: Self)
        ensures
            name@ == self@,
    {
        proof {
            use_type_invariant(self);
        }
        Self(self.0.clone())
    }
}

} // verus!
impl fmt::Display for ChannelName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl fmt::Display for ChannelNameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCharacter => formatter.write_str(
                "a channel name can contain only a to z, 0 to 9, hyphen, and underscore",
            ),
            Self::TooShort => write!(
                formatter,
                "a channel name must have a minimum of {} characters",
                ChannelName::MIN_LENGTH
            ),
            Self::TooLong => write!(
                formatter,
                "a channel name can have a maximum of {} characters",
                ChannelName::MAX_LENGTH
            ),
        }
    }
}

impl std::error::Error for ChannelNameError {}

impl JsonSchema for ChannelName {
    fn schema_name() -> Cow<'static, str> {
        "ChannelName".into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "The name of a channel: 2 to 30 characters from a to z, 0 to 9, hyphen, and underscore.",
            "type": "string",
            "pattern": "^[a-z0-9_-]{2,30}$"
        })
    }
}

impl Serialize for ChannelName {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ChannelName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::{ChannelName, ChannelNameError};

    #[test]
    fn accepts_lower_case_letters_digits_hyphen_and_underscore() {
        let longest = "abcdefghij0123456789abcdefghij";
        assert_eq!(longest.len(), 30);
        for valid in ["general", "dev-2", "a_b", "az", "09", longest] {
            let name = ChannelName::parse(valid).expect(valid);
            assert_eq!(name.as_str(), valid);
            assert_eq!(name.to_string(), valid);
            assert_eq!(name.clone(), name);
        }
    }

    #[test]
    fn refuses_a_name_with_less_than_two_characters() {
        assert_eq!(ChannelName::parse(""), Err(ChannelNameError::TooShort));
        assert_eq!(ChannelName::parse("a"), Err(ChannelNameError::TooShort));
    }

    #[test]
    fn refuses_a_name_with_more_than_thirty_characters() {
        assert_eq!(
            ChannelName::parse("abcdefghij0123456789abcdefghijx"),
            Err(ChannelNameError::TooLong)
        );
    }

    #[test]
    fn refuses_a_character_that_is_not_permitted() {
        for invalid in [
            "General",
            "gen eral",
            "gen.eral",
            "general\n",
            "#general",
            "caf\u{e9}",
        ] {
            assert_eq!(
                ChannelName::parse(invalid),
                Err(ChannelNameError::InvalidCharacter),
                "{invalid:?}"
            );
        }
    }

    #[test]
    fn examines_the_characters_before_the_length() {
        assert_eq!(
            ChannelName::parse("#"),
            Err(ChannelNameError::InvalidCharacter)
        );
        assert_eq!(
            ChannelName::parse(&"X".repeat(31)),
            Err(ChannelNameError::InvalidCharacter)
        );
    }

    #[test]
    fn reads_and_writes_a_name_as_json_text() {
        let name: ChannelName = serde_json::from_str("\"general\"").expect("a valid name");
        assert_eq!(name.as_str(), "general");
        assert_eq!(serde_json::to_string(&name).expect("text"), "\"general\"");
        assert!(serde_json::from_str::<ChannelName>("\"#general\"").is_err());
        assert!(serde_json::from_str::<ChannelName>("7").is_err());
    }

    #[test]
    fn describes_each_error() {
        assert_eq!(
            ChannelNameError::InvalidCharacter.to_string(),
            "a channel name can contain only a to z, 0 to 9, hyphen, and underscore"
        );
        assert_eq!(
            ChannelNameError::TooShort.to_string(),
            "a channel name must have a minimum of 2 characters"
        );
        assert_eq!(
            ChannelNameError::TooLong.to_string(),
            "a channel name can have a maximum of 30 characters"
        );
    }
}
