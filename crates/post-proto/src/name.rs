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

use crate::ascii::{is_digit, is_lower};

verus! {

pub open spec fn is_name_character(byte: u8) -> bool {
    is_lower(byte) || is_digit(byte) || byte == b'-' || byte == b'_'
}

pub open spec fn has_name_characters(bytes: Seq<u8>) -> bool {
    forall|i: int| 0 <= i < bytes.len() ==> is_name_character(#[trigger] bytes[i])
}

pub open spec fn is_name(bytes: Seq<u8>) -> bool {
    has_name_characters(bytes) && 2 <= bytes.len() <= 20
}

#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Name(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameError {
    InvalidCharacter,
    TooShort,
    TooLong,
}

impl Name {
    pub const MIN_LENGTH: usize = 2;

    pub const MAX_LENGTH: usize = 20;

    #[verifier::type_invariant]
    spec fn is_a_name(self) -> bool {
        is_name(encode_utf8(self.0@))
    }

    pub closed spec fn view(self) -> Seq<char> {
        self.0@
    }

    pub fn parse(text: &str) -> (name: Result<Self, NameError>)
        ensures
            match name {
                Ok(name) => is_name(text.spec_bytes()) && name@ == text@,
                Err(NameError::InvalidCharacter) => !has_name_characters(text.spec_bytes()),
                Err(NameError::TooShort) => has_name_characters(text.spec_bytes())
                    && text.spec_bytes().len() < 2,
                Err(NameError::TooLong) => has_name_characters(text.spec_bytes())
                    && text.spec_bytes().len() > 20,
            },
    {
        let bytes = text.as_bytes();
        if !has_permitted_characters(bytes) {
            return Err(NameError::InvalidCharacter);
        }
        if bytes.len() < Self::MIN_LENGTH {
            return Err(NameError::TooShort);
        }
        if bytes.len() > Self::MAX_LENGTH {
            return Err(NameError::TooLong);
        }
        Ok(Self(text.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> (text: &str)
        ensures
            text@ == self@,
            is_name(text.spec_bytes()),
    {
        proof {
            use_type_invariant(self);
        }
        self.0.as_str()
    }
}

impl Clone for Name {
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

fn has_permitted_characters(bytes: &[u8]) -> (permitted: bool)
    ensures
        permitted == has_name_characters(bytes@),
{
    let mut index = 0;
    while index < bytes.len()
        invariant
            index <= bytes.len(),
            forall|i: int| 0 <= i < index ==> is_name_character(#[trigger] bytes@[i]),
        decreases bytes.len() - index,
    {
        let byte = bytes[index];
        if !(is_lower(byte) || is_digit(byte) || byte == b'-' || byte == b'_') {
            return false;
        }
        index += 1;
    }
    true
}

} // verus!
impl fmt::Display for Name {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl fmt::Display for NameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCharacter => formatter
                .write_str("a name can contain only a to z, 0 to 9, hyphen, and underscore"),
            Self::TooShort => write!(
                formatter,
                "a name must have a minimum of {} characters",
                Name::MIN_LENGTH
            ),
            Self::TooLong => write!(
                formatter,
                "a name can have a maximum of {} characters",
                Name::MAX_LENGTH
            ),
        }
    }
}

impl std::error::Error for NameError {}

impl JsonSchema for Name {
    fn schema_name() -> Cow<'static, str> {
        "Name".into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "The name of a user: 2 to 20 characters from a to z, 0 to 9, hyphen, and underscore.",
            "type": "string",
            "pattern": "^[a-z0-9_-]{2,20}$"
        })
    }
}

impl Serialize for Name {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Name {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::{Name, NameError};

    #[test]
    fn accepts_lower_case_letters_digits_hyphen_and_underscore() {
        for valid in ["ana", "ben-2", "a_b", "az", "09", "abcdefghij0123456789"] {
            let name = Name::parse(valid).expect(valid);
            assert_eq!(name.as_str(), valid);
            assert_eq!(name.to_string(), valid);
        }
    }

    #[test]
    fn refuses_a_name_with_less_than_two_characters() {
        assert_eq!(Name::parse(""), Err(NameError::TooShort));
        assert_eq!(Name::parse("a"), Err(NameError::TooShort));
    }

    #[test]
    fn refuses_a_name_with_more_than_twenty_characters() {
        assert_eq!(
            Name::parse("abcdefghij0123456789x"),
            Err(NameError::TooLong)
        );
    }

    #[test]
    fn refuses_a_character_that_is_not_permitted() {
        for invalid in [
            "Ana", "an a", "an.a", "ana\n", "#ana", "@ana", "a\u{f1}a", "`ana", "{ana", "/ana",
            ":ana",
        ] {
            assert_eq!(
                Name::parse(invalid),
                Err(NameError::InvalidCharacter),
                "{invalid:?}"
            );
        }
    }

    #[test]
    fn examines_the_characters_before_the_length() {
        assert_eq!(Name::parse("A"), Err(NameError::InvalidCharacter));
        assert_eq!(
            Name::parse("ABCDEFGHIJ0123456789X"),
            Err(NameError::InvalidCharacter)
        );
    }

    #[test]
    fn describes_each_error() {
        assert_eq!(
            NameError::InvalidCharacter.to_string(),
            "a name can contain only a to z, 0 to 9, hyphen, and underscore"
        );
        assert_eq!(
            NameError::TooShort.to_string(),
            "a name must have a minimum of 2 characters"
        );
        assert_eq!(
            NameError::TooLong.to_string(),
            "a name can have a maximum of 20 characters"
        );
    }
}
